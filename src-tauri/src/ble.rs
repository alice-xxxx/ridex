//! BLE 通信独立实现。
//!
//! 真实数据流的处理顺序固定为：
//!
//! BLE 通知密文 -> AES 解密 -> 字节环形缓冲 -> buffer_get_frame
//! -> 完整帧覆盖队列 -> check_frame。

use crate::modbus;

use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use aes::cipher::{Block, BlockModeDecrypt, BlockModeEncrypt, KeyInit};
use aes::Aes128;
use crossbeam_queue::ArrayQueue;
use ecb::{Decryptor, Encryptor};
use rand::RngExt;
use ringbuf::{traits::*, HeapRb};
use serde::Serialize;
use tauri_plugin_blec::{
    models::{ScanFilter, WriteType},
    OnDisconnectHandler,
};
use tokio::{
    sync::{broadcast, mpsc},
    time::{timeout, Instant},
};
use uuid::Uuid;
use zeroize::Zeroizing;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x0000fee7_0000_1000_8000_00805f9b34fb);
const WRITE_UUID: Uuid = Uuid::from_u128(0x000036f5_0000_1000_8000_00805f9b34fb);
const NOTIFY_UUID: Uuid = Uuid::from_u128(0x000036f6_0000_1000_8000_00805f9b34fb);

const AUTH_KEY: [u8; 16] = [
    0xAB, 0x0E, 0x05, 0x5F, 0x71, 0x0A, 0xC6, 0xE8, 0x05, 0xB6, 0x30, 0x9C, 0xCD, 0xEE, 0x3B, 0x5A,
];

const BYTE_RING_CAPACITY: usize = 8 * 1024;
const FRAME_RING_CAPACITY: usize = 64;
const AUTH_RESPONSE_LENGTH: usize = 8;

/// BLE 扫描结果。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub address: String,
    pub rssi: Option<i16>,
}

/// BLE 独立实现的错误类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BleError {
    InvalidArgument(&'static str),
    Timeout,
    NotConnected,
    AddressMismatch,
    Disconnected(String),
    NotAuthenticated,
    AuthenticationRequired(&'static str),
    Transport(String),
    Protocol(String),
}

impl fmt::Display for BleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(formatter, "参数无效：{message}"),
            Self::Timeout => write!(formatter, "操作超时"),
            Self::NotConnected => write!(formatter, "BLE 尚未连接"),
            Self::AddressMismatch => write!(formatter, "设备地址与当前连接不一致"),
            Self::Disconnected(message) => write!(formatter, "BLE 已断开：{message}"),
            Self::NotAuthenticated => write!(formatter, "BLE 尚未认证"),
            Self::AuthenticationRequired(message) => write!(formatter, "{message}"),
            Self::Transport(message) => write!(formatter, "BLE 传输失败：{message}"),
            Self::Protocol(message) => write!(formatter, "协议处理失败：{message}"),
        }
    }
}

impl std::error::Error for BleError {}

impl From<modbus::FrameError> for BleError {
    fn from(error: modbus::FrameError) -> Self {
        Self::Protocol(error.to_string())
    }
}

/// 独立 BLE 会话。
///
/// 这里直接调用 tauri_plugin_blec，不通过现有 Channel、Global 或前端状态。
/// 认证成功后才启动通知解析线程；断开时停止线程并释放线程资源。
pub struct BleChannel {
    address: String,
    name: String,
    notifications: broadcast::Sender<Vec<u8>>,
    connected: Arc<AtomicBool>,
    key: Option<Zeroizing<[u8; 16]>>,

    parser_stop: Arc<AtomicBool>,
    parser_task: Option<JoinHandle<()>>,
    last_decrypted: Arc<ArrayQueue<Vec<u8>>>,

    // 每个槽位是一条完整帧，固定容量，满时覆盖最旧完整帧，不使用互斥锁。
    frame_ring: Arc<ArrayQueue<Vec<u8>>>,
}

impl BleChannel {
    /// 扫描设备、去重、删除无名称设备，并按信号强度排序。
    pub async fn discover(
        timeout_ms: u64,
        service_uuid: Option<&str>,
    ) -> Result<Vec<DeviceInfo>, BleError> {
        if !(500..=10_000).contains(&timeout_ms) {
            return Err(BleError::InvalidArgument(
                "蓝牙扫描时间必须在 500..=10000 ms 之间",
            ));
        }

        let filter = match service_uuid {
            Some(value) => ScanFilter::Service(
                Uuid::parse_str(value).map_err(|_| BleError::InvalidArgument("服务 UUID 无效"))?,
            ),
            None => ScanFilter::None,
        };

        let (sender, mut receiver) = mpsc::channel(100);
        handler()?
            .discover(Some(sender), timeout_ms, filter, false)
            .await
            .map_err(plugin_error)?;

        let mut devices = HashMap::new();
        let collect = async {
            while let Some(batch) = receiver.recv().await {
                for device in batch {
                    devices.insert(device.address.clone(), device);
                }
            }
        };

        let _ = timeout(
            Duration::from_millis(timeout_ms.saturating_add(1_500)),
            collect,
        )
        .await;

        let mut devices: Vec<_> = devices.into_values().collect();
        devices.sort_by(|left, right| {
            right
                .rssi
                .unwrap_or(i16::MIN)
                .cmp(&left.rssi.unwrap_or(i16::MIN))
                .then_with(|| left.address.cmp(&right.address))
        });

        Ok(devices
            .into_iter()
            .filter_map(|device| {
                if device.name.trim().is_empty() || device.address.trim().is_empty() {
                    return None;
                }
                Some(DeviceInfo {
                    name: device.name,
                    address: device.address,
                    rssi: device.rssi,
                })
            })
            .collect())
    }

    /// 连接设备并订阅通知。
    pub async fn connect(
        address: impl Into<String>,
        name: impl Into<String>,
        timeout_ms: u64,
    ) -> Result<Self, BleError> {
        let address = address.into();
        if address.trim().is_empty() {
            return Err(BleError::InvalidArgument("蓝牙地址不能为空"));
        }
        if timeout_ms == 0 {
            return Err(BleError::InvalidArgument("连接超时时间必须大于零"));
        }

        let connected = Arc::new(AtomicBool::new(true));
        let disconnected = Arc::clone(&connected);
        let on_disconnect = OnDisconnectHandler::from_async(move || {
            disconnected.store(false, Ordering::Release);
            async {}
        });

        match timeout(
            Duration::from_millis(timeout_ms),
            handler()?.connect(&address, on_disconnect, false),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(plugin_error(error)),
            Err(_) => return Err(BleError::Timeout),
        }

        let (notifications, _) = broadcast::channel(64);
        let notify = notifications.clone();
        if let Err(error) = handler()?
            .subscribe(NOTIFY_UUID, Some(SERVICE_UUID), move |bytes| {
                let _ = notify.send(bytes);
            })
            .await
        {
            let _ = handler()?.disconnect().await;
            return Err(plugin_error(error));
        }

        let name = name.into();
        let name = if name.trim().is_empty() {
            address.clone()
        } else {
            name
        };
        Ok(Self {
            address,
            name,
            notifications,
            connected,
            key: None,
            parser_stop: Arc::new(AtomicBool::new(false)),
            parser_task: None,
            last_decrypted: Arc::new(ArrayQueue::new(1)),
            frame_ring: Arc::new(ArrayQueue::new(FRAME_RING_CAPACITY)),
        })
    }

    /// 断开当前设备并清理认证状态、解析任务和全部缓冲区。
    pub async fn disconnect(&mut self, address: &str) -> Result<(), BleError> {
        if self.address != address {
            return Err(BleError::AddressMismatch);
        }

        self.connected.store(false, Ordering::Release);
        self.stop_notification_task();
        self.key = None;

        let result = match timeout(Duration::from_secs(6), handler()?.disconnect()).await {
            Ok(Ok(())) | Ok(Err(tauri_plugin_blec::Error::NoDeviceConnected)) => Ok(()),
            Ok(Err(error)) => Err(plugin_error(error)),
            Err(_) => Err(BleError::Timeout),
        };

        result
    }

    /// 使用车辆编号和 BLE 名称组装认证包，等待并校验 8 字节认证响应。
    pub async fn authenticate(
        &mut self,
        vehicle_code: &str,
        timeout_ms: u64,
    ) -> Result<(), BleError> {
        self.ensure_connected()?;
        if timeout_ms == 0 {
            return Err(BleError::InvalidArgument("认证超时时间必须大于零"));
        }

        self.stop_notification_task();
        self.key = None;
        self.reset_frame_ring();

        let mut receiver = self.notifications.subscribe();
        let (request, session_key) = auth_request(vehicle_code, &self.name);
        handler()?
            .send_data(
                WRITE_UUID,
                Some(SERVICE_UUID),
                &request,
                WriteType::WithoutResponse,
            )
            .await
            .map_err(plugin_error)?;

        let response = receive_auth_response(&mut receiver, &session_key, timeout_ms).await?;
        check_auth_result(&response)?;

        self.key = Some(Zeroizing::new(session_key));
        self.start_notification_task()?;
        Ok(())
    }

    /// 组帧、加密、发送，并消费通知解析任务放入的第一条完整响应帧。
    pub async fn modbus_tx(
        &self,
        request: &modbus::Protocol,
        timeout_ms: u64,
    ) -> Result<modbus::TransactionResult, BleError> {
        self.ensure_connected()?;
        let key = self
            .key
            .as_ref()
            .map(|value| **value)
            .ok_or(BleError::NotAuthenticated)?;
        if timeout_ms == 0 {
            return Err(BleError::InvalidArgument("Modbus 超时时间必须大于零"));
        }
        let result = self.modbus_tx_inner(request, &key, timeout_ms).await;
        result
    }

    async fn modbus_tx_inner(
        &self,
        request: &modbus::Protocol,
        key: &[u8; 16],
        timeout_ms: u64,
    ) -> Result<modbus::TransactionResult, BleError> {
        let request_frame = modbus::assembly_frame(request.clone())?;
        let encrypted = encrypt(&request_frame, key);

        // 组帧和加密完成后，紧挨着 BLE 写入清除旧的完整帧，
        // 防止本次请求消费发送前已经留在帧队列中的旧响应。
        while self.last_decrypted.pop().is_some() {}
        while self.frame_ring.pop().is_some() {}
        handler()?
            .send_data(
                WRITE_UUID,
                Some(SERVICE_UUID),
                &encrypted,
                WriteType::WithoutResponse,
            )
            .await
            .map_err(plugin_error)?;

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let Some(response) = self.pop_frame_until(deadline).await? else {
            return match self.last_decrypted.pop() {
                Some(data) => Err(BleError::Protocol(format!(
                    "Modbus 解析器校验数据异常，收到的数据为：{}",
                    format_bytes(&data),
                ))),
                None => Err(BleError::Timeout),
            };
        };

        // buffer_get_frame 已经完成帧边界捕获；这里只做一次 Modbus 业务校验。
        let data = modbus::check_frame(&request_frame, &response)?;
        Ok(modbus::TransactionResult {
            request: request_frame,
            response,
            data,
        })
    }

    fn ensure_connected(&self) -> Result<(), BleError> {
        if self.connected.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(BleError::NotConnected)
        }
    }

    fn start_notification_task(&mut self) -> Result<(), BleError> {
        let key = self
            .key
            .as_ref()
            .map(|value| **value)
            .ok_or(BleError::NotAuthenticated)?;

        self.parser_stop.store(false, Ordering::Release);
        let receiver = self.notifications.subscribe();
        let stop = Arc::clone(&self.parser_stop);
        let connected = Arc::clone(&self.connected);
        let last_decrypted = Arc::clone(&self.last_decrypted);
        let frame_ring = Arc::clone(&self.frame_ring);

        self.parser_task = Some(thread::spawn(move || {
            notification_task(receiver, key, stop, connected, last_decrypted, frame_ring);
        }));
        Ok(())
    }

    fn stop_notification_task(&mut self) {
        self.parser_stop.store(true, Ordering::Release);
        if let Some(task) = self.parser_task.take() {
            let _ = task.join();
        }
    }

    fn reset_frame_ring(&mut self) {
        self.frame_ring = Arc::new(ArrayQueue::new(FRAME_RING_CAPACITY));
    }

    /// 从完整帧环形队列取一条帧，直到取得帧或等待超时。
    async fn pop_frame_until(&self, deadline: Instant) -> Result<Option<Vec<u8>>, BleError> {
        loop {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(None);
            };

            if let Some(frame) = self.frame_ring.pop() {
                return Ok(Some(frame));
            }

            let poll_interval = Duration::from_millis(10);
            let sleep_duration = if remaining < poll_interval {
                remaining
            } else {
                poll_interval
            };
            tokio::time::sleep(sleep_duration).await;
        }
    }
}

/// 通知解析线程。
///
/// 注意顺序：
///
/// 1. 从 BLE 通知通道轮询取得一片密文；
/// 2. 直接解密当前通知数据并移除 AES 填充；
/// 3. 把解密字节追加到字节环形缓冲；
/// 4. 调用一次 modbus.rs 的 buffer_get_frame；
/// 5. 如果捕获到完整帧，就放入固定容量的无锁覆盖队列；
/// 6. 本轮处理完成后短暂休眠，再执行下一轮。
fn notification_task(
    mut receiver: broadcast::Receiver<Vec<u8>>,
    key: [u8; 16],
    stop: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    last_decrypted: Arc<ArrayQueue<Vec<u8>>>,
    frame_ring: Arc<ArrayQueue<Vec<u8>>>,
) {
    // 字节环形只由当前通知线程访问，整个接收过程按顺序执行。
    let mut byte_ring = HeapRb::<u8>::new(BYTE_RING_CAPACITY);

    while !stop.load(Ordering::Acquire) && connected.load(Ordering::Acquire) {
        let data = match receiver.try_recv() {
            Ok(data) => data,
            Err(broadcast::error::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(broadcast::error::TryRecvError::Lagged(_))
            | Err(broadcast::error::TryRecvError::Closed) => break,
        };

        if data.is_empty() {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        let decrypted = match decrypt(&data, &key) {
            Ok(decrypted) => decrypted,
            Err(_) => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        };
        let decrypted = strip_aes_padding(decrypted);
        if !decrypted.is_empty() {
            let _ = last_decrypted.force_push(decrypted.clone());
        }
        byte_ring.push_slice_overwrite(&decrypted);

        // 直接把字节环形交给协议模块；帧捕获和未消费字节的保留由协议模块负责。
        if let Some(frame) = modbus::buffer_get_frame(&mut byte_ring) {
            // 完整帧环形只保存帧对象；满时覆盖最旧帧。
            let _ = frame_ring.force_push(frame);
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 认证阶段不走 Modbus 帧捕获器，因为认证响应是固定 8 字节状态帧。
/// 这里只负责凑齐 AES 块、解密并取出前 8 字节，随后由 check_auth_result 校验。
async fn receive_auth_response(
    receiver: &mut broadcast::Receiver<Vec<u8>>,
    key: &[u8; 16],
    timeout_ms: u64,
) -> Result<Vec<u8>, BleError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut encrypted = Vec::new();
    let mut decrypted = Vec::new();

    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err(BleError::Timeout);
        };

        let chunk = match timeout(remaining, receiver.recv()).await {
            Ok(Ok(chunk)) => chunk,
            Ok(Err(broadcast::error::RecvError::Lagged(count))) => {
                return Err(BleError::Disconnected(format!(
                    "认证通知丢失 {count} 个数据片段"
                )));
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                return Err(BleError::Disconnected("认证通知通道已关闭".to_string()));
            }
            Err(_) => return Err(BleError::Timeout),
        };

        encrypted.extend_from_slice(&chunk);
        let complete = encrypted.len() / 16 * 16;
        if complete == 0 {
            continue;
        }

        decrypted.extend(decrypt(&encrypted[..complete], key)?);
        encrypted.drain(..complete);

        if decrypted.len() >= AUTH_RESPONSE_LENGTH {
            return Ok(decrypted.drain(..AUTH_RESPONSE_LENGTH).collect());
        }
    }
}

/// 根据车辆编号和 BLE 名称生成认证密钥及认证请求。
fn auth_request(vehicle_code: &str, ble_name: &str) -> (Vec<u8>, [u8; 16]) {
    let mut random = [0u8; 4];
    rand::rng().fill(&mut random);

    let mut vehicle = [0xFF; 12];
    if !vehicle_code.trim().is_empty() {
        vehicle.fill(0);
        for (index, character) in vehicle_code.trim().chars().take(12).enumerate() {
            vehicle[index] = character.to_digit(10).map_or(0, |digit| digit as u8);
        }
    }

    let key = auth_key(ble_name);
    let mut packet = [0u8; 16];
    packet[..4].copy_from_slice(&random);
    packet[4..].copy_from_slice(&vehicle);

    let mut session_key = key;
    for (index, byte) in session_key.iter_mut().enumerate() {
        *byte = byte.wrapping_add(random[index % 4]);
    }

    (encrypt(&packet, &key), session_key)
}

fn auth_key(ble_name: &str) -> [u8; 16] {
    let mut key = AUTH_KEY;
    for (index, byte) in ble_name.bytes().enumerate() {
        key[index % 16] = key[index % 16].wrapping_add(byte);
    }
    key
}

/// 校验认证响应 CRC 和状态字节。
fn check_auth_result(data: &[u8]) -> Result<(), BleError> {
    if data.len() < AUTH_RESPONSE_LENGTH {
        return Err(BleError::Protocol("认证响应长度不足".to_string()));
    }

    let actual = u16::from_le_bytes([data[6], data[7]]);
    let expected = modbus::crc16_modbus(&data[..6]);
    if actual != expected {
        return Err(BleError::Protocol("认证响应 CRC 校验失败".to_string()));
    }

    match data[2] {
        1 => Ok(()),
        2 => Err(BleError::AuthenticationRequired("认证失败，请先启动车辆")),
        3 => Err(BleError::AuthenticationRequired(
            "认证失败，请点击一键启动按钮",
        )),
        status => Err(BleError::Protocol(format!("未知认证状态 {status}"))),
    }
}

/// 按项目现有 BLE AES-128 ECB 规则加密。
fn encrypt(data: &[u8], key: &[u8; 16]) -> Vec<u8> {
    let padding = (16 - data.len() % 16) % 16;
    let mut padded = data.to_vec();
    if padding > 0 {
        padded.resize(data.len() + padding, padding as u8);
    }

    let mut output = Vec::with_capacity(padded.len());
    let mut cipher = Encryptor::<Aes128>::new(key.into());
    for bytes in padded.chunks(16) {
        let mut block = Block::<Aes128>::try_from(bytes).expect("AES block length");
        cipher.encrypt_block(&mut block);
        output.extend_from_slice(&block);
    }
    output
}

/// 按项目现有 BLE AES-128 ECB 规则解密。
fn decrypt(data: &[u8], key: &[u8; 16]) -> Result<Vec<u8>, BleError> {
    if data.is_empty() || !data.len().is_multiple_of(16) {
        return Err(BleError::Protocol("AES 密文长度无效".to_string()));
    }

    let mut output = Vec::with_capacity(data.len());
    let mut cipher = Decryptor::<Aes128>::new(key.into());
    for bytes in data.chunks(16) {
        let mut block = Block::<Aes128>::try_from(bytes).expect("AES block length");
        cipher.decrypt_block(&mut block);
        output.extend_from_slice(&block);
    }
    Ok(output)
}

fn strip_aes_padding(mut data: Vec<u8>) -> Vec<u8> {
    let Some(&padding) = data.last() else {
        return data;
    };
    let padding = padding as usize;
    if !(1..16).contains(&padding)
        || data.len() < padding
        || !data[data.len() - padding..]
            .iter()
            .all(|byte| usize::from(*byte) == padding)
    {
        return data;
    }

    data.truncate(data.len() - padding);
    data
}

fn handler() -> Result<&'static tauri_plugin_blec::Handler, BleError> {
    tauri_plugin_blec::get_handler()
        .map_err(|_| BleError::Disconnected("请先确保蓝牙功能开启".to_string()))
}

fn plugin_error(error: tauri_plugin_blec::Error) -> BleError {
    BleError::Transport(format!("{error:?}"))
}
