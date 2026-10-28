use std::{fs, io::Write, path::PathBuf, time::Duration};

use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;

#[derive(Clone, Deserialize, Serialize)]
pub struct AppUpdate {
    version: String,
    url: String,
}

#[derive(Default)]
pub struct UpdateState(Mutex<Option<PathBuf>>);

fn download_url(value: &str) -> Result<reqwest::Url, String> {
    reqwest::Url::parse(value)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        .ok_or_else(|| "更新下载地址无效".into())
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<Option<AppUpdate>, String> {
    let url = format!(
        "{}/v1/app/update/{}-{}",
        crate::license::SERVER_URL.trim_end_matches('/'),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "无法检查新版本")?
        .get(url)
        .send()
        .await
        .map_err(|_| "无法连接更新服务")?;
    if matches!(response.status().as_u16(), 204 | 404) {
        return Ok(None);
    }
    let update: AppUpdate = response
        .error_for_status()
        .map_err(|_| "更新服务暂时不可用")?
        .json()
        .await
        .map_err(|_| "更新信息格式无效")?;
    // An empty URL leaves publishing disabled until an actual package is ready.
    if update.url.is_empty() {
        return Ok(None);
    }
    let version = Version::parse(&update.version).map_err(|_| "更新版本号无效")?;
    if version.cmp_precedence(&app.package_info().version).is_le() {
        return Ok(None);
    }
    download_url(&update.url)?;
    Ok(Some(update))
}

fn package_extension(url: &reqwest::Url) -> &'static str {
    let path = url.path().to_ascii_lowercase();
    match std::env::consts::OS {
        "windows" if path.ends_with(".msi") => "msi",
        "windows" => "exe",
        "android" => "apk",
        "ios" => "ipa",
        "macos" if path.ends_with(".pkg") => "pkg",
        "macos" => "dmg",
        "linux" if path.ends_with(".deb") => "deb",
        "linux" if path.ends_with(".rpm") => "rpm",
        _ => "AppImage",
    }
}

#[tauri::command]
pub async fn download_app_update(
    app: AppHandle,
    update: AppUpdate,
    state: State<'_, UpdateState>,
) -> Result<(), String> {
    let mut ready = state.0.try_lock().map_err(|_| "安装包正在下载或处理中")?;
    let version = Version::parse(&update.version).map_err(|_| "更新版本号无效")?;
    if version.cmp_precedence(&app.package_info().version).is_le() {
        return Err("该版本无需更新".into());
    }
    let url = download_url(&update.url)?;
    let extension = package_extension(&url);
    #[cfg(target_os = "ios")]
    let directory = app.path().document_dir().map_err(|_| "无法访问文件目录")?;
    #[cfg(not(target_os = "ios"))]
    let directory = app
        .path()
        .app_cache_dir()
        .map_err(|_| "无法访问下载目录")?
        .join("app-update");
    fs::create_dir_all(&directory).map_err(|_| "无法创建下载目录")?;
    let path = directory.join(format!("RideX-{version}.{extension}"));
    let partial = path.with_extension("part");
    *ready = None;

    let result = async {
        let mut response = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|_| "无法开始下载")?
            .get(url)
            .send()
            .await
            .map_err(|_| "安装包下载失败，请重试")?
            .error_for_status()
            .map_err(|_| "安装包下载失败，请重试")?;
        let total = response.content_length();
        let mut file = fs::File::create(&partial).map_err(|_| "无法保存安装包")?;
        let mut downloaded = 0u64;
        while let Some(chunk) = response.chunk().await.map_err(|_| "下载中断，请重试")? {
            file.write_all(&chunk).map_err(|_| "保存安装包失败")?;
            downloaded += chunk.len() as u64;
        }
        if downloaded == 0 || total.is_some_and(|total| total != downloaded) {
            return Err("安装包下载不完整，请重试".to_string());
        }
        file.sync_all().map_err(|_| "保存安装包失败")?;
        drop(file);
        if path.exists() {
            fs::remove_file(&path).map_err(|_| "无法替换旧安装包")?;
        }
        fs::rename(&partial, &path).map_err(|_| "无法保存安装包")?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    } else {
        *ready = Some(path);
    }
    result
}

#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<bool, String> {
    let ready = state.0.try_lock().map_err(|_| "安装包正在下载或处理中")?;
    let path = ready.as_ref().ok_or("请先下载安装包")?;
    if !path.is_file() {
        return Err("安装包不存在，请重新下载".into());
    }

    #[cfg(target_os = "ios")]
    {
        Err("当前平台请保存安装包".into())
    }
    #[cfg(not(target_os = "ios"))]
    {
        if crate::OTA_STOP.lock().await.is_some() {
            return Err("设备 OTA 更新中，请结束后再安装软件更新".into());
        }
        #[cfg(target_os = "android")]
        {
            let path = path.to_string_lossy().into_owned();
            let (sender, receiver) = tokio::sync::oneshot::channel();
            tauri::wry::prelude::dispatch(move |env, activity, _webview| {
                let result = (|| {
                    let path = env.new_string(path)?;
                    env.call_method(
                        activity,
                        "installAppUpdate",
                        "(Ljava/lang/String;)Z",
                        &[(&path).into()],
                    )?
                    .z()
                })();
                let result = match result {
                    Ok(true) => Ok(true),
                    Ok(false) => Err("请允许安装未知应用；也可以保存安装包后手动安装".to_string()),
                    Err(_) => {
                        let _ = env.exception_clear();
                        Err("无法打开安装器，请重试".to_string())
                    }
                };
                let _ = sender.send(result);
            });
            return receiver.await.map_err(|_| "无法打开安装器")?;
        }
        #[cfg(not(target_os = "android"))]
        {
            use tauri_plugin_opener::OpenerExt;
            crate::disconnect_active_channel().await?;
            crate::ble::disconnect_all()
                .await
                .map_err(|_| "无法断开蓝牙，请稍后再安装")?;
            app.opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map_err(|_| "无法打开安装包，请重试")?;
            #[cfg(target_os = "windows")]
            app.exit(0);
            Ok(true)
        }
    }
}

#[tauri::command]
pub async fn save_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;
    let ready = state.0.try_lock().map_err(|_| "安装包正在下载或处理中")?;
    let source = ready.as_ref().ok_or("请先下载安装包")?;
    if !source.is_file() {
        return Err("安装包不存在，请重新下载".into());
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    // On iOS this exports the existing file from Documents, with its actual contents.
    app.dialog()
        .file()
        .set_title("保存安装包")
        .set_file_name(source.file_name().unwrap().to_string_lossy())
        .save_file(move |path| {
            let _ = sender.send(path);
        });
    let Some(destination) = receiver.await.map_err(|_| "无法打开保存对话框")? else {
        return Ok(false);
    };
    #[cfg(not(target_os = "ios"))]
    {
        use tauri_plugin_fs::{FsExt, OpenOptions};
        if destination.clone().into_path().ok().as_ref() != Some(source) {
            let mut input = fs::File::open(source).map_err(|_| "无法读取安装包")?;
            let mut options = OpenOptions::new();
            options.create(true).write(true).truncate(true);
            let mut output = app
                .fs()
                .open(destination, options)
                .map_err(|_| "无法写入保存位置")?;
            std::io::copy(&mut input, &mut output).map_err(|_| "保存安装包失败")?;
            output.sync_all().map_err(|_| "保存安装包失败")?;
        }
    }
    #[cfg(target_os = "ios")]
    let _ = destination;
    Ok(true)
}
