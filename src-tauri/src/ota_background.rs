#[cfg(target_os = "android")]
pub async fn set_active(_app: &tauri::AppHandle, active: bool) -> Result<(), String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    tauri::wry::prelude::dispatch(move |env, activity, _webview| {
        let result = env
            .call_method(activity, "setOtaBackground", "(Z)Z", &[active.into()])
            .and_then(|value| value.z());
        let result = match result {
            Ok(true) => Ok(()),
            Ok(false) => Err("无法设置 OTA 后台服务".to_string()),
            Err(error) => {
                let _ = env.exception_clear();
                Err(format!("调用 OTA 后台服务失败：{error}"))
            }
        };
        let _ = sender.send(result);
    });
    receiver.await.map_err(|error| error.to_string())?
}

#[cfg(target_os = "ios")]
pub async fn set_active(app: &tauri::AppHandle, active: bool) -> Result<(), String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(ios::set_active(active));
    })
    .map_err(|error| error.to_string())?;
    receiver.await.map_err(|error| error.to_string())?
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub async fn set_active(_app: &tauri::AppHandle, _active: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "ios")]
mod ios {
    use std::sync::Mutex;

    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_ui_kit::{UIApplication, UIBackgroundTaskIdentifier, UIBackgroundTaskInvalid};

    static TASK: Mutex<Option<UIBackgroundTaskIdentifier>> = Mutex::new(None);

    fn end(app: &UIApplication) {
        let task = TASK.lock().unwrap().take();
        if let Some(task) = task {
            app.endBackgroundTask(task);
        }
    }

    pub fn set_active(active: bool) -> Result<(), String> {
        let main = MainThreadMarker::new().ok_or("OTA 后台任务必须在 iOS 主线程设置")?;
        let app = UIApplication::sharedApplication(main);
        if !active {
            end(&app);
            return Ok(());
        }
        if TASK.lock().unwrap().is_some() {
            return Ok(());
        }

        let expiration_app = app.clone();
        let expiration = RcBlock::new(move || {
            tauri_plugin_log::log::warn!(
                "iOS OTA 额外后台执行时间已到期，后续执行依赖蓝牙后台事件"
            );
            end(&expiration_app);
        });
        // This is a bounded execution extension, alongside bluetooth-central;
        // it does not promise unlimited background runtime.
        let task = app.beginBackgroundTaskWithExpirationHandler(Some(&expiration));
        // SAFETY: UIKit exports this immutable sentinel for this exact API.
        if task == unsafe { UIBackgroundTaskInvalid } {
            tauri_plugin_log::log::warn!("iOS 未批准 OTA 额外后台执行时间");
        } else {
            *TASK.lock().unwrap() = Some(task);
        }
        Ok(())
    }
}
