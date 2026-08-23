use std::fmt;

use ringbuf::{traits::*, HeapRb};
use serde::{Deserialize, Serialize};

pub const READ_COILS: u8 = 0x01;
pub const READ_REGISTERS: u8 = 0x03;
pub const WRITE_COIL: u8 = 0x05;
pub const WRITE_REGISTER: u8 = 0x06;
pub const WRITE_COILS: u8 = 0x0F;
pub const WRITE_REGISTERS: u8 = 0x10;
pub const OTA_HANDSHAKE: u8 = 0xF0;
pub const OTA_DATA: u8 = 0xF1;
pub const OTA_RESULT: u8 = 0xF2;
pub const OTA_PACKET_SIZE: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
/// 帧处理过程中可能出现的错误。
pub enum FrameError {
    InvalidArgument(&'static str),
    RequestTooShort,
    ResponseTooShort,
    AddressMismatch,
    FunctionMismatch,
    InvalidLength,
    InvalidData,
    CrcMismatch,
    Exception {
        function: u8,
        code: u8,
        response: Vec<u8>,
    },
    OtaResult {
        function: u8,
        code: u16,
        response: Vec<u8>,
    },
}

fn format_frame_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(formatter, "参数无效：{message}"),
            Self::RequestTooShort => write!(formatter, "请求帧长度不足"),
            Self::ResponseTooShort => write!(formatter, "响应帧长度不足"),
            Self::AddressMismatch => write!(formatter, "响应设备地址与请求不一致"),
            Self::FunctionMismatch => write!(formatter, "响应功能码与请求不一致"),
            Self::InvalidLength => write!(formatter, "响应帧长度无效"),
            Self::InvalidData => write!(formatter, "响应数据无效"),
            Self::CrcMismatch => write!(formatter, "响应 CRC 校验失败"),
            Self::Exception {
                function,
                code,
                response,
            } => {
                write!(
                    formatter,
                    "Modbus 异常响应：功能码=0x{function:02X}，异常码={code}，响应数据={}",
                    format_frame_bytes(response),
                )
            }
            Self::OtaResult {
                function,
                code,
                response,
            } => {
                write!(
                    formatter,
                    "OTA 异常响应：功能码=0x{function:02X}，结果码={code}，响应数据={}",
                    format_frame_bytes(response),
                )
            }
        }
    }
}

impl std::error::Error for FrameError {}

pub type FrameResult<T> = Result<T, FrameError>;

/// 一次 Modbus 事务的完整结果。
///
/// `request` 是实际发送的完整帧，`response` 是收到的完整原始帧，
/// `data` 是通过协议校验后提取出的业务数据。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResult {
    pub request: Vec<u8>,
    pub response: Vec<u8>,
    pub data: Vec<u8>,
}

/// 计算 Modbus 通用 CRC16。
///
/// Modbus 在线路上按低字节在前发送 CRC 结果，具体追加工作由 append_crc 完成。
pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc = 0xFFFF;
    for byte in data {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn append_crc(frame: &mut Vec<u8>) {
    frame.extend_from_slice(&crc16_modbus(frame).to_le_bytes());
}

fn has_valid_crc(frame: &[u8]) -> bool {
    let Some(body_end) = frame.len().checked_sub(2) else {
        return false;
    };
    let Some((&low, &high)) = frame.get(body_end).zip(frame.get(body_end + 1)) else {
        return false;
    };
    u16::from_le_bytes([low, high]) == crc16_modbus(&frame[..body_end])
}

/// 根据响应功能码和 byte-count 计算完整帧长度。
///
/// 读响应的长度由响应自身携带的 byte-count 决定，不能使用请求帧的长度
/// 或请求数量推测。返回 None 表示功能码不属于本原型支持的协议。
pub fn frame_length(function: u8, byte_count: Option<u8>) -> Option<usize> {
    match function {
        READ_COILS => byte_count
            .filter(|count| (1..=250).contains(count))
            .map(|count| usize::from(count) + 5),
        READ_REGISTERS => byte_count
            .filter(|count| (2..=250).contains(count) && count.is_multiple_of(2))
            .map(|count| usize::from(count) + 5),
        WRITE_COIL | WRITE_REGISTER | WRITE_COILS | WRITE_REGISTERS => Some(8),
        OTA_HANDSHAKE | OTA_RESULT => Some(6),
        OTA_DATA => Some(8),
        function if is_exception(function) => Some(5),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LengthState {
    Ready(usize),
    Waiting,
    Invalid,
}

fn candidate_length(function: u8, byte_count: Option<u8>) -> LengthState {
    if matches!(function, READ_COILS | READ_REGISTERS) && byte_count.is_none() {
        return LengthState::Waiting;
    }
    if frame_length(function, byte_count).is_some() {
        LengthState::Ready(frame_length(function, byte_count).unwrap_or(0))
    } else {
        LengthState::Invalid
    }
}

fn is_base_function(function: u8) -> bool {
    matches!(
        function,
        READ_COILS | READ_REGISTERS | WRITE_COIL | WRITE_REGISTER | WRITE_COILS | WRITE_REGISTERS
    )
}

fn is_exception(function: u8) -> bool {
    function & 0x80 != 0 && is_base_function(function & 0x7F)
}

fn is_candidate_function(function: u8) -> bool {
    is_base_function(function)
        || matches!(function, OTA_HANDSHAKE | OTA_DATA | OTA_RESULT)
        || is_exception(function)
}

/// 从字节环形缓冲中查找并消费一帧有效协议帧。
///
/// 环形缓冲由调用方持续写入，本函数负责把环中的字节交给协议捕获器，
/// 并把未消费的字节放回同一个环。缓冲区可能包含噪声、半帧、一帧或连续多帧；
/// 成功捕获后只消费当前帧，后续帧字节继续留在环中。
pub fn buffer_get_frame(buffer: &mut HeapRb<u8>) -> Option<Vec<u8>> {
    if buffer.occupied_len() < 5 {
        return None;
    }

    loop {
        let available = buffer.occupied_len();
        let Some(start) = buffer
            .iter()
            .skip(1)
            .take(available - 1)
            .position(|function| is_candidate_function(*function))
        else {
            // 保留最后一个字节，因为它可能是下一次读取中功能码的前置地址。
            buffer.skip(available - 1);
            return None;
        };
        if start > 0 {
            buffer.skip(start);
        }

        let function = buffer.iter().nth(1).copied()?;
        let byte_count = buffer.iter().nth(2).copied();
        let state = candidate_length(function, byte_count);
        let length = match state {
            LengthState::Ready(length) => length,
            LengthState::Waiting => return None,
            LengthState::Invalid => {
                buffer.skip(1);
                continue;
            }
        };
        if buffer.occupied_len() < length {
            return None;
        }
        if !has_valid_ring_crc(buffer, length) {
            // 当前候选帧 CRC 错误，向前移动一个字节继续搜索，避免错误数据
            // 阻塞后续有效帧的捕获。
            buffer.skip(1);
            continue;
        }

        let frame = buffer.iter().take(length).copied().collect();
        buffer.skip(length);
        return Some(frame);
    }
}

/// 直接按环形缓冲中的字节计算候选帧 CRC，不复制整个缓冲区。
fn has_valid_ring_crc(buffer: &HeapRb<u8>, length: usize) -> bool {
    if length < 2 || buffer.occupied_len() < length {
        return false;
    }

    let mut crc = 0xFFFF;
    for byte in buffer.iter().take(length - 2) {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }

    let mut crc_bytes = buffer.iter().skip(length - 2).take(2);
    let Some(low) = crc_bytes.next().copied() else {
        return false;
    };
    let Some(high) = crc_bytes.next().copied() else {
        return false;
    };
    u16::from_le_bytes([low, high]) == crc
}

/// 检查帧的最小长度、功能码对应长度和 CRC。
fn ensure_frame_shape(frame: &[u8]) -> FrameResult<()> {
    if frame.len() < 5 {
        return Err(FrameError::ResponseTooShort);
    }
    let length = candidate_length(frame[1], frame.get(2).copied());
    let LengthState::Ready(expected) = length else {
        return Err(FrameError::InvalidLength);
    };
    if frame.len() != expected {
        return Err(FrameError::InvalidLength);
    }
    if !has_valid_crc(frame) {
        return Err(FrameError::CrcMismatch);
    }
    Ok(())
}

/// 从请求帧的地址字段中读取 Modbus 数量。
fn read_count(frame: &[u8]) -> FrameResult<u16> {
    frame
        .get(4..6)
        .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
        .ok_or(FrameError::RequestTooShort)
}

/// 校验写操作响应中的回显字段。
fn compare_echo(tx_frame: &[u8], rx_frame: &[u8]) -> FrameResult<()> {
    if tx_frame.get(..6) != rx_frame.get(..6) {
        return Err(FrameError::InvalidData);
    }
    Ok(())
}

/// 校验 OTA 响应中的结果码。
fn ota_result(rx_frame: &[u8], function: u8) -> FrameResult<()> {
    let code = u16::from_be_bytes([rx_frame[2], rx_frame[3]]);
    if code == 0 {
        Ok(())
    } else {
        Err(FrameError::OtaResult {
            function,
            code,
            response: rx_frame.to_vec(),
        })
    }
}

/// 根据请求帧校验一条完整响应，并返回业务数据。
///
/// 读操作返回响应中的数据区；写操作和 OTA 成功响应只是确认帧，因此返回
/// 空数据。异常响应返回对应的功能码和异常码。
pub fn check_frame(tx_frame: &[u8], rx_frame: &[u8]) -> FrameResult<Vec<u8>> {
    if tx_frame.len() < 2 {
        return Err(FrameError::RequestTooShort);
    }
    ensure_frame_shape(rx_frame)?;
    if tx_frame[0] != rx_frame[0] {
        return Err(FrameError::AddressMismatch);
    }

    let tx_function = tx_frame[1];
    let rx_function = rx_frame[1];
    if is_exception(rx_function) {
        return Err(FrameError::Exception {
            function: rx_function & 0x7F,
            code: rx_frame[2],
            response: rx_frame.to_vec(),
        });
    }
    if rx_function != tx_function {
        return Err(FrameError::FunctionMismatch);
    }

    match tx_function {
        READ_COILS | READ_REGISTERS => {
            let count = usize::from(read_count(tx_frame)?);
            let expected = if tx_function == READ_COILS {
                count.div_ceil(8)
            } else {
                count.checked_mul(2).ok_or(FrameError::InvalidLength)?
            };
            if usize::from(rx_frame[2]) != expected || rx_frame.len() - 5 != expected {
                return Err(FrameError::InvalidData);
            }
            Ok(rx_frame[3..rx_frame.len() - 2].to_vec())
        }
        WRITE_COIL | WRITE_REGISTER => {
            compare_echo(tx_frame, rx_frame)?;
            Ok(rx_frame[4..6].to_vec())
        }
        WRITE_COILS | WRITE_REGISTERS => {
            compare_echo(tx_frame, rx_frame)?;
            Ok(Vec::new())
        }
        OTA_HANDSHAKE | OTA_RESULT => {
            if tx_function == OTA_HANDSHAKE && rx_frame.len() == 6 {
                ota_result(rx_frame, tx_function)?;
                Ok(Vec::new())
            } else if tx_function == OTA_RESULT && rx_frame.len() == 6 {
                ota_result(rx_frame, tx_function)?;
                Ok(Vec::new())
            } else {
                Err(FrameError::InvalidLength)
            }
        }
        OTA_DATA => {
            if tx_frame.len() < 4 || rx_frame.len() != 8 {
                return Err(FrameError::InvalidLength);
            }
            if tx_frame[2..4] != rx_frame[2..4] {
                return Err(FrameError::InvalidData);
            }
            let code = u16::from_be_bytes([rx_frame[4], rx_frame[5]]);
            if code == 0 {
                Ok(Vec::new())
            } else {
                Err(FrameError::OtaResult {
                    function: tx_function,
                    code,
                    response: rx_frame.to_vec(),
                })
            }
        }
        _ => Err(FrameError::FunctionMismatch),
    }
}

/// 待组装的协议请求。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Protocol {
    /// 读线圈（功能码 0x01）。
    ReadCoils { device: u8, start: u16, count: u16 },
    /// 读保持寄存器（功能码 0x03）。
    ReadRegisters { device: u8, start: u16, count: u16 },
    /// 写一个或多个线圈；一个值使用 0x05，多个值使用 0x0F。
    WriteCoils {
        device: u8,
        start: u16,
        values: Vec<bool>,
    },
    /// 写一个或多个寄存器；一个值使用 0x06，多个值使用 0x10。
    WriteRegisters {
        device: u8,
        start: u16,
        values: Vec<u16>,
    },
    /// OTA 握手请求，字段按 OTA 协议要求使用大端序。
    OtaHandshake {
        device: u8,
        firmware_size: u32,
        manufacturer: [u8; 4],
        hardware_version: [u8; 2],
        software_version: [u8; 2],
        firmware_crc: u32,
    },
    /// OTA 数据分包，数据不足 128 字节时使用 0xFF 填充。
    OtaData {
        device: u8,
        sequence: u16,
        data: Vec<u8>,
    },
    /// OTA 传输结果确认请求。
    OtaResult { device: u8 },
}

/// Tauri 命令允许的普通 Modbus 请求。
///
/// OTA 帧只能由 OTA 流程内部生成，因此不放进命令输入类型，避免前端直接
/// 构造未经 OTA 流程控制的握手、分包和结果帧。
fn validate_range(start: u16, count: usize, maximum: usize) -> FrameResult<()> {
    // 先校验数量，再校验起始地址加数量后的末地址，防止地址回绕。
    if count == 0 || count > maximum {
        return Err(FrameError::InvalidArgument("数量超出范围"));
    }
    let count = u16::try_from(count).map_err(|_| FrameError::InvalidArgument("数量过大"))?;
    start
        .checked_add(count - 1)
        .ok_or(FrameError::InvalidArgument("地址范围超出 u16"))?;
    Ok(())
}

fn frame(device: u8, function: u8, body: &[u8]) -> Vec<u8> {
    // 所有请求和响应都使用统一的地址、功能码、数据区、CRC 组帧顺序。
    let mut result = Vec::with_capacity(body.len() + 4);
    result.extend_from_slice(&[device, function]);
    result.extend_from_slice(body);
    append_crc(&mut result);
    result
}

/// 根据协议请求组装一条完整发送帧。
///
/// 多值写入自动选择 0x0F 或 0x10；单值写入使用紧凑的 0x05 或 0x06。
pub fn assembly_frame(protocol: Protocol) -> FrameResult<Vec<u8>> {
    match protocol {
        Protocol::ReadCoils {
            device,
            start,
            count,
        } => {
            validate_range(start, usize::from(count), 2000)?;
            Ok(frame(
                device,
                READ_COILS,
                &[
                    (start >> 8) as u8,
                    start as u8,
                    (count >> 8) as u8,
                    count as u8,
                ],
            ))
        }
        Protocol::ReadRegisters {
            device,
            start,
            count,
        } => {
            validate_range(start, usize::from(count), 125)?;
            Ok(frame(
                device,
                READ_REGISTERS,
                &[
                    (start >> 8) as u8,
                    start as u8,
                    (count >> 8) as u8,
                    count as u8,
                ],
            ))
        }
        Protocol::WriteCoils {
            device,
            start,
            values,
        } => {
            validate_range(start, values.len(), 1968)?;
            if values.len() == 1 {
                return Ok(frame(
                    device,
                    WRITE_COIL,
                    &[
                        (start >> 8) as u8,
                        start as u8,
                        if values[0] { 0xFF } else { 0 },
                        0,
                    ],
                ));
            }
            let packed_len = values.len().div_ceil(8);
            let mut body = vec![
                (start >> 8) as u8,
                start as u8,
                (values.len() >> 8) as u8,
                values.len() as u8,
                packed_len as u8,
            ];
            body.resize(body.len() + packed_len, 0);
            for (index, value) in values.iter().enumerate() {
                if *value {
                    body[5 + index / 8] |= 1 << (index % 8);
                }
            }
            Ok(frame(device, WRITE_COILS, &body))
        }
        Protocol::WriteRegisters {
            device,
            start,
            values,
        } => {
            validate_range(start, values.len(), 123)?;
            if values.len() == 1 {
                return Ok(frame(
                    device,
                    WRITE_REGISTER,
                    &[
                        (start >> 8) as u8,
                        start as u8,
                        (values[0] >> 8) as u8,
                        values[0] as u8,
                    ],
                ));
            }
            let byte_count = values
                .len()
                .checked_mul(2)
                .ok_or(FrameError::InvalidArgument("寄存器数据过大"))?;
            let mut body = vec![
                (start >> 8) as u8,
                start as u8,
                (values.len() >> 8) as u8,
                values.len() as u8,
                byte_count as u8,
            ];
            for value in values {
                body.extend_from_slice(&value.to_be_bytes());
            }
            Ok(frame(device, WRITE_REGISTERS, &body))
        }
        Protocol::OtaHandshake {
            device,
            firmware_size,
            manufacturer,
            hardware_version,
            software_version,
            firmware_crc,
        } => {
            let mut body = Vec::with_capacity(16);
            body.extend_from_slice(&firmware_size.to_be_bytes());
            body.extend_from_slice(&manufacturer);
            body.extend_from_slice(&hardware_version);
            body.extend_from_slice(&software_version);
            body.extend_from_slice(&firmware_crc.to_be_bytes());
            Ok(frame(device, OTA_HANDSHAKE, &body))
        }
        Protocol::OtaData {
            device,
            sequence,
            data,
        } => {
            if data.len() > OTA_PACKET_SIZE {
                return Err(FrameError::InvalidArgument("OTA 数据包不能超过 128 字节"));
            }
            let mut body = Vec::with_capacity(2 + OTA_PACKET_SIZE);
            body.extend_from_slice(&sequence.to_be_bytes());
            body.resize(body.len() + OTA_PACKET_SIZE, 0xFF);
            body[2..2 + data.len()].copy_from_slice(&data);
            Ok(frame(device, OTA_DATA, &body))
        }
        Protocol::OtaResult { device } => Ok(frame(device, OTA_RESULT, &[])),
    }
}
