//! 独立串口通信实现。
//!
//! 串口接收线程的固定处理顺序为：
//!
//! 串口读取 -> 写入字节环形 -> buffer_get_frame -> 完整帧覆盖队列。

use crate::modbus;

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_queue::ArrayQueue;
use ringbuf::{traits::*, HeapRb};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_serialplugin::{
    api::SerialPort,
    state::{ClearBuffer, DataBits, FlowControl, Parity, StopBits},
};

const READ_CHUNK_SIZE: usize = 256;
const READ_POLL_MS: u64 = 10;
const FRAME_RING_CAPACITY: usize = 64;
const BYTE_RING_CAPACITY: usize = 8 * 1024;

/// 串口独立实现的错误类型。
#[derive(Debug)]
pub enum SerialError {
    InvalidArgument(&'static str),
    Timeout,
    Disconnected(String),
    Protocol(String),
    Internal(String),
}

impl fmt::Display for SerialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(formatter, "参数无效：{message}"),
            Self::Timeout => write!(formatter, "串口操作超时"),
            Self::Disconnected(message) => write!(formatter, "串口已断开：{message}"),
            Self::Protocol(message) => write!(formatter, "协议处理失败：{message}"),
            Self::Internal(message) => write!(formatter, "内部错误：{message}"),
        }
    }
}

impl std::error::Error for SerialError {}

impl From<modbus::FrameError> for SerialError {
    fn from(error: modbus::FrameError) -> Self {
        Self::Protocol(error.to_string())
    }
}

/// 独立串口会话。
pub struct SerialChannel {
    app: AppHandle,
    path: String,
    receiver_stop: Arc<AtomicBool>,
    receiver_thread: Option<JoinHandle<()>>,
    frame_ring: Arc<ArrayQueue<Vec<u8>>>,
}

impl SerialChannel {
    /// Enumerate available serial ports and return `(path, display_name)`.
    pub fn discover(app: &AppHandle) -> Result<Vec<(String, String)>, SerialError> {
        let ports = app
            .state::<SerialPort<Wry>>()
            .available_ports(true)
            .map_err(|error| SerialError::Internal(format!("读取串口列表失败: {error}")))?;

        Ok(ports
            .into_iter()
            .map(|(path, info)| {
                let name = info
                    .get("product")
                    .filter(|name| !name.is_empty() && name.as_str() != "Unknown")
                    .cloned()
                    .unwrap_or_else(|| path.clone());
                (path, name)
            })
            .collect())
    }

    /// 打开串口并启动独立接收线程。
    pub async fn connect(
        app: AppHandle,
        path: impl Into<String>,
        baud_rate: u32,
    ) -> Result<Self, SerialError> {
        let path = path.into();
        if path.trim().is_empty() || path.len() > 256 {
            return Err(SerialError::InvalidArgument("串口路径长度无效"));
        }
        if !(300..=3_000_000).contains(&baud_rate) {
            return Err(SerialError::InvalidArgument(
                "串口波特率必须在 300..=3000000 之间",
            ));
        }

        let port = app.state::<SerialPort<Wry>>().inner().clone();
        let open_port = port.clone();
        let clear_port = port.clone();
        let open_path = path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let opened = open_port
                .open(
                    open_path,
                    baud_rate,
                    Some(DataBits::Eight),
                    Some(FlowControl::None),
                    Some(Parity::None),
                    Some(StopBits::One),
                    Some(1_000),
                )
                .map_err(|error| SerialError::Disconnected(error.to_string()))?;
            clear_port
                .clear_buffer(opened, ClearBuffer::Input)
                .map_err(|error| SerialError::Internal(error.to_string()))
        })
        .await
        .map_err(|error| SerialError::Internal(error.to_string()))??;

        let mut channel = Self {
            app,
            path,
            receiver_stop: Arc::new(AtomicBool::new(false)),
            receiver_thread: None,
            frame_ring: Arc::new(ArrayQueue::new(FRAME_RING_CAPACITY)),
        };
        channel.start_receiver_thread(port);
        Ok(channel)
    }

    /// 停止接收线程并关闭串口。
    pub async fn disconnect(&mut self) -> Result<(), SerialError> {
        self.stop_receiver_thread();

        let port = self.app.state::<SerialPort<Wry>>().inner().clone();
        let path = self.path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            port.close(path)
                .map_err(|error| SerialError::Disconnected(error.to_string()))
        })
        .await
        .map_err(|error| SerialError::Internal(error.to_string()))??;
        Ok(())
    }

    /// 组帧、发送，并在指定毫秒数内轮询完整帧队列。
    pub async fn modbus_tx(
        &self,
        request: &modbus::Protocol,
        timeout_ms: u64,
    ) -> Result<modbus::TransactionResult, SerialError> {
        if timeout_ms == 0 {
            return Err(SerialError::InvalidArgument("Modbus 超时时间必须大于零"));
        }

        let request_frame = modbus::assembly_frame(request.clone())?;

        // 串口写入前直接消费队列中的旧完整帧，避免本次请求误取旧响应。
        while self.frame_ring.pop().is_some() {}

        let port = self.app.state::<SerialPort<Wry>>().inner().clone();
        let path = self.path.clone();
        let write_frame = request_frame.clone();
        tauri::async_runtime::spawn_blocking(move || {
            port.write_binary(path, write_frame)
                .map_err(|error| SerialError::Disconnected(format!("串口发送失败：{error}")))
        })
        .await
        .map_err(|error| SerialError::Internal(error.to_string()))??;

        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        let response = loop {
            if let Some(frame) = self.frame_ring.pop() {
                break frame;
            }

            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
                return Err(SerialError::Timeout);
            };
            let sleep_ms = remaining.as_millis().min(READ_POLL_MS as u128) as u64;
            if sleep_ms == 0 {
                return Err(SerialError::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        };

        let data = modbus::check_frame(&request_frame, &response)?;
        Ok(modbus::TransactionResult {
            request: request_frame,
            response,
            data,
        })
    }

    fn start_receiver_thread(&mut self, port: SerialPort<Wry>) {
        self.receiver_stop.store(false, Ordering::Release);
        let path = self.path.clone();
        let stop = Arc::clone(&self.receiver_stop);
        let frame_ring = Arc::clone(&self.frame_ring);
        self.receiver_thread = Some(thread::spawn(move || {
            receiver_thread(port, path, stop, frame_ring);
        }));
    }

    fn stop_receiver_thread(&mut self) {
        self.receiver_stop.store(true, Ordering::Release);
        if let Some(thread) = self.receiver_thread.take() {
            let _ = thread.join();
        }
    }
}

/// 串口接收线程：每轮读取一次数据，写入字节环形，持续捕获当前已有的全部完整帧后休眠。
fn receiver_thread(
    port: SerialPort<Wry>,
    path: String,
    stop: Arc<AtomicBool>,
    frame_ring: Arc<ArrayQueue<Vec<u8>>>,
) {
    let mut byte_ring = HeapRb::<u8>::new(BYTE_RING_CAPACITY);

    while !stop.load(Ordering::Acquire) {
        let data = match port.read_binary(path.clone(), Some(READ_POLL_MS), Some(READ_CHUNK_SIZE)) {
            Ok(data) => data,
            Err(error) => {
                let message = error.to_string();
                if message.contains("no data received within") {
                    continue;
                }
                tauri_plugin_log::log::error!("[SER][RX] port={path} read failed: {message}");
                break;
            }
        };

        if !data.is_empty() {
            byte_ring.push_slice_overwrite(&data);
            while let Some(frame) = modbus::buffer_get_frame(&mut byte_ring) {
                let _ = frame_ring.force_push(frame);
            }
        }

        thread::sleep(Duration::from_millis(READ_POLL_MS));
    }
}
