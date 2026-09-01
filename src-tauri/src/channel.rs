//! 独立通道分发层。
//!
//! 本文件只负责选择具体传输通道并调用其函数，不实现 BLE、串口、网络
//! 的连接细节，也不重复实现 Modbus 协议。

use crate::{ble, modbus, network, serial};

use std::sync::{atomic::AtomicBool, Arc};

use serde::{Deserialize, Serialize};

use tauri::AppHandle;

use tauri_plugin_fs::FsExt;
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

    pub fn file_url(path: &str) -> Result<tauri::Url, String> {
        if path.trim().is_empty() {
            return Err(format!("输出路径不能为空"));
        }
        if path.starts_with("content://") || path.starts_with("file://") {
            tauri::Url::parse(path).map_err(|error| format!("输出路径无效: {error}"))
        } else {
            tauri::Url::from_file_path(path).map_err(|_| format!("输出路径无效"))
        }
    }
}
