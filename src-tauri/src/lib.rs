mod ble;
mod channel;
mod license;
mod modbus;
mod network;
mod serial;

use std::io::Write;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, LazyLock,
};

use crc::{Crc, CRC_32_ISO_HDLC};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_fs::{FsExt, OpenOptions};
use tokio::sync::Mutex;

use channel::{AnyChannel, ConnectRequest, DiscoverRequest, DiscoveredDevice};

const OTA_HEADER_SIZE: usize = 64;
const OTA_CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OtaInfoRequest {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OtaPackRequest {
    input_path: String,
    output_path: String,
    manufacturer: [u8; 4],
    device_type: [u8; 2],
    hardware_version: [u8; 2],
    software_version: [u8; 2],
    device_address: u8,
    description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OtaFileInfo {
    file_crc: u32,
    file_size: u32,
    manufacturer: [u8; 4],
    device_type: [u8; 2],
    hardware_version: [u8; 2],
    software_version: [u8; 2],
    device_address: Option<u8>,
    description: String,
    upgrade_crc: u32,
    upgrade_size: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OtaStartResult {
    bytes_per_cycle: usize,
}

struct ActiveChannel {
    channel: AnyChannel,
}

static CHANNEL: LazyLock<Mutex<Option<ActiveChannel>>> = LazyLock::new(|| Mutex::new(None));
// ota_start 持有 CHANNEL 锁直到传输结束；取消不能再通过 try_lock 获取 CHANNEL，
// 因此单独保存当前 OTA 的停止标志句柄。
static OTA_STOP: LazyLock<Mutex<Option<Arc<AtomicBool>>>> = LazyLock::new(|| Mutex::new(None));

#[tauri::command]
async fn channel_caps(status: State<'_, Mutex<license::License>>) -> Result<bool, String> {
    require_authorized(&status).await?;
    Ok(cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )))
}

#[tauri::command]
async fn license_status(
    status: State<'_, Mutex<license::License>>,
) -> Result<license::License, String> {
    Ok(status.lock().await.clone())
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
async fn discover(
    request: DiscoverRequest,
    app: AppHandle,
    status: State<'_, Mutex<license::License>>,
) -> Result<Vec<DiscoveredDevice>, String> {
    require_authorized(&status).await?;
    AnyChannel::discover(request, &app).await
}

#[tauri::command]
fn network_credentials(
    platform: network::NetworkPlatform,
    app: AppHandle,
    status: State<'_, Mutex<license::License>>,
) -> Result<network::CredentialStatus, String> {
    status
        .inner()
        .try_lock()
        .map_err(|_| "授权状态正在更新".to_string())?
        .ensure_authorized()?;
    network::credential_status(&app, platform)
}

#[tauri::command]
async fn connect(
    request: ConnectRequest,
    app: AppHandle,
    status: State<'_, Mutex<license::License>>,
) -> Result<(), String> {
    require_authorized(&status).await?;
    let connected = AnyChannel::connect(request, &app).await?;
    let mut state = CHANNEL.lock().await;
    if let Some(previous) = state.as_mut() {
        let _ = AnyChannel::disconnect(&mut previous.channel).await;
    }
    *state = Some(ActiveChannel { channel: connected });
    Ok(())
}

#[tauri::command]
async fn disconnect(status: State<'_, Mutex<license::License>>) -> Result<(), String> {
    require_authorized(&status).await?;
    disconnect_active_channel().await
}

async fn disconnect_active_channel() -> Result<(), String> {
    let mut state = CHANNEL.lock().await;
    if let Some(active) = state.as_mut() {
        AnyChannel::disconnect(&mut active.channel).await?;
        *state = None;
    }
    Ok(())
}

#[tauri::command]
async fn authenticate(
    vehicle_code: String,
    timeout_ms: u64,
    status: State<'_, Mutex<license::License>>,
) -> Result<(), String> {
    require_authorized(&status).await?;
    let mut state = CHANNEL.lock().await;
    let active = state
        .as_mut()
        .ok_or_else(|| "当前没有已连接的通道".to_string())?;
    active
        .channel
        .authenticate(&vehicle_code, timeout_ms)
        .await?;
    Ok(())
}

#[tauri::command]
async fn modbus_tx(
    protocol: modbus::Protocol,
    timeout_ms: u64,
    status: State<'_, Mutex<license::License>>,
) -> Result<modbus::TransactionResult, String> {
    require_authorized(&status).await?;
    let state = CHANNEL.lock().await;
    let active = state
        .as_ref()
        .ok_or_else(|| "当前没有已连接的通道".to_string())?;
    active.channel.modbus_tx(&protocol, timeout_ms).await
}

#[tauri::command]
fn ota_info(
    request: OtaInfoRequest,
    app: AppHandle,
    status: State<'_, Mutex<license::License>>,
) -> Result<OtaFileInfo, String> {
    status
        .inner()
        .try_lock()
        .map_err(|_| "授权状态正在更新".to_string())?
        .ensure_authorized()?;
    let bytes = AnyChannel::read_file(&app, &request.path)?;
    parse_ota_file(&bytes)
}

#[tauri::command]
fn ota_pack(
    request: OtaPackRequest,
    app: AppHandle,
    status: State<'_, Mutex<license::License>>,
) -> Result<OtaFileInfo, String> {
    status
        .inner()
        .try_lock()
        .map_err(|_| "授权状态正在更新".to_string())?
        .ensure_authorized()?;
    let firmware = AnyChannel::read_file(&app, &request.input_path)?;
    if firmware.is_empty() {
        return Err("BIN 固件内容为空".to_string());
    }
    if !request.description.is_ascii() {
        return Err("文件描述只能使用 ASCII 字符".to_string());
    }
    if request.description.len() > 20 {
        return Err("文件描述最多 20 个字节".to_string());
    }

    let total_size = OTA_HEADER_SIZE
        .checked_add(firmware.len())
        .ok_or_else(|| "BIN 固件过大".to_string())?;
    let file_size = u32::try_from(total_size - 8).map_err(|_| "OTA 文件超过 4 GiB".to_string())?;
    let upgrade_size =
        u32::try_from(firmware.len()).map_err(|_| "BIN 固件超过 4 GiB".to_string())?;
    let mut ota = vec![0u8; total_size];
    ota[4..8].copy_from_slice(&file_size.to_be_bytes());
    ota[8..12].copy_from_slice(&request.manufacturer);
    ota[12..14].copy_from_slice(&request.device_type);
    ota[14..16].copy_from_slice(&request.hardware_version);
    ota[16..18].copy_from_slice(&request.software_version);
    ota[18] = request.device_address;
    ota[32..32 + request.description.len()].copy_from_slice(request.description.as_bytes());
    ota[56..60].copy_from_slice(&OTA_CRC32.checksum(&firmware).to_be_bytes());
    ota[60..64].copy_from_slice(&upgrade_size.to_be_bytes());
    ota[64..].copy_from_slice(&firmware);
    let file_crc = OTA_CRC32.checksum(&ota[4..]);
    ota[0..4].copy_from_slice(&file_crc.to_be_bytes());

    let output_url = file_url(&request.output_path, "输出路径")?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut file = app
        .fs()
        .open(output_url, options)
        .map_err(|error| format!("打开 OTA 输出文件失败: {error}"))?;
    file.write_all(&ota)
        .map_err(|error| format!("写入 OTA 文件失败: {error}"))?;
    file.flush()
        .map_err(|error| format!("保存 OTA 文件失败: {error}"))?;

    parse_ota_file(&ota)
}

#[tauri::command]
async fn ota_start(
    app: AppHandle,
    path: String,
    file_kind: String,
    device: u8,
    manufacturer: [u8; 4],
    hardware_version: [u8; 2],
    software_version: [u8; 2],
    cycles: u16,
    status: State<'_, Mutex<license::License>>,
) -> Result<OtaStartResult, String> {
    require_authorized(&status).await?;
    // 由 Rust 统一读取文件；移动端 content URI 和桌面路径都走同一套权限/读取逻辑。
    // packaged 的 64 字节头部只用于校验，不能作为设备升级内容发送。
    let file = AnyChannel::read_file(&app, &path)?;
    let firmware = match file_kind.as_str() {
        "binary" => file,
        "packaged" => {
            parse_ota_file(&file)?;
            file.get(OTA_HEADER_SIZE..)
                .ok_or_else(|| "OTA 升级内容为空".to_string())?
                .to_vec()
        }
        _ => return Err("OTA 文件类型无效".to_string()),
    };
    let bytes_per_cycle = firmware.len();
    let result = {
        let state = CHANNEL.lock().await;
        let active = state
            .as_ref()
            .ok_or_else(|| "当前没有已连接的通道".to_string())?;
        let stop = active.channel.ota_stop_handle();
        *OTA_STOP.lock().await = Some(stop);
        active
            .channel
            .ota_start(
                &app,
                device,
                &firmware,
                manufacturer,
                hardware_version,
                software_version,
                cycles,
            )
            .await
    };
    OTA_STOP.lock().await.take();
    result.map(|()| OtaStartResult { bytes_per_cycle })
}

#[tauri::command]
async fn ota_cancel(status: State<'_, Mutex<license::License>>) -> Result<(), String> {
    require_authorized(&status).await?;
    if let Some(stop) = OTA_STOP.lock().await.as_ref() {
        stop.store(true, Ordering::Release);
    }
    Ok(())
}

async fn require_authorized(status: &State<'_, Mutex<license::License>>) -> Result<(), String> {
    status.lock().await.ensure_authorized()
}

fn parse_ota_file(data: &[u8]) -> Result<OtaFileInfo, String> {
    if data.len() < OTA_HEADER_SIZE {
        return Err(format!("OTA 文件至少需要 {OTA_HEADER_SIZE} 字节头部"));
    }
    let read_u32 = |offset: usize| {
        u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    };
    let file_crc = read_u32(0);
    let file_size = read_u32(4);
    let upgrade_crc = read_u32(56);
    let upgrade_size = read_u32(60);
    let expected_file_size =
        u32::try_from(data.len() - 8).map_err(|_| "OTA 文件超过 4 GiB".to_string())?;
    let expected_upgrade_size = u32::try_from(data.len() - OTA_HEADER_SIZE)
        .map_err(|_| "OTA 升级内容超过 4 GiB".to_string())?;
    if file_size != expected_file_size || upgrade_size != expected_upgrade_size {
        return Err("OTA 文件大小字段不匹配".to_string());
    }

    let description_bytes = &data[32..52];
    let description_len = description_bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(description_bytes.len());
    let description = String::from_utf8(description_bytes[..description_len].to_vec())
        .map_err(|_| "OTA 描述必须是 ASCII".to_string())?;
    let content = &data[OTA_HEADER_SIZE..];
    if OTA_CRC32.checksum(content) != upgrade_crc || OTA_CRC32.checksum(&data[4..]) != file_crc {
        return Err("OTA CRC 校验失败".to_string());
    }

    Ok(OtaFileInfo {
        file_crc,
        file_size,
        manufacturer: [data[8], data[9], data[10], data[11]],
        device_type: [data[12], data[13]],
        hardware_version: [data[14], data[15]],
        software_version: [data[16], data[17]],
        device_address: (data[18] != 0xFF).then_some(data[18]),
        description,
        upgrade_crc,
        upgrade_size,
    })
}

fn file_url(path: &str, label: &str) -> Result<tauri::Url, String> {
    if path.trim().is_empty() {
        return Err(format!("{label}不能为空"));
    }
    if path.starts_with("content://") || path.starts_with("file://") {
        tauri::Url::parse(path).map_err(|error| format!("{label}无效: {error}"))
    } else {
        tauri::Url::from_file_path(path).map_err(|_| format!("{label}无效"))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
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
            let license = match license::License::init(app.handle()) {
                Ok(license) => license,
                Err(error) => license::License::unavailable(error),
            };
            let should_authorize = license.message.is_none();
            app.manage(Mutex::new(license));
            app.get_webview_window("main")
                .ok_or("main window not found")?
                .show()?;
            if should_authorize {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<Mutex<license::License>>();
                    let mut license = state.lock().await;
                    let _ = license.authorize(&app_handle).await;
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            channel_caps,
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
