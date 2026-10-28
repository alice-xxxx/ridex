mod app_update;
mod ble;
mod channel;
mod four_g_ota;
mod license;
mod modbus;
mod network;
mod ota;
mod ota_background;
mod ota_notifications;
mod serial;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, LazyLock, OnceLock,
};
use std::{fs, io::ErrorKind, path::PathBuf};

use tauri::{AppHandle, Emitter, Manager, State};

use tokio::sync::{Mutex, Notify};

use channel::{AnyChannel, ConnectRequest, DiscoverRequest, DiscoveredDevice};
use ota::{OtaFileInfo, OtaPackRequest, OtaStartResult};

static CHANNEL: LazyLock<Mutex<Option<AnyChannel>>> = LazyLock::new(|| Mutex::new(None));
static OTA_STOP: LazyLock<Mutex<Option<Arc<Notify>>>> = LazyLock::new(|| Mutex::new(None));
static EXITING: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct LicenseState(OnceLock<license::License>);

impl LicenseState {
    fn ensure_authorized(&self) -> Result<(), String> {
        self.0
            .get()
            .ok_or_else(|| "应用仍在启动".to_string())?
            .ensure_authorized()
    }
}

#[tauri::command]
fn license_status(status: State<'_, LicenseState>) -> Option<license::License> {
    status.0.get().cloned()
}

#[tauri::command]
async fn exit_app(app: AppHandle) -> Result<(), String> {
    if EXITING.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let result = async {
        disconnect_active_channel().await?;
        ble::disconnect_all()
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    if let Err(error) = result {
        EXITING.store(false, Ordering::Release);
        return Err(error);
    }
    app.exit(0);
    Ok(())
}

fn component_catalog_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    if !matches!(
        file_name,
        "vcu.json" | "bms.json" | "ble.json" | "mcu.json" | "abs.json" | "tft.json" | "light.json"
    ) {
        return Err("不支持的部件配置文件名".to_string());
    }
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?
        .join("component-catalogs")
        .join(file_name))
}

#[tauri::command]
fn read_component_catalog(
    file_name: String,
    app: AppHandle,
    status: State<'_, LicenseState>,
) -> Result<Option<String>, String> {
    status.ensure_authorized()?;
    let path = component_catalog_path(&app, &file_name)?;
    match fs::read_to_string(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("读取 {file_name} 失败: {error}")),
    }
}

#[tauri::command]
fn write_component_catalog(
    file_name: String,
    source: String,
    app: AppHandle,
    status: State<'_, LicenseState>,
) -> Result<(), String> {
    status.ensure_authorized()?;
    let path = component_catalog_path(&app, &file_name)?;
    let directory = path.parent().ok_or("配置目录无效")?;
    fs::create_dir_all(directory).map_err(|error| format!("创建配置目录失败: {error}"))?;
    fs::write(path, source).map_err(|error| format!("保存 {file_name} 失败: {error}"))
}

#[tauri::command]
async fn discover(
    request: DiscoverRequest,
    app: AppHandle,
    status: State<'_, LicenseState>,
) -> Result<Vec<DiscoveredDevice>, String> {
    status.ensure_authorized()?;
    let state = CHANNEL.lock().await;
    if state.is_none() && matches!(&request, DiscoverRequest::Ble { .. }) {
        ble::disconnect_all()
            .await
            .map_err(|error| error.to_string())?;
    }
    AnyChannel::discover(request, &app).await
}

#[tauri::command]
fn network_credentials(
    platform: network::NetworkPlatform,
    app: AppHandle,
    status: State<'_, LicenseState>,
) -> Result<network::CredentialStatus, String> {
    status.ensure_authorized()?;
    network::credential_status(&app, platform)
}

#[tauri::command]
async fn connect(
    request: ConnectRequest,
    app: AppHandle,
    status: State<'_, LicenseState>,
) -> Result<(), String> {
    status.ensure_authorized()?;
    let mut state = CHANNEL.lock().await;
    if EXITING.load(Ordering::Acquire) {
        return Err("应用正在退出".to_string());
    }
    if let Some(previous) = state.as_mut() {
        previous.disconnect().await?;
    }
    *state = None;
    if matches!(&request, ConnectRequest::Ble { .. }) {
        ble::disconnect_all()
            .await
            .map_err(|error| error.to_string())?;
    }
    *state = Some(AnyChannel::connect(request, &app).await?);
    if let Some(AnyChannel::Ble { channel }) = state.as_ref() {
        let mut connected = channel.connection_state();
        tauri::async_runtime::spawn(async move {
            let _ = connected.wait_for(|connected| !*connected).await;
            let mut state = CHANNEL.lock().await;
            // A callback from an old connection must not clear a replacement connection.
            if let Some(AnyChannel::Ble { channel }) = state.as_ref() {
                if channel.connection_state().same_channel(&connected) {
                    *state = None;
                    let _ = app.emit(
                        "connection-closed",
                        serde_json::json!({ "transport": "ble", "reason": "蓝牙连接已断开" }),
                    );
                }
            }
        });
    }
    Ok(())
}

#[tauri::command]
async fn disconnect(status: State<'_, LicenseState>) -> Result<(), String> {
    status.ensure_authorized()?;
    disconnect_active_channel().await
}

async fn disconnect_active_channel() -> Result<(), String> {
    if let Some(stop) = OTA_STOP.lock().await.as_ref() {
        stop.notify_one();
    }
    let mut state = CHANNEL.lock().await;
    if let Some(active) = state.as_mut() {
        active.disconnect().await?;
        *state = None;
    }
    Ok(())
}

#[tauri::command]
async fn authenticate(
    vehicle_code: String,
    timeout_ms: u64,
    status: State<'_, LicenseState>,
) -> Result<(), String> {
    status.ensure_authorized()?;

    CHANNEL
        .lock()
        .await
        .as_mut()
        .ok_or_else(|| "当前没有已连接的通道".to_string())?
        .authenticate(&vehicle_code, timeout_ms)
        .await?;
    Ok(())
}

#[tauri::command]
async fn modbus_tx(
    protocol: modbus::Protocol,
    timeout_ms: u64,
    status: State<'_, LicenseState>,
) -> Result<modbus::TransactionResult, String> {
    status.ensure_authorized()?;

    CHANNEL
        .lock()
        .await
        .as_ref()
        .ok_or_else(|| "当前没有已连接的通道".to_string())?
        .modbus_tx(&protocol, timeout_ms)
        .await
}

#[tauri::command]
fn ota_info(
    path: String,
    app: AppHandle,
    status: State<'_, LicenseState>,
) -> Result<OtaFileInfo, String> {
    status.ensure_authorized()?;
    ota::ota_info(path, app)
}

#[tauri::command]
fn ota_pack(
    input_path: String,
    output_path: String,
    request: OtaPackRequest,
    app: AppHandle,
    status: State<'_, LicenseState>,
) -> Result<(), String> {
    status.ensure_authorized()?;
    ota::ota_pack(input_path, output_path, request, app)
}

#[tauri::command]
async fn ota_start(
    app: AppHandle,
    path: String,
    is_ota: bool,
    request: OtaPackRequest,
    cycles: u16,
    status: State<'_, LicenseState>,
) -> Result<OtaStartResult, String> {
    status.ensure_authorized()?;

    let stop = Arc::new(Notify::new());
    {
        let mut running = OTA_STOP.lock().await;
        if running.is_some() {
            return Err("已有 OTA 更新正在进行".to_string());
        }
        *running = Some(Arc::clone(&stop));
    }

    let result = tokio::select! {
        biased;
        _ = stop.notified() => Err("OTA 已停止".to_string()),
        result = async {
            let state = CHANNEL.lock().await;
            let channel = state
                .as_ref()
                .ok_or_else(|| "当前没有已连接的通道".to_string())?;
            let disconnected = async {
                if let AnyChannel::Ble { channel } = channel {
                    channel.wait_disconnected().await;
                } else {
                    std::future::pending::<()>().await;
                }
            };
            tokio::select! {
                biased;
                _ = disconnected => Err("BLE 已断开".to_string()),
                result = async {
                    ota_notifications::request_permission().await;
                    ota_background::set_active(true).await?;
                    ota_notifications::start();
                    let firmware = ota::get_bin(&path, is_ota, &app)?;
                    let bytes_per_cycle = firmware.len();
                    channel
                        .ota_start(
                            &app,
                            request.device_address,
                            &firmware,
                            request.manufacturer,
                            request.hardware_version,
                            request.software_version,
                            cycles,
                        )
                        .await?;
                    Ok(OtaStartResult { bytes_per_cycle })
                } => result,
            }
        } => result,
    };
    if let Err(error) = ota_background::set_active(false).await {
        tauri_plugin_log::log::warn!("结束 OTA 后台服务失败：{error}");
    }
    ota_notifications::finish(result.as_ref().map(|_| ()).map_err(Clone::clone));
    OTA_STOP.lock().await.take();

    result
}

#[tauri::command]
async fn ota_cancel(status: State<'_, LicenseState>) -> Result<(), String> {
    status.ensure_authorized()?;

    if let Some(stop) = OTA_STOP.lock().await.as_ref() {
        stop.notify_one();
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    tauri_plugin_log::log::LevelFilter::Debug
                } else {
                    tauri_plugin_log::log::LevelFilter::Warn
                })
                .clear_format()
                .build(),
        )
        .plugin(tauri_plugin_blec::init())
        .plugin(tauri_plugin_machine_uid::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(focused) = event {
                ota_notifications::focused(*focused);
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async {
                    if let Err(error) = exit_app(app).await {
                        tauri_plugin_log::log::error!("退出前断开设备失败：{error}");
                    }
                });
            }
        });

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    let builder = builder
        .plugin(tauri_plugin_serialplugin::init())
        .plugin(tauri_plugin_opener::init());

    builder
        .setup(move |app| {
            ota_notifications::init(app.handle().clone())?;
            app.manage(LicenseState::default());
            app.manage(app_update::UpdateState::default());
            let app_handle = app.handle().clone();
            let fallback_handle = app_handle.clone();
            if let Err(error) = std::thread::Builder::new()
                .name("license-initialization".into())
                .spawn(move || {
                    let mut license = match license::License::init(&app_handle) {
                        Ok(license) => license,
                        Err(error) => license::License::unavailable(error),
                    };
                    if license.message.is_none() {
                        let _ = tauri::async_runtime::block_on(license.authorize(&app_handle));
                    }
                    let _ = app_handle.state::<LicenseState>().0.set(license);
                })
            {
                let _ =
                    fallback_handle
                        .state::<LicenseState>()
                        .0
                        .set(license::License::unavailable(format!(
                            "无法启动授权任务: {error}"
                        )));
            }
            app.get_webview_window("main")
                .ok_or("main window not found")?
                .show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            license_status,
            app_update::check_app_update,
            app_update::download_app_update,
            app_update::install_app_update,
            app_update::save_app_update,
            exit_app,
            read_component_catalog,
            write_component_catalog,
            discover,
            network_credentials,
            connect,
            disconnect,
            authenticate,
            modbus_tx,
            ota_info,
            ota_pack,
            ota_start,
            ota_cancel,
            four_g_ota::four_g_ota_credentials,
            four_g_ota::four_g_ota_login,
            four_g_ota::four_g_ota_upload,
            four_g_ota::four_g_ota_start,
            four_g_ota::four_g_ota_progress,
            four_g_ota::four_g_ota_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
