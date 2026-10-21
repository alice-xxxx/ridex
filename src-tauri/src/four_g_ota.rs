//! 4G OTA 平台客户端。
//!
//! 本模块负责 `4gota接口文档.md` 中的 HTTP 调用，并通过项目已有的跨平台文件读取能力
//! 从固件路径读取上传内容。Token 仅保存在内存中，用户凭据加密后单独保存。

use std::{io::Write, sync::LazyLock, time::Duration};

use aes_gcm::{
    aead::{Aead, Generate, KeyInit},
    Aes256Gcm, Nonce,
};
use reqwest::{
    header::AUTHORIZATION,
    multipart::{Form, Part},
    Client, RequestBuilder,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FsExt, OpenOptions};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

const FILE_BASE_URL: &str = "https://kubeasy.garow.tech";
const OTA_BASE_URL: &str = "https://4g.garow.com";
const DEVICE_ID_HEADER: &str = "X-Kubeasy-Device-Id";
const DEFAULT_FILE_TYPE: &str = "common";
const OTA_MIME: &str = "application/octet-stream";
const TASK_PAGE_SIZE: u32 = 100;
const CREDENTIAL_MAGIC: &[u8; 8] = b"RIDEX4G1";
const CREDENTIAL_KEY_SEED: &[u8] = b"ridex-four-g-ota-credentials-v1";

static CLIENT: LazyLock<Mutex<Option<FourGOtaClient>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OtaEnvironment {
    Dev,
    Prod,
}

impl OtaEnvironment {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Prod => "prod",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadedFile {
    pub id: String,
    pub name: String,
    pub relative_path: String,
    pub url_path: String,
    pub public_url: String,
    pub file_type: String,
    pub mime: String,
    pub size: u64,
    pub converted_web_p: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtaTask {
    pub task_id: String,
    pub target_sn: String,
    pub firmware_url: String,
    pub ota_type: u32,
    #[serde(default)]
    pub patch: Option<String>,
    pub environment: String,
    pub status: String,
    pub total_count: u64,
    pub sent_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    #[serde(default)]
    pub interval_seconds: Option<u64>,
    #[serde(default)]
    pub last_serial_number: Option<u64>,
    pub waiting_reply: bool,
    #[serde(default)]
    pub last_reply_status: Option<String>,
    #[serde(default)]
    pub last_reply_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub next_run_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtaTaskPage {
    pub items: Vec<OtaTask>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub total_pages: u32,
    #[serde(default)]
    pub search: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartTaskCommand {
    pub target_sn: String,
    pub ota_type: u32,
    pub loop_count: u32,
    pub interval_seconds: u32,
    pub firmware_url: String,
    pub environment: OtaEnvironment,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginCommand {
    pub file_username: String,
    pub file_password: String,
    pub ota_username: String,
    pub ota_password: String,
}

#[derive(Deserialize, Serialize)]
struct StoredCredentials {
    file_username: String,
    file_password: String,
    ota_username: String,
    ota_password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    pub saved: bool,
    pub file_username: Option<String>,
    pub ota_username: Option<String>,
}

/// 两个平台共用一个 HTTP client，但分别保存各自的临时 Token。
pub struct FourGOtaClient {
    client: Client,
    device_id: String,
    file_token: Option<Zeroizing<String>>,
    ota_token: Option<Zeroizing<String>>,
}

impl FourGOtaClient {
    /// 创建客户端并立即登录文件平台和 OTA 平台。
    async fn new(
        device_id: impl Into<String>,
        timeout: Duration,
        credentials: &StoredCredentials,
    ) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| format!("创建 4G OTA HTTP 客户端失败: {error}"))?;

        let mut ota = Self {
            client,
            device_id: device_id.into(),
            file_token: None,
            ota_token: None,
        };

        ota.login_file_platform(&credentials.file_username, &credentials.file_password)
            .await?;
        ota.login_ota_platform(&credentials.ota_username, &credentials.ota_password)
            .await?;
        Ok(ota)
    }

    async fn login_file_platform(&mut self, username: &str, password: &str) -> Result<(), String> {
        let response: FileLoginResponse = send_json(
            self.client
                .post(format!("{FILE_BASE_URL}/api/auth/login"))
                .header(DEVICE_ID_HEADER, &self.device_id)
                .json(&LoginRequest { username, password }),
            "文件平台登录",
        )
        .await?;
        ensure_success(response.success, &response.message, "文件平台登录")?;
        self.file_token = Some(Zeroizing::new(response.data.token));
        Ok(())
    }

    pub async fn upload_firmware(
        &self,
        app: &AppHandle,
        firmware_path: &str,
    ) -> Result<UploadedFile, String> {
        let token = self
            .file_token
            .as_ref()
            .ok_or_else(|| "请先登录文件平台".to_string())?;
        let firmware = read_file(app, firmware_path)?;
        if firmware.is_empty() {
            return Err("OTA 文件内容为空".to_string());
        }
        let firmware_name = firmware_name_from_path(firmware_path);
        let file_part = Part::bytes(firmware)
            .file_name(firmware_name)
            .mime_str(OTA_MIME)
            .map_err(|error| format!("创建 OTA 文件上传表单失败: {error}"))?;
        let form = Form::new()
            .part("files", file_part)
            .text("fileType", DEFAULT_FILE_TYPE);

        let response: UploadResponse = send_json(
            self.client
                .post(format!("{FILE_BASE_URL}/api/business/files"))
                .header(AUTHORIZATION, bearer(token.as_str()))
                .header(DEVICE_ID_HEADER, &self.device_id)
                .multipart(form),
            "上传 OTA 文件",
        )
        .await?;
        ensure_success(response.success, &response.message, "上传 OTA 文件")?;

        response
            .data
            .files
            .into_iter()
            .next()
            .ok_or_else(|| "上传 OTA 文件成功，但响应中没有文件信息".to_string())
    }

    async fn login_ota_platform(&mut self, username: &str, password: &str) -> Result<(), String> {
        let response: OtaLoginResponse = send_json(
            self.client
                .post(format!("{OTA_BASE_URL}/api/auth/login"))
                .json(&LoginRequest { username, password }),
            "OTA 平台登录",
        )
        .await?;
        ensure_success(response.success, &response.message, "OTA 平台登录")?;
        self.ota_token = Some(Zeroizing::new(response.token));
        Ok(())
    }

    pub async fn switch_environment(&self, environment: OtaEnvironment) -> Result<(), String> {
        let response: SwitchEnvironmentResponse = send_json(
            self.ota_request("/api/auth/environment/switch")?
                .json(&SwitchEnvironmentRequest {
                    target_environment: environment.as_str(),
                }),
            "切换 OTA 环境",
        )
        .await?;
        ensure_success(response.success, &response.message, "切换 OTA 环境")?;

        if response.current_environment != environment.as_str() {
            return Err(format!(
                "OTA 环境切换结果不一致: 期望 {}，实际 {}",
                environment.as_str(),
                response.current_environment
            ));
        }
        Ok(())
    }

    pub async fn start_task(
        &self,
        target_sn: &str,
        ota_type: u32,
        loop_count: u32,
        interval_seconds: u32,
        firmware_url: &str,
    ) -> Result<OtaTask, String> {
        let response: OtaTaskResponse = send_json(
            self.ota_request("/api/ota/upgrade/single/loop/start")?
                .json(&StartTaskRequest {
                    target_sn,
                    ota_type,
                    loop_count,
                    interval_seconds,
                    firmware_url,
                }),
            "启动 4G OTA 任务",
        )
        .await?;
        ensure_ota_success(&response, "启动 4G OTA 任务")?;
        response
            .data
            .ok_or_else(|| "启动 4G OTA 任务成功，但响应中没有任务信息".to_string())
    }

    pub async fn list_tasks(&self, page: u32, page_size: u32) -> Result<OtaTaskPage, String> {
        if page == 0 || page_size == 0 {
            return Err("page 和 pageSize 必须大于 0".to_string());
        }

        let response: TaskListResponse = send_json(
            self.ota_get("/api/ota/upgrade/single/loop/tasks")?
                .query(&[("page", page), ("pageSize", page_size)]),
            "查询 4G OTA 任务",
        )
        .await?;
        ensure_success(response.success, &response.message, "查询 4G OTA 任务")?;
        response
            .data
            .ok_or_else(|| "查询 4G OTA 任务成功，但响应中没有任务列表".to_string())
    }

    pub async fn task(&self, task_id: &str) -> Result<OtaTask, String> {
        let mut page = 1;
        loop {
            let tasks = self.list_tasks(page, TASK_PAGE_SIZE).await?;
            if let Some(task) = tasks.items.into_iter().find(|task| task.task_id == task_id) {
                return Ok(task);
            }
            if page >= tasks.total_pages {
                return Err(format!("未找到 4G OTA 任务: {task_id}"));
            }
            page += 1;
        }
    }

    pub async fn stop_task(
        &self,
        task_id: &str,
        environment: OtaEnvironment,
    ) -> Result<OtaTask, String> {
        let path = format!(
            "/api/ota/upgrade/single/loop/{}/stop",
            encode_path_segment(task_id)
        );
        let response: OtaTaskResponse = send_json(
            self.ota_request(&path)?
                .query(&[("environment", environment.as_str())]),
            "停止 4G OTA 任务",
        )
        .await?;
        ensure_ota_success(&response, "停止 4G OTA 任务")?;
        response
            .data
            .ok_or_else(|| "停止 4G OTA 任务成功，但响应中没有任务信息".to_string())
    }

    fn ota_request(&self, path: &str) -> Result<RequestBuilder, String> {
        Ok(self
            .client
            .post(format!("{OTA_BASE_URL}{path}"))
            .header(AUTHORIZATION, self.ota_authorization()?))
    }

    fn ota_get(&self, path: &str) -> Result<RequestBuilder, String> {
        Ok(self
            .client
            .get(format!("{OTA_BASE_URL}{path}"))
            .header(AUTHORIZATION, self.ota_authorization()?))
    }

    fn ota_authorization(&self) -> Result<String, String> {
        self.ota_token
            .as_ref()
            .map(|token| bearer(token.as_str()))
            .ok_or_else(|| "请先登录 OTA 平台".to_string())
    }
}

#[tauri::command]
pub async fn four_g_ota_login(
    app: AppHandle,
    device_id: String,
    timeout_ms: u64,
    request: LoginCommand,
) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err("设备 ID 为空，无法保存 4G OTA 凭据".to_string());
    }
    let saved = load_credentials(&app, &device_id)?;
    let file = resolve_credential_pair(
        &request.file_username,
        &request.file_password,
        saved
            .as_ref()
            .map(|value| (value.file_username.as_str(), value.file_password.as_str())),
        "文件平台",
    )?;
    let ota = resolve_credential_pair(
        &request.ota_username,
        &request.ota_password,
        saved
            .as_ref()
            .map(|value| (value.ota_username.as_str(), value.ota_password.as_str())),
        "OTA 平台",
    )?;
    let credentials = StoredCredentials {
        file_username: file.0,
        file_password: file.1,
        ota_username: ota.0,
        ota_password: ota.1,
    };
    let client = FourGOtaClient::new(
        &device_id,
        Duration::from_millis(timeout_ms),
        &credentials,
    )
    .await?;
    save_credentials(&app, &device_id, &credentials)?;
    *CLIENT.lock().await = Some(client);
    Ok(())
}

#[tauri::command]
pub fn four_g_ota_credentials(
    app: AppHandle,
    device_id: String,
) -> Result<CredentialStatus, String> {
    let credentials = load_credentials(&app, &device_id)?;
    Ok(match credentials {
        Some(credentials) => CredentialStatus {
            saved: true,
            file_username: Some(credentials.file_username),
            ota_username: Some(credentials.ota_username),
        },
        None => CredentialStatus {
            saved: false,
            file_username: None,
            ota_username: None,
        },
    })
}

#[tauri::command]
pub async fn four_g_ota_upload(app: AppHandle, path: String) -> Result<UploadedFile, String> {
    CLIENT
        .lock()
        .await
        .as_ref()
        .ok_or_else(|| "请先登录 4G OTA 平台".to_string())?
        .upload_firmware(&app, &path)
        .await
}

#[tauri::command]
pub async fn four_g_ota_start(request: StartTaskCommand) -> Result<OtaTask, String> {
    let client = CLIENT.lock().await;
    let client = client
        .as_ref()
        .ok_or_else(|| "请先登录 4G OTA 平台".to_string())?;
    client.switch_environment(request.environment).await?;
    client
        .start_task(
            &request.target_sn,
            request.ota_type,
            request.loop_count,
            request.interval_seconds,
            &request.firmware_url,
        )
        .await
}

#[tauri::command]
pub async fn four_g_ota_progress(task_id: String) -> Result<OtaTask, String> {
    CLIENT
        .lock()
        .await
        .as_ref()
        .ok_or_else(|| "请先登录 4G OTA 平台".to_string())?
        .task(&task_id)
        .await
}

#[tauri::command]
pub async fn four_g_ota_stop(
    task_id: String,
    environment: OtaEnvironment,
) -> Result<OtaTask, String> {
    CLIENT
        .lock()
        .await
        .as_ref()
        .ok_or_else(|| "请先登录 4G OTA 平台".to_string())?
        .stop_task(&task_id, environment)
        .await
}

#[derive(Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct FileLoginResponse {
    success: bool,
    message: String,
    data: FileLoginData,
}

#[derive(Deserialize)]
struct FileLoginData {
    token: String,
}

#[derive(Deserialize)]
struct UploadResponse {
    success: bool,
    message: String,
    data: UploadData,
}

#[derive(Deserialize)]
struct UploadData {
    files: Vec<UploadedFile>,
}

#[derive(Deserialize)]
struct OtaLoginResponse {
    success: bool,
    message: String,
    token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SwitchEnvironmentRequest<'a> {
    target_environment: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchEnvironmentResponse {
    success: bool,
    message: String,
    current_environment: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartTaskRequest<'a> {
    target_sn: &'a str,
    ota_type: u32,
    loop_count: u32,
    interval_seconds: u32,
    firmware_url: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OtaTaskResponse {
    success: bool,
    message: String,
    #[serde(default)]
    error_message: Option<String>,
    #[serde(default)]
    data: Option<OtaTask>,
}

#[derive(Deserialize)]
struct TaskListResponse {
    success: bool,
    message: String,
    #[serde(default)]
    data: Option<OtaTaskPage>,
}

async fn send_json<T: DeserializeOwned>(
    request: RequestBuilder,
    operation: &str,
) -> Result<T, String> {
    let response = request
        .send()
        .await
        .map_err(|error| format!("{operation}请求失败: {error}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("{operation}读取响应失败: {error}"))?;

    if !status.is_success() {
        let message = response_message(&body);
        return Err(format!("{operation}失败，HTTP {status}: {message}"));
    }

    serde_json::from_slice(&body).map_err(|error| {
        format!(
            "{operation}响应解析失败: {error}; 响应: {}",
            response_message(&body)
        )
    })
}

fn ensure_success(success: bool, message: &str, operation: &str) -> Result<(), String> {
    if success {
        Ok(())
    } else {
        Err(format!("{operation}失败: {message}"))
    }
}

fn ensure_ota_success(response: &OtaTaskResponse, operation: &str) -> Result<(), String> {
    if response.success {
        return Ok(());
    }

    let message = response
        .error_message
        .as_deref()
        .filter(|message| !message.is_empty())
        .unwrap_or(&response.message);
    Err(format!("{operation}失败: {message}"))
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn resolve_credential_pair(
    username: &str,
    password: &str,
    saved: Option<(&str, &str)>,
    platform: &str,
) -> Result<(String, String), String> {
    match (username.trim(), password) {
        ("", _) => Err(format!("请输入{platform}账号")),
        (username, "") => match saved {
            Some((saved_username, saved_password)) if username == saved_username => {
                Ok((username.to_string(), saved_password.to_string()))
            }
            _ => Err(format!("请输入{platform}密码")),
        },
        (username, password) => Ok((username.to_string(), password.to_string())),
    }
}

fn credential_key(device_id: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CREDENTIAL_KEY_SEED);
    digest.update(device_id.as_bytes());
    digest.finalize().into()
}

fn credential_url(app: &AppHandle) -> Result<tauri::Url, String> {
    let app_config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&app_config_dir).map_err(|error| error.to_string())?;
    tauri::Url::from_file_path(app_config_dir.join("four-g-ota.dat"))
        .map_err(|_| "4G OTA 凭据文件路径无效".to_string())
}

fn load_credentials(app: &AppHandle, device_id: &str) -> Result<Option<StoredCredentials>, String> {
    let bytes = match app.fs().read(credential_url(app)?) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };
    let header_len = CREDENTIAL_MAGIC.len() + 12;
    if bytes.len() <= header_len || !bytes.starts_with(CREDENTIAL_MAGIC) {
        return Ok(None);
    }
    let nonce = match Nonce::try_from(&bytes[CREDENTIAL_MAGIC.len()..header_len]) {
        Ok(nonce) => nonce,
        Err(_) => return Ok(None),
    };
    let cipher = Aes256Gcm::new_from_slice(&credential_key(device_id))
        .map_err(|_| "4G OTA 凭据加密密钥无效".to_string())?;
    let plain = Zeroizing::new(match cipher.decrypt(&nonce, &bytes[header_len..]) {
        Ok(plain) => plain,
        Err(_) => return Ok(None),
    });
    Ok(serde_json::from_slice(&plain).ok())
}

fn save_credentials(
    app: &AppHandle,
    device_id: &str,
    credentials: &StoredCredentials,
) -> Result<(), String> {
    let plain = Zeroizing::new(serde_json::to_vec(credentials).map_err(|error| error.to_string())?);
    let nonce = Nonce::generate();
    let encrypted = Aes256Gcm::new_from_slice(&credential_key(device_id))
        .map_err(|_| "4G OTA 凭据加密密钥无效".to_string())?
        .encrypt(&nonce, plain.as_ref())
        .map_err(|error| format!("加密 4G OTA 凭据失败: {error}"))?;

    let mut bytes = Vec::with_capacity(CREDENTIAL_MAGIC.len() + nonce.len() + encrypted.len());
    bytes.extend_from_slice(CREDENTIAL_MAGIC);
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&encrypted);

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut file = app
        .fs()
        .open(credential_url(app)?, options)
        .map_err(|error| format!("保存 4G OTA 凭据失败: {error}"))?;
    file.set_len(0).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())
}

fn read_file(app: &AppHandle, path: &str) -> Result<Vec<u8>, String> {
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

fn firmware_name_from_path(path: &str) -> String {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let base_name = path
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("firmware.ota");
    base_name
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '"' | '\\' => '_',
            other => other,
        })
        .collect()
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn response_message(body: &[u8]) -> String {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ErrorBody {
        message: Option<String>,
        error_message: Option<String>,
    }

    serde_json::from_slice::<ErrorBody>(body)
        .ok()
        .and_then(|response| response.error_message.or(response.message))
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| String::from_utf8_lossy(body).chars().take(500).collect())
}

#[cfg(test)]
mod tests {
    use super::{encode_path_segment, firmware_name_from_path};

    #[test]
    fn gets_safe_file_name_from_path() {
        assert_eq!(
            firmware_name_from_path("C:\\ota\\bad\r\n\"name.ota?ignored"),
            "bad___name.ota"
        );
    }

    #[test]
    fn encodes_task_id_as_one_path_segment() {
        assert_eq!(encode_path_segment("task/a b"), "task%2Fa%20b");
    }
}
