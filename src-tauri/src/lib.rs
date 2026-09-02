mod ble;
mod channel;
mod license;
mod modbus;
mod network;
mod ota;
mod serial;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, LazyLock, OnceLock,
};

use tauri::{AppHandle, Manager, State};

use tokio::sync::Mutex;

use channel::{AnyChannel, ConnectRequest, DiscoverRequest, DiscoveredDevice};
use ota::{OtaFileInfo, OtaPackRequest, OtaStartResult};

static CHANNEL: LazyLock<Mutex<Option<AnyChannel>>> = LazyLock::new(|| Mutex::new(None));
static OTA_STOP: LazyLock<Mutex<Option<Arc<AtomicBool>>>> = LazyLock::new(|| Mutex::new(None));

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
fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
async fn discover(
    request: DiscoverRequest,
    app: AppHandle,
    status: State<'_, LicenseState>,
) -> Result<Vec<DiscoveredDevice>, String> {
    status.ensure_authorized()?;
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
    let connected = AnyChannel::connect(request, &app).await?;
    let mut state = CHANNEL.lock().await;
    if let Some(previous) = state.as_mut() {
        let _ = previous.disconnect().await;
    }
    *state = Some(connected);
    Ok(())
}

#[tauri::command]
async fn disconnect(status: State<'_, LicenseState>) -> Result<(), String> {
    status.ensure_authorized()?;
    disconnect_active_channel().await
}

async fn disconnect_active_channel() -> Result<(), String> {
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
    status: State<'_, license::License>,
) -> Result<OtaStartResult, String> {
    status.ensure_authorized()?;

    let firmware = ota::get_bin(&path, is_ota, &app)?;

    let bytes_per_cycle = firmware.len();

    let stop = CHANNEL
        .lock()
        .await
        .as_ref()
        .map(|active| active.ota_stop_handle());
    if let Some(stop) = stop {
        *OTA_STOP.lock().await = Some(stop);
    }

    let result = CHANNEL
        .lock()
        .await
        .as_ref()
        .ok_or_else(|| "当前没有已连接的通道".to_string())?
        .ota_start(
            &app,
            request.device_address,
            &firmware,
            request.manufacturer,
            request.hardware_version,
            request.software_version,
            cycles,
        )
        .await;
    OTA_STOP.lock().await.take();

    result.map(|()| OtaStartResult { bytes_per_cycle })
}

#[tauri::command]
async fn ota_cancel(status: State<'_, LicenseState>) -> Result<(), String> {
    status.ensure_authorized()?;

    OTA_STOP
        .lock()
        .await
        .as_ref()
        .ok_or_else(|| "当前没有已连接的通道".to_string())?
        .store(true, Ordering::Release);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
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
        .on_window_event(|_window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                tauri::async_runtime::spawn(async {
                    let _ = disconnect_active_channel().await;
                });
            }
        });

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_serialplugin::init());

    builder
        .setup(move |app| {
            app.manage(LicenseState::default());
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
            exit_app,
            discover,
            network_credentials,
            connect,
            disconnect,
            authenticate,
            modbus_tx,
            ota_info,
            ota_pack,
            ota_start,
            ota_cancel
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
