//! 独立通道分发层。
//!
//! 本文件只负责选择具体传输通道并调用其函数，不实现 BLE、串口、网络
//! 的连接细节，也不重复实现 Modbus 协议。

use crate::{ble, modbus, network, serial};

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use crc::{Crc, CRC_32_ISO_HDLC};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use tauri::{AppHandle, Emitter};
use tauri_plugin_fs::FsExt;

const OTA_PACKET_SIZE: usize = 128;
const OTA_MAX_PACKETS: usize = u16::MAX as usize + 1;
const OTA_TIMEOUT_RETRIES: usize = 3;
const OTA_CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

/// 创建通道时所需的参数。
#[derive(Debug, Deserialize)]
#[serde(
    tag = "transport",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConnectRequest {
    Ble {
        address: String,
        name: String,
        timeout_ms: u64,
    },
    Serial {
        path: String,
        baud_rate: u32,
    },
    Network {
        name: String,
        platform: network::NetworkPlatform,
        username: String,
        password: String,
        timeout_ms: u64,
    },
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "transport",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DiscoverRequest {
    Ble {
        timeout_ms: u64,
        service_uuid: Option<String>,
    },
    Serial,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscoveredDevice {
    Ble(ble::DeviceInfo),
    Serial { path: String, name: String },
}

/// 统一持有一种具体传输通道。
pub enum AnyChannel {
    Ble {
        channel: ble::BleChannel,
        address: String,
        ota_stop: Arc<AtomicBool>,
    },
    Serial {
        channel: serial::SerialChannel,
        ota_stop: Arc<AtomicBool>,
    },
    Network {
        channel: network::NetworkChannel,
        ota_stop: Arc<AtomicBool>,
    },
}

impl AnyChannel {
    /// 根据连接参数创建对应的具体通道。
    pub async fn connect(request: ConnectRequest, app: &AppHandle) -> Result<Self, String> {
        match request {
            ConnectRequest::Ble {
                address,
                name,
                timeout_ms,
            } => {
                let disconnect_address = address.clone();
                ble::BleChannel::connect(address, name, timeout_ms)
                    .await
                    .map(|channel| Self::Ble {
                        channel,
                        address: disconnect_address,
                        ota_stop: Arc::new(AtomicBool::new(false)),
                    })
                    .map_err(|error| error.to_string())
            }
            ConnectRequest::Serial { path, baud_rate } => {
                serial::SerialChannel::connect(app.clone(), path, baud_rate)
                    .await
                    .map(|channel| Self::Serial {
                        channel,
                        ota_stop: Arc::new(AtomicBool::new(false)),
                    })
                    .map_err(|error| error.to_string())
            }
            ConnectRequest::Network {
                name,
                platform,
                username,
                password,
                timeout_ms,
            } => network::NetworkChannel::connect(
                app, name, platform, &username, &password, timeout_ms,
            )
            .await
            .map(|channel| Self::Network {
                channel,
                ota_stop: Arc::new(AtomicBool::new(false)),
            }),
        }
    }

    /// 调用当前具体通道的 Modbus 发送函数。
    pub async fn modbus_tx(
        &self,
        request: &modbus::Protocol,
        timeout_ms: u64,
    ) -> Result<modbus::TransactionResult, String> {
        match self {
            Self::Ble { channel, .. } => channel
                .modbus_tx(request, timeout_ms)
                .await
                .map_err(|error| error.to_string()),
            Self::Serial { channel, .. } => channel
                .modbus_tx(request, timeout_ms)
                .await
                .map_err(|error| error.to_string()),
            Self::Network { channel, .. } => channel.modbus_tx(request, timeout_ms).await,
        }
    }

    /// 断开当前通道并释放其底层资源。
    pub async fn disconnect(&mut self) -> Result<(), String> {
        match self {
            Self::Ble {
                channel, address, ..
            } => channel
                .disconnect(address)
                .await
                .map_err(|error| error.to_string()),
            Self::Serial { channel, .. } => channel
                .disconnect()
                .await
                .map_err(|error| error.to_string()),
            Self::Network { .. } => Ok(()),
        }
    }

    /// 分发 BLE 和串口设备发现。
    pub async fn discover(
        request: DiscoverRequest,
        app: &AppHandle,
    ) -> Result<Vec<DiscoveredDevice>, String> {
        match request {
            DiscoverRequest::Ble {
                timeout_ms,
                service_uuid,
            } => ble::BleChannel::discover(timeout_ms, service_uuid.as_deref())
                .await
                .map(|devices| devices.into_iter().map(DiscoveredDevice::Ble).collect())
                .map_err(|error| error.to_string()),
            DiscoverRequest::Serial => serial::SerialChannel::discover(app)
                .map(|devices| {
                    devices
                        .into_iter()
                        .map(|(path, name)| DiscoveredDevice::Serial { path, name })
                        .collect()
                })
                .map_err(|error| error.to_string()),
        }
    }

    /// 只有 BLE 通道支持认证。
    pub async fn authenticate(
        &mut self,
        vehicle_code: &str,
        timeout_ms: u64,
    ) -> Result<(), String> {
        match self {
            Self::Ble { channel, .. } => channel
                .authenticate(vehicle_code, timeout_ms)
                .await
                .map_err(|error| error.to_string()),
            Self::Serial { .. } | Self::Network { .. } => Err("当前通道不支持认证".to_string()),
        }
    }

    pub fn read_file(app: &AppHandle, path: &str) -> Result<Vec<u8>, String> {
        if path.trim().is_empty() {
            return Err("固件路径不能为空".to_string());
        }
        let url = if path.starts_with("content://") || path.starts_with("file://") {
            tauri::Url::parse(path).map_err(|error| error.to_string())?
        } else {
            tauri::Url::from_file_path(path).map_err(|_| "固件路径无效".to_string())?
        };
        app.fs()
            .read(url)
            .map_err(|error| format!("读取固件失败: {error}"))
    }
    /// 使用当前通道发送 OTA 握手、数据分包和结果确认。
    ///
    /// 固件内容由调用方提供；数据不足 128 字节的最后一包由协议组帧时补 0xFF。
    pub async fn ota_start(
        &self,
        app: &AppHandle,
        device: u8,
        firmware: &[u8],
        manufacturer: [u8; 4],
        hardware_version: [u8; 2],
        software_version: [u8; 2],
        cycles: u16,
    ) -> Result<(), String> {
        if firmware.is_empty() {
            return Err("OTA 固件内容为空".to_string());
        }
        if !(1..=100).contains(&cycles) {
            return Err("OTA 循环次数必须在 1..=100 之间".to_string());
        }
        if firmware.len().div_ceil(OTA_PACKET_SIZE) > OTA_MAX_PACKETS {
            return Err("OTA 固件超过 8 MiB".to_string());
        }

        self.ota_stop_flag().store(false, Ordering::Release);
        let firmware_crc = OTA_CRC32.checksum(firmware);
        let packets = firmware.len().div_ceil(OTA_PACKET_SIZE);
        let total_packets = packets * usize::from(cycles);

        for cycle in 0..cycles {
            self.ensure_ota_running()?;
            self.ota_modbus_tx(
                &modbus::Protocol::OtaHandshake {
                    device,
                    firmware_size: firmware.len() as u32,
                    manufacturer,
                    hardware_version,
                    software_version,
                    firmware_crc,
                },
                5_000,
                &format!("OTA 握手（第 {}/{} 次）", cycle + 1, cycles),
            )
            .await?;

            for (index, packet) in firmware.chunks(OTA_PACKET_SIZE).enumerate() {
                self.ensure_ota_running()?;
                self.ota_modbus_tx(
                    &modbus::Protocol::OtaData {
                        device,
                        sequence: index as u16,
                        data: packet.to_vec(),
                    },
                    5_000,
                    &format!(
                        "OTA 数据包（第 {}/{} 次，第 {}/{} 包）",
                        cycle + 1,
                        cycles,
                        index + 1,
                        packets,
                    ),
                )
                .await?;
                let completed_packets = usize::from(cycle) * packets + index + 1;
                emit_ota_progress(app, (completed_packets * 100 / total_packets) as u8);
            }

            self.ensure_ota_running()?;
            self.ota_modbus_tx(
                &modbus::Protocol::OtaResult { device },
                5_000,
                &format!("OTA 结果确认（第 {}/{} 次）", cycle + 1, cycles),
            )
            .await?;

            if cycle + 1 < cycles {
                sleep(Duration::from_secs(2)).await;
            } else {
                emit_ota_progress(app, 100);
            }
        }

        Ok(())
    }

    /// 返回 OTA 停止标志的共享句柄。命令层在 OTA 持有通道锁时也需要能够取消。
    pub fn ota_stop_handle(&self) -> Arc<AtomicBool> {
        match self {
            Self::Ble { ota_stop, .. } => Arc::clone(ota_stop),
            Self::Serial { ota_stop, .. } => Arc::clone(ota_stop),
            Self::Network { ota_stop, .. } => Arc::clone(ota_stop),
        }
    }

    fn ota_stop_flag(&self) -> &AtomicBool {
        match self {
            Self::Ble { ota_stop, .. } => ota_stop,
            Self::Serial { ota_stop, .. } => ota_stop,
            Self::Network { ota_stop, .. } => ota_stop,
        }
    }

    fn ensure_ota_running(&self) -> Result<(), String> {
        if self.ota_stop_flag().load(Ordering::Acquire) {
            Err("OTA 已停止".to_string())
        } else {
            Ok(())
        }
    }

    /// OTA 失败时保留本次实际组装的完整请求帧，便于前端定位失败阶段和协议内容。
    async fn ota_modbus_tx(
        &self,
        request: &modbus::Protocol,
        timeout_ms: u64,
        stage: &str,
    ) -> Result<modbus::TransactionResult, String> {
        let request_frame = modbus::assembly_frame(request.clone())
            .map_err(|error| format!("{stage}组帧失败：{error}"))?;
        let mut attempts = 0;
        loop {
            attempts += 1;
            match self.modbus_tx(request, timeout_ms).await {
                Ok(result) => return Ok(result),
                Err(error) if is_timeout_error(&error) && attempts < OTA_TIMEOUT_RETRIES => {
                    continue;
                }
                Err(error) => {
                    let retries = attempts.saturating_sub(1);
                    return Err(format!(
                        "{stage}失败（已尝试 {attempts} 次，超时重试 {retries} 次）：{error}；发送帧：{}",
                        format_frame_bytes(&request_frame),
                    ));
                }
            }
        }
    }
}

fn is_timeout_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    error.contains("超时")
        || normalized.contains("timeout")
        || normalized.contains("timed out")
        || normalized.contains("no data received within")
}

fn format_frame_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn emit_ota_progress(app: &AppHandle, percent: u8) {
    let _ = app.emit("ota-progress", percent);
}
