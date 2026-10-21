//! 独立网络通信实现。
//!
//! 网络接口返回的已经是完整 Modbus 帧，不经过字节环形和帧捕获器。
//! 每次 modbus_tx 直接完成基线查询、发送请求和回复轮询。

use crate::modbus;

use std::{io::Write, time::Duration};

use aes_gcm::{
    aead::{Aead, Generate, KeyInit},
    Aes256Gcm, Nonce,
};
use reqwest::{Client, RequestBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FsExt, OpenOptions};
use zeroize::Zeroizing;

use tokio::time::timeout;

const POLL_INTERVAL_MS: u64 = 300;
const PRODUCTION_BASE_URL: &str = "https://iot.garow.com/admin";
const TEST_BASE_URL: &str = "https://dev-iot.garow.com/admin";
const LOGIN_PASSWORD_SALT: &str = "ba4d656d5a504e1fa43e2ad2b8791fc2";

const CREDENTIAL_MAGIC: &[u8; 8] = b"RIDEXN01";
const CREDENTIAL_KEY_SEED: &[u8] = b"ridex-network-credentials-v1";

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkPlatform {
    #[default]
    Production,
    Test,
}

impl NetworkPlatform {
    fn base_url(self) -> &'static str {
        match self {
            Self::Production => PRODUCTION_BASE_URL,
            Self::Test => TEST_BASE_URL,
        }
    }
    fn login_url(self) -> String {
        format!("{}/auth/login", self.base_url())
    }
    fn tx_url(self) -> String {
        format!("{}/vehicle-management/modbus/command/send", self.base_url())
    }
    fn rx_url(self) -> String {
        format!("{}/vehicle-management/modbus/replies/page", self.base_url())
    }
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
    if username.is_empty() {
        if password.is_empty() {
            let credentials = load_credentials(app, platform)?
                .ok_or_else(|| "网络账号和密码未输入".to_string())?;
            return Ok((credentials.username, credentials.password, false));
        } else {
            return Err("账号未输入".to_string());
        }
    } else {
        if password.is_empty() {
            return Err("密码未输入".to_string());
        } else {
            Ok((username.to_string(), password.to_string(), true))
        }
    }
}

fn credential_key() -> [u8; 32] {
    Sha256::digest(CREDENTIAL_KEY_SEED).into()
}

fn credential_url(app: &AppHandle) -> Result<tauri::Url, String> {
    let app_config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&app_config_dir).map_err(|error| error.to_string())?;
    tauri::Url::from_file_path(app_config_dir.join("network.dat"))
        .map_err(|_| "文件名转url失败".to_string())
}

fn load_credentials(
    app: &AppHandle,
    platform: NetworkPlatform,
) -> Result<Option<StoredCredentials>, String> {
    let bytes = match app.fs().read(credential_url(app)?) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };
    let header = CREDENTIAL_MAGIC.len() + 12;
    let nonce = match Nonce::try_from(&bytes[CREDENTIAL_MAGIC.len()..header]) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    };
    let cipher = Aes256Gcm::new_from_slice(&credential_key())
        .map_err(|_| "网络凭据加密密钥无效".to_string())?;
    let plain = Zeroizing::new(match cipher.decrypt(&nonce, &bytes[header..]) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    });
    let credentials: Vec<StoredCredentials> = match serde_json::from_slice(&plain) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    };
    Ok(credentials
        .into_iter()
        .find(|credentials| credentials.platform == platform))
}

fn save_credentials(app: &AppHandle, credentials: StoredCredentials) -> Result<(), String> {
    let other_platform = match credentials.platform {
        NetworkPlatform::Production => NetworkPlatform::Test,
        NetworkPlatform::Test => NetworkPlatform::Production,
    };
    let stored_credentials = load_credentials(app, other_platform)?
        .into_iter()
        .collect::<Vec<_>>()
        .push(credentials);

    let plain =
        Zeroizing::new(serde_json::to_vec(&stored_credentials).map_err(|error| error.to_string())?);
    let nonce = Nonce::generate();

    let encrypted = Aes256Gcm::new_from_slice(&credential_key())
        .map_err(|error| error.to_string())?
        .encrypt(&nonce, plain.as_ref())
        .map_err(|error| error.to_string())?;

    let mut bytes = Vec::with_capacity(CREDENTIAL_MAGIC.len() + nonce.len() + encrypted.len());
    bytes.extend_from_slice(CREDENTIAL_MAGIC);
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&encrypted);

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);

    let mut file = app
        .fs()
        .open(credential_url(app)?, options)
        .map_err(|error| error.to_string())?;
    file.set_len(0).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    Ok(())
}

//发送json解析
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest<'a> {
    auth_type: &'static str,
    account: &'a str,
    password: &'a str,
}
impl<'a> Default for LoginRequest<'a> {
    fn default() -> Self {
        Self {
            auth_type: "password",
            account: "",
            password: "",
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SendRequest<'a> {
    sn: &'a str,
    modbus_command_str: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplyRequest<'a> {
    page_num: u8,
    page_size: u8,
    sn: &'a str,
    topic_number: &'static str,
}
impl<'a> Default for ReplyRequest<'a> {
    fn default() -> Self {
        Self {
            page_num: 1,
            page_size: 20,
            sn: "",
            topic_number: "system/subconnection/retransmit_reply",
        }
    }
}
//回复json解析
#[derive(Deserialize)]
struct ApiResponse<T> {
    code: i32,
    msg: String,
    data: Option<T>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginData {
    x_token: String,
}

#[derive(Deserialize)]
struct ReplyData {
    total: String,
    #[serde(rename = "pageData")]
    page_data: Vec<ReplyItem>,
}
#[derive(Deserialize)]
struct ReplyItem {
    #[serde(rename = "dataStr")]
    data_str: String,
}

/// 独立网络会话。会话本身不启动回复接收线程。
pub struct NetworkChannel {
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
        let (username, password, should_save) =
            resolve_credentials(app, platform, username, password)?;

        let name = name.into().trim().to_string();
        let client = Client::new();
        let password_hash = login_password_hash(&password);

        let mut login_request = LoginRequest::default();
        login_request.password = &password_hash;
        login_request.account = &username;

        let token = api_request::<LoginData>(
            client.post(platform.login_url()).json(&login_request),
            Duration::from_millis(timeout_ms),
        )
        .await?
        .ok_or_else(|| "登录响应缺少 Token".to_string())?
        .x_token;

        if should_save {
            save_credentials(
                app,
                StoredCredentials {
                    platform,
                    username,
                    password,
                },
            )?;
        }

        Ok(Self {
            client,
            name,
            platform,
            token: Zeroizing::new(token),
        })
    }

    /// 发送请求并直接轮询回复，收到有效完整帧或超时后结束本次事务。
    pub async fn modbus_tx(
        &self,
        request: &modbus::Protocol,
        timeout_ms: u64,
    ) -> Result<modbus::TransactionResult, String> {
        let request_frame =
            modbus::assembly_frame(request.clone()).map_err(|error| error.to_string())?;

        let cursor = self.latest_reply(Duration::from_millis(1000)).await?.total;

        let command: String = request_frame
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ");

        api_request::<()>(
            self.client
                .post(self.platform.tx_url())
                .header("X-Token", self.token.as_str())
                .json(&SendRequest {
                    sn: &self.name,
                    modbus_command_str: command,
                }),
            Duration::from_millis(1000),
        )
        .await?;

        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        while let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) {
            let latest = self.latest_reply(remaining).await?;

            if latest.total > cursor {
                let count = (latest.total - cursor).min(latest.messages.len() as u64) as usize;
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

            tokio::time::sleep(remaining.min(Duration::from_millis(POLL_INTERVAL_MS))).await;
        }

        Err("车辆响应超时".to_string())
    }

    async fn latest_reply(&self, request_timeout: Duration) -> Result<LatestReply, String> {
        let mut reply_request = ReplyRequest::default();
        reply_request.sn = &self.name;

        let data = api_request::<ReplyData>(
            self.client
                .post(self.platform.rx_url())
                .header("X-Token", self.token.as_str())
                .json(&reply_request),
            request_timeout,
        )
        .await?
        .ok_or_else(|| "回复查询缺少数据".to_string())?;

        Ok(LatestReply {
            total: data.total.parse().map_err(|_| format!("total解析失败"))?,
            messages: data
                .page_data
                .into_iter()
                .filter_map(|item| parse_reply_frame(&item.data_str).ok())
                .collect(),
        })
    }
}

async fn api_request<T: DeserializeOwned>(
    request: RequestBuilder,
    request_timeout: Duration,
) -> Result<Option<T>, String> {
    let response = timeout(request_timeout, async {
        let response = request.send().await.map_err(|error| error.to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("response status: {status}"));
        }
        let body = response.bytes().await.map_err(|error| error.to_string())?;
        serde_json::from_slice::<ApiResponse<T>>(&body).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    match response.code {
        0 => Ok(response.data),
        code => Err(format!("业务状态码错误 {code}: {}", response.msg)),
    }
}

fn login_password_hash(password: &str) -> String {
    let first: String = format!(
        "{:x}",
        md5::compute(format!("{LOGIN_PASSWORD_SALT}{password}"))
    );
    format!("{:x}", md5::compute(first))
}

struct LatestReply {
    total: u64,
    messages: Vec<Vec<u8>>,
}
#[derive(Deserialize)]
struct ReplyValue {
    value: String,
}
fn parse_reply_frame(data: &str) -> Result<Vec<u8>, String> {
    let value = serde_json::from_str::<ReplyValue>(data)
        .map_err(|error| error.to_string())?
        .value;
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16).map_err(|error| error.to_string())
        })
        .collect()
}
