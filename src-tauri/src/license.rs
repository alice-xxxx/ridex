use std::{
    error::Error as StdError,
    io::{ErrorKind, Write},
    time::Duration,
};

use aes_gcm::{
    aead::{Aead, Generate, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Manager;
use tauri_plugin_fs::{FsExt, OpenOptions};
use tauri_plugin_machine_uid::MachineUidExt;
use zeroize::Zeroizing;

mod compiled {
    include!(concat!(env!("OUT_DIR"), "/auth_config.rs"));
}

const VERSION: u8 = 1;
const ISSUER: &str = "ridex-auth";
const AUDIENCE: &str = "ridex-app";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Ticket {
    pub version: u8,
    pub issuer: String,
    pub audience: String,
    pub device_id: String,
    pub nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    #[serde(rename = "features")]
    pub _features: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    code: String,
    ticket: String,
}

#[derive(Deserialize)]
struct Rejection {
    message: Option<String>,
}

fn verify_ticket(
    encoded: &str,
    device_id: &str,
    expected_nonce: Option<&str>,
) -> Result<Ticket, String> {
    let (payload, signature) = encoded
        .split_once('.')
        .filter(|(_, signature)| !signature.contains('.'))
        .ok_or("授权票据格式无效")?;
    let key = VerifyingKey::from_sec1_bytes(&decode::<65>(compiled::PUBLIC_KEY)?)
        .map_err(|_| "授权公钥无效")?;
    key.verify(
        payload.as_bytes(),
        &Signature::from_slice(&decode::<64>(signature)?).map_err(|_| "授权票据签名无效")?,
    )
    .map_err(|_| "授权票据签名无效")?;
    let ticket: Ticket = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| "授权票据内容无效")?,
    )
    .map_err(|_| "授权票据内容无效")?;
    let now = chrono::Utc::now().timestamp();
    if ticket.version != VERSION
        || ticket.issuer != ISSUER
        || ticket.audience != AUDIENCE
        || ticket.device_id != device_id
        || expected_nonce.is_some_and(|nonce| ticket.nonce != nonce)
        || ticket.issued_at > now + 300
        || ticket.expires_at <= now
    {
        return Err("授权票据已过期或不适用于本设备".into());
    }
    Ok(ticket)
}

fn decode<const N: usize>(value: &str) -> Result<[u8; N], String> {
    URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| "Base64URL 数据无效")?
        .try_into()
        .map_err(|_| "数据长度无效".into())
}

fn path(app: &tauri::AppHandle) -> Result<tauri::Url, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("failed to create license directory: {error}"))?;
    let file = directory.join("license.dat");
    tauri::Url::from_file_path(file).map_err(|_| "授权文件路径无效".to_string())
}
#[derive(Debug)]
enum LocalTicketError {
    Missing,
    Failed(String),
}

impl LocalTicketError {
    fn into_message(self) -> String {
        match self {
            Self::Missing => "本地授权文件不存在".to_string(),
            Self::Failed(message) => message,
        }
    }
}

fn is_missing_file(error: &(dyn StdError + 'static)) -> bool {
    let mut source = Some(error);
    while let Some(current) = source {
        if let Some(io_error) = current.downcast_ref::<std::io::Error>() {
            let raw_os_error = io_error.raw_os_error();
            if io_error.kind() == ErrorKind::NotFound
                || raw_os_error == Some(2)
                || (cfg!(windows) && raw_os_error == Some(3))
            {
                return true;
            }
        }
        source = current.source();
    }

    let message = error.to_string().to_ascii_lowercase();
    message.contains("enoent")
        || message.contains("no such file")
        || message.contains("file not found")
        || message.contains("cannot find the path")
        || message.contains("os error 2")
        || (cfg!(windows) && message.contains("os error 3"))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct License {
    pub(crate) device_id: String,
    pub(crate) authorized: bool,
    pub(crate) ready: bool,
    pub(crate) message: Option<String>,
}

impl License {
    pub(crate) fn init(app: &tauri::AppHandle) -> Result<Self, String> {
        let device_id = app
            .machine_uid()
            .get_machine_uid()
            .map_err(|error| error.to_string())?
            .id
            .ok_or_else(|| "missing machine id".to_string())?;
        Ok(Self {
            device_id,
            authorized: false,
            ready: false,
            message: None,
        })
    }

    pub(crate) fn unavailable(error: String) -> Self {
        let message = format!("启动授权失败: {error}");
        Self {
            device_id: format!("设备 ID 不可用: {error}"),
            authorized: false,
            ready: true,
            message: Some(message),
        }
    }

    pub(crate) fn ensure_authorized(&self) -> Result<(), String> {
        if self.authorized {
            return Ok(());
        }

        let message = self
            .message
            .clone()
            .unwrap_or_else(|| "应用尚未完成授权".to_string());
        if message.contains("设备 ID:") {
            Err(message)
        } else {
            Err(format!("{message}; 设备 ID: {}", self.device_id))
        }
    }

    pub(crate) async fn authorize(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct AuthorizationRequest {
            version: u8,
            device_id: String,
            nonce: String,
            app_version: String,
            platform: String,
        }

        enum OnlineFailure {
            Network,
            Rejected {
                message: String,
                invalidate_cache: bool,
            },
            InvalidResponse(String),
        }

        const TICKET_MAGIC: &[u8; 8] = b"RIDEXT01";

        let exit_with = |message: String| -> Result<(), String> {
            app.exit(1);
            Err(message)
        };

        let online = async {
            let mut nonce = [0u8; 32];
            rand::rng().fill(&mut nonce);
            let request = AuthorizationRequest {
                version: VERSION,
                device_id: self.device_id.clone(),
                nonce: URL_SAFE_NO_PAD.encode(nonce),
                app_version: env!("CARGO_PKG_VERSION").to_string(),
                platform: std::env::consts::OS.to_string(),
            };

            let client = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|_| OnlineFailure::Network)?;
            let response = client
                .post(format!(
                    "{}/v1/app/authorize",
                    compiled::URL.trim_end_matches('/')
                ))
                .json(&request)
                .send()
                .await
                .map_err(|_| OnlineFailure::Network)?;

            if !response.status().is_success() {
                let status = response.status();
                let message = response
                    .json::<Rejection>()
                    .await
                    .ok()
                    .and_then(|body| body.message)
                    .unwrap_or_else(|| format!("服务器拒绝授权 ({status})"));
                if status.is_server_error() {
                    return Err(OnlineFailure::Network);
                }
                // Only the auth server's explicit device-denied response invalidates
                // the cache. Transport, proxy, protocol, and rate-limit errors do not.
                return Err(OnlineFailure::Rejected {
                    invalidate_cache: status == reqwest::StatusCode::FORBIDDEN
                        && message == "device denied",
                    message,
                });
            }

            let response = response.json::<Response>().await.map_err(|error| {
                OnlineFailure::InvalidResponse(format!("授权响应无效: {error}"))
            })?;
            if response.code != "authorized" {
                return Err(OnlineFailure::InvalidResponse(
                    "服务器返回未授权响应".into(),
                ));
            }

            Ok::<(String, String), OnlineFailure>((response.ticket, request.nonce))
        }
        .await;

        // Only derive the local storage key after the authorization request
        // has completed. The server-signed ticket is the value stored locally.
        let mut derived_key = [0u8; 32];
        derived_key.copy_from_slice(
            Sha256::digest(format!("ridex-license-ticket-v1:{}", self.device_id).as_bytes())
                .as_slice(),
        );
        let derived_key = Zeroizing::new(derived_key);

        let write_ticket = |ticket: &str| -> Result<(), String> {
            let file_path = path(app)?;

            let cipher = Aes256Gcm::new_from_slice(derived_key.as_ref())
                .map_err(|_| "本地授权密钥无效".to_string())?;
            let nonce = Nonce::generate();
            let plain = Zeroizing::new(ticket.as_bytes().to_vec());
            let encrypted = cipher
                .encrypt(&nonce, plain.as_ref())
                .map_err(|_| "本地授权加密失败".to_string())?;

            let mut bytes = Vec::with_capacity(TICKET_MAGIC.len() + nonce.len() + encrypted.len());
            bytes.extend_from_slice(TICKET_MAGIC);
            bytes.extend_from_slice(&nonce);
            bytes.extend_from_slice(&encrypted);

            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);
            let mut file = app
                .fs()
                .open(file_path, options)
                .map_err(|error| format!("打开授权文件失败: {error}"))?;
            file.write_all(&bytes)
                .map_err(|error| format!("写入授权文件失败: {error}"))?;
            file.flush()
                .map_err(|error| format!("保存授权文件失败: {error}"))
        };

        let read_ticket = || -> Result<String, LocalTicketError> {
            let file_path = path(app).map_err(LocalTicketError::Failed)?;
            let bytes = app.fs().read(file_path).map_err(|error| {
                if is_missing_file(&error) {
                    LocalTicketError::Missing
                } else {
                    LocalTicketError::Failed(format!("读取本地授权失败: {error}"))
                }
            })?;

            let header = TICKET_MAGIC.len() + 12;
            if bytes.len() < header || &bytes[..TICKET_MAGIC.len()] != TICKET_MAGIC {
                return Err(LocalTicketError::Failed("本地授权文件格式无效".into()));
            }

            let cipher = Aes256Gcm::new_from_slice(derived_key.as_ref())
                .map_err(|_| LocalTicketError::Failed("本地授权密钥无效".to_string()))?;
            let nonce = Nonce::try_from(&bytes[TICKET_MAGIC.len()..header])
                .map_err(|_| LocalTicketError::Failed("本地授权 nonce 无效".to_string()))?;
            let plain = Zeroizing::new(
                cipher
                    .decrypt(&nonce, &bytes[header..])
                    .map_err(|_| LocalTicketError::Failed("本地授权解密失败".to_string()))?,
            );
            String::from_utf8(plain.to_vec())
                .map_err(|_| LocalTicketError::Failed("本地授权内容无效".to_string()))
        };

        let clear_ticket = || -> Result<(), String> {
            let file_path = path(app)?;

            let mut options = OpenOptions::new();
            options.write(true).truncate(true);
            match app.fs().open(file_path, options) {
                Ok(mut file) => file
                    .flush()
                    .map_err(|error| format!("清空本地授权失败: {error}")),
                Err(error) if is_missing_file(&error) => Ok(()),
                Err(error) => Err(format!("清空本地授权失败: {error}")),
            }
        };

        let result = match online {
            Ok((ticket, nonce)) => (|| -> Result<(), String> {
                write_ticket(&ticket)?;
                let cached = read_ticket().map_err(LocalTicketError::into_message)?;
                verify_ticket(&cached, &self.device_id, Some(&nonce))
                    .map(|_| ())
                    .map_err(|error| format!("本地授权校验失败: {error}"))
            })(),
            Err(OnlineFailure::Rejected {
                message,
                invalidate_cache,
            }) if invalidate_cache => {
                let error = match clear_ticket() {
                    Ok(()) => message,
                    Err(clear_error) => format!("{message}; {clear_error}"),
                };
                exit_with(error)
            }
            Err(OnlineFailure::Rejected { message, .. }) => Err(message),
            Err(OnlineFailure::InvalidResponse(error)) => Err(error),
            Err(OnlineFailure::Network) => match read_ticket() {
                Ok(ticket) => verify_ticket(&ticket, &self.device_id, None)
                    .map(|_| ())
                    .map_err(|cache_error| format!("离线授权无效: {cache_error}")),
                Err(LocalTicketError::Missing) => {
                    Err("没有可用的本地授权: 本地授权文件不存在".to_string())
                }
                Err(LocalTicketError::Failed(cache_error)) => {
                    Err(format!("没有可用的本地授权: {cache_error}"))
                }
            },
        };

        match result {
            Ok(()) => {
                self.authorized = true;
                self.ready = true;
                self.message = None;
                Ok(())
            }
            Err(error) => {
                let error = format!("{error}; 设备 ID: {}", self.device_id);
                self.authorized = false;
                self.ready = true;
                self.message = Some(error.clone());
                Err(error)
            }
        }
    }
}
