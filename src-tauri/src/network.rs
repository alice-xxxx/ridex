//! 独立网络通信实现。
//!
//! 网络接口返回的已经是完整 Modbus 帧，不经过字节环形和帧捕获器。
//! 每次 modbus_tx 直接完成基线查询、发送请求和回复轮询。

use crate::modbus;

use std::{
    io::{ErrorKind, Write},
    time::Duration,
};

use aes_gcm::{
    aead::{Aead, Generate, KeyInit},
    Aes256Gcm, Nonce,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FsExt, OpenOptions};
use zeroize::Zeroizing;

use tokio::time::timeout;

const POLL_INTERVAL_MS: u64 = 300;
const PRODUCTION_BASE_URL: &str = "https://app.garow.com/garow";
const TEST_BASE_URL: &str = "https://dev-iot.garow.com/garow";
const CREDENTIAL_MAGIC: &[u8; 8] = b"RIDEXN01";
const CREDENTIAL_KEY_SEED: &[u8] = b"ridex-network-credentials-v1";
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkPlatform {
    #[default]
    Production,
    Test,
}

#[derive(Clone, Deserialize, Serialize)]
struct StoredCredentials {
    platform: NetworkPlatform,
    username: String,
    password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    pub saved: bool,
    pub username: Option<String>,
}

pub fn credential_status(
    app: &AppHandle,
    platform: NetworkPlatform,
) -> Result<CredentialStatus, String> {
    Ok(match load_credentials(app, platform)? {
        Some(credentials) => CredentialStatus {
            saved: true,
            username: Some(credentials.username),
        },
        None => CredentialStatus {
            saved: false,
            username: None,
        },
    })
}

fn resolve_credentials(
    app: &AppHandle,
    platform: NetworkPlatform,
    username: &str,
    password: &str,
) -> Result<(String, String, bool), String> {
    let username = username.trim();
    if username.is_empty() && password.is_empty() {
        let credentials =
            load_credentials(app, platform)?.ok_or_else(|| "请填写网络账号和密码".to_string())?;
        return Ok((credentials.username, credentials.password, false));
    }

    if username.is_empty() || password.is_empty() {
        return Err("网络账号和密码必须同时填写".to_string());
    }

    Ok((username.to_string(), password.to_string(), true))
}

fn credential_key() -> [u8; 32] {
    Sha256::digest(CREDENTIAL_KEY_SEED).into()
}

fn credential_url(app: &AppHandle) -> Result<tauri::Url, String> {
    let app_config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("获取app配置目录失败{error}"))?;
    std::fs::create_dir_all(&app_config_dir).map_err(|error| format!("创建目录失败{error}"))?;
    tauri::Url::from_file_path(app_config_dir.join("network.dat"))
        .map_err(|_| "文件名转url失败".to_string())
}

fn load_credentials(
    app: &AppHandle,
    platform: NetworkPlatform,
) -> Result<Option<StoredCredentials>, String> {
    let url = credential_url(app)?;
    let bytes = match app.fs().read(url) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("读取文件失败: {error}")),
    };
    let header = CREDENTIAL_MAGIC.len() + 12;
    if bytes.len() < header || &bytes[..CREDENTIAL_MAGIC.len()] != CREDENTIAL_MAGIC {
        return Err("网络凭据文件格式无效".to_string());
    }

    let nonce = Nonce::try_from(&bytes[CREDENTIAL_MAGIC.len()..header])
        .map_err(|_| "网络凭据 nonce 无效".to_string())?;
    let cipher = Aes256Gcm::new_from_slice(&credential_key())
        .map_err(|_| "网络凭据加密密钥无效".to_string())?;
    let plain = Zeroizing::new(
        cipher
            .decrypt(&nonce, &bytes[header..])
            .map_err(|_| "网络凭据解密失败".to_string())?,
    );
    let credentials: Vec<StoredCredentials> =
        serde_json::from_slice(&plain).map_err(|_| "网络凭据内容无效".to_string())?;
    Ok(credentials
        .into_iter()
        .find(|credentials| credentials.platform == platform))
}

fn save_credentials(app: &AppHandle, credentials: StoredCredentials) -> Result<(), String> {
    let other_platform = match credentials.platform {
        NetworkPlatform::Production => NetworkPlatform::Test,
        NetworkPlatform::Test => NetworkPlatform::Production,
    };
    let mut stored_credentials = load_credentials(app, other_platform)?
        .into_iter()
        .collect::<Vec<_>>();
    stored_credentials.push(credentials);
    let credentials = stored_credentials;

    let plain = Zeroizing::new(
        serde_json::to_vec(&credentials).map_err(|error| format!("编码网络凭据失败: {error}"))?,
    );
    let cipher = Aes256Gcm::new_from_slice(&credential_key())
        .map_err(|_| "网络凭据加密密钥无效".to_string())?;
    let nonce = Nonce::generate();
    let encrypted = cipher
        .encrypt(&nonce, plain.as_ref())
        .map_err(|_| "网络凭据加密失败".to_string())?;
    let mut bytes = Vec::with_capacity(CREDENTIAL_MAGIC.len() + nonce.len() + encrypted.len());
    bytes.extend_from_slice(CREDENTIAL_MAGIC);
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&encrypted);
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut file = app
        .fs()
        .open(credential_url(app)?, options)
        .map_err(|error| format!("打开网络凭据失败: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("写入网络凭据失败: {error}"))
}

/// 独立网络会话。会话本身不启动回复接收线程。
pub struct NetworkChannel {
    app: AppHandle,
    client: Client,
    name: String,
    platform: NetworkPlatform,
    token: Zeroizing<String>,
}

impl NetworkChannel {
    /// 登录网络设备，但不启动长期回复监听线程。
    pub async fn connect(
        app: &AppHandle,
        name: impl Into<String>,
        platform: NetworkPlatform,
        username: &str,
        password: &str,
        timeout_ms: u64,
    ) -> Result<Self, String> {
        let (username, password, save_credentials) =
            resolve_credentials(app, platform, username, password)?;
        let name = name.into().trim().to_string();
        if name.len() != 16 {
            return Err("网络设备名称必须为 16 个字符".to_string());
        }
        if username.trim().is_empty() || password.is_empty() {
            return Err("网络账号和密码不能为空".to_string());
        }
        if timeout_ms == 0 {
            return Err("网络超时时间必须大于零".to_string());
        }

        let client = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|error| error.to_string())?;
        let url = match platform {
            NetworkPlatform::Production => format!("{PRODUCTION_BASE_URL}/loginbyshangweiji"),
            NetworkPlatform::Test => format!("{TEST_BASE_URL}/loginbyshangweiji"),
        };
        let response = timeout(
            Duration::from_millis(timeout_ms),
            client
                .post(url)
                .json(&LoginRequest {
                    username: username.trim(),
                    password: &password,
                })
                .send(),
        )
        .await
        .map_err(|_| "网络响应超时".to_string())?
        .map_err(|error| format!("连接 Garow 服务失败: {error}"))?;
        let response = response
            .error_for_status()
            .map_err(|error| format!("Garow HTTP 请求失败: {error}"))?
            .json::<ApiResponse<LoginData>>()
            .await
            .map_err(|error| format!("Garow 响应解析失败: {error}"))?;
        if response.code != 0 {
            return Err(format!(
                "Garow API 错误 {}: {}",
                response.code, response.msg
            ));
        }
        let token = response
            .data
            .ok_or_else(|| "Garow API 成功响应缺少 data".to_string())?
            .token;

        let channel = Self {
            app: app.clone(),
            client,
            name,
            platform,
            token: Zeroizing::new(token),
        };
        if save_credentials {
            channel.authenticate(&username, &password)?;
        }
        Ok(channel)
    }

    /// 保存网络账号密码；密码只写入独立的加密凭据文件。
    pub fn authenticate(&self, username: &str, password: &str) -> Result<(), String> {
        let username = username.trim();
        if username.is_empty() || password.is_empty() {
            return Err("网络账号和密码不能为空".to_string());
        }
        save_credentials(
            &self.app,
            StoredCredentials {
                platform: self.platform,
                username: username.to_string(),
                password: password.to_string(),
            },
        )
    }

    /// 发送请求并直接轮询回复，收到有效完整帧或超时后结束本次事务。
    pub async fn modbus_tx(
        &self,
        request: &modbus::Protocol,
        timeout_ms: u64,
    ) -> Result<modbus::TransactionResult, String> {
        if timeout_ms == 0 {
            return Err("网络超时时间必须大于零".to_string());
        }

        let request_frame =
            modbus::assembly_frame(request.clone()).map_err(|error| error.to_string())?;
        let mut cursor = timeout(
            Duration::from_millis(timeout_ms),
            latest_reply(&self.client, self.platform, &self.token, &self.name),
        )
        .await
        .map_err(|_| "网络响应超时".to_string())?
        .map_err(|error| error)?
        .total;

        let command: String = request_frame
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect();
        let url = match self.platform {
            NetworkPlatform::Production => format!("{PRODUCTION_BASE_URL}/api/modbuscommand/send"),
            NetworkPlatform::Test => format!("{TEST_BASE_URL}/api/modbuscommand/send"),
        };
        timeout(
            Duration::from_millis(timeout_ms),
            self.client
                .post(url)
                .header("token", self.token.as_str())
                .json(&SendRequest {
                    bluetooth_name: &self.name,
                    modbus_command_str: command,
                })
                .send(),
        )
        .await
        .map_err(|_| "网络响应超时".to_string())?
        .map_err(|error| format!("发送网络指令失败: {error}"))?;

        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
                break;
            };

            let latest = timeout(
                remaining,
                latest_reply(&self.client, self.platform, &self.token, &self.name),
            )
            .await
            .map_err(|_| "网络响应超时".to_string())?
            .map_err(|error| error)?;

            if latest.total > cursor {
                let count = usize::try_from(latest.total - cursor)
                    .unwrap_or(usize::MAX)
                    .min(latest.messages.len());
                cursor = latest.total;
                for frame in latest.messages.iter().take(count).rev() {
                    match modbus::check_frame(&request_frame, frame) {
                        Ok(data) => {
                            return Ok(modbus::TransactionResult {
                                request: request_frame.clone(),
                                response: frame.clone(),
                                data,
                            });
                        }
                        Err(error @ modbus::FrameError::Exception { .. }) => {
                            return Err(error.to_string());
                        }
                        Err(_) => {}
                    }
                }
            }

            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
                break;
            };
            let sleep_ms = remaining.as_millis().min(POLL_INTERVAL_MS as u128) as u64;
            if sleep_ms == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }

        Err("网络响应超时".to_string())
    }
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    code: i32,
    msg: String,
    data: Option<T>,
}

#[derive(Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct LoginData {
    token: String,
}

#[derive(Deserialize)]
struct ReplyData {
    total: u64,
    list: Vec<ReplyItem>,
}

#[derive(Deserialize)]
struct ReplyItem {
    #[serde(rename = "dataStr")]
    data_str: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ReplyTotal {
    Number(u64),
    Text(String),
}

impl ReplyTotal {
    fn value(self) -> Result<u64, String> {
        match self {
            Self::Number(value) => Ok(value),
            Self::Text(value) => value
                .parse()
                .map_err(|_| format!("Garow 测试平台返回了无效消息总数: {value}")),
        }
    }
}

#[derive(Deserialize)]
struct TestReplyData {
    total: ReplyTotal,
    #[serde(rename = "pageData")]
    page_data: Vec<TestReplyItem>,
}

#[derive(Deserialize)]
struct TestReplyItem {
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TestReplyRequest<'a> {
    order: &'static str,
    order_field: &'static str,
    sn: &'a str,
    topic_name: &'static str,
    page_num: u8,
    page_size: u8,
}

struct LatestReply {
    total: u64,
    messages: Vec<Vec<u8>>,
}

async fn latest_reply(
    client: &Client,
    platform: NetworkPlatform,
    token: &str,
    name: &str,
) -> Result<LatestReply, String> {
    let (total, messages) = match platform {
        NetworkPlatform::Production => {
            let url = format!("{PRODUCTION_BASE_URL}/api/modbuscommand/getReplyMessageBySN");
            let response = client
                .get(url)
                .header("token", token)
                .query(&[("page", "1"), ("limit", "20"), ("bluetoothName", name)])
                .send()
                .await
                .map_err(|error| format!("查询网络回复失败: {error}"))?;
            let response = response
                .json::<ApiResponse<ReplyData>>()
                .await
                .map_err(|error| format!("Garow 响应解析失败: {error}"))?;
            let data = response
                .data
                .ok_or_else(|| "Garow API 成功响应缺少 data".to_string())?;
            (
                data.total,
                data.list
                    .into_iter()
                    .filter_map(|item| parse_reply_frame(&item.data_str).ok())
                    .collect::<Vec<_>>(),
            )
        }
        NetworkPlatform::Test => {
            let url = format!("{TEST_BASE_URL}/admin/original-data/page");
            let response = client
                .post(url)
                .header("token", token)
                .json(&TestReplyRequest {
                    order: "desc",
                    order_field: "create_date",
                    sn: name,
                    topic_name: "system/subconnection/retransmit_reply",
                    page_num: 1,
                    page_size: 20,
                })
                .send()
                .await
                .map_err(|error| format!("查询测试平台网络回复失败: {error}"))?;
            let response = response
                .json::<ApiResponse<TestReplyData>>()
                .await
                .map_err(|error| format!("Garow 响应解析失败: {error}"))?;
            let data = response
                .data
                .ok_or_else(|| "Garow API 成功响应缺少 data".to_string())?;
            (
                data.total.value()?,
                data.page_data
                    .into_iter()
                    .filter_map(|item| parse_reply_frame(&item.message).ok())
                    .collect::<Vec<_>>(),
            )
        }
    };

    Ok(LatestReply { total, messages })
}

#[derive(Deserialize)]
struct ReplyValue {
    value: String,
}

fn parse_reply_frame(data: &str) -> Result<Vec<u8>, String> {
    let value = serde_json::from_str::<ReplyValue>(data)
        .map(|reply| reply.value)
        .unwrap_or_else(|_| data.to_string());
    if value.eq_ignore_ascii_case("timeout") {
        return Err("服务器等待设备响应超时".to_string());
    }
    let compact: String = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    if compact.is_empty()
        || !compact.len().is_multiple_of(2)
        || !compact
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("网络回复不是有效 HEX 帧".to_string());
    }
    (0..compact.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|error| format!("网络回复 HEX 解析失败: {error}"))
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SendRequest<'a> {
    bluetooth_name: &'a str,
    modbus_command_str: String,
}
