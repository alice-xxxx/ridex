use crate::{channel::AnyChannel, modbus};

use std::{
    io::Write,
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
use tauri_plugin_fs::{FsExt, OpenOptions};

use bin_file::BinFile;

const OTA_PACKET_SIZE: usize = 128;
const OTA_MAX_PACKETS: usize = u16::MAX as usize + 1;
const OTA_MAX_FIRMWARE_SIZE: usize = OTA_PACKET_SIZE * OTA_MAX_PACKETS;
const OTA_TIMEOUT_RETRIES: usize = 3;
const OTA_CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
pub(crate) const OTA_HEADER_SIZE: usize = 64;
const TEXT_FIRMWARE_EXTENSIONS: &[&str] = &[
    "hex", "ihex", "ihx", "tek", "tekhex", "srec", "s19", "s28", "s37", "mot", "srecord", "txt",
    "titxt", "ti-txt", "vmem", "mem",
];
const ELF_FIRMWARE_EXTENSIONS: &[&str] = &["elf", "axf", "out"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OtaPackRequest {
    pub manufacturer: [u8; 4],
    pub device_type: [u8; 2],
    pub hardware_version: [u8; 2],
    pub software_version: [u8; 2],
    pub device_address: u8,
    pub description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtaFileInfo {
    pub file_crc: u32,
    pub file_size: u32,
    pub manufacturer: [u8; 4],
    pub device_type: [u8; 2],
    pub hardware_version: [u8; 2],
    pub software_version: [u8; 2],
    pub device_address: Option<u8>,
    pub description: String,
    pub upgrade_crc: u32,
    pub upgrade_size: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtaStartResult {
    pub bytes_per_cycle: usize,
}

pub fn ota_info(path: String, app: AppHandle) -> Result<OtaFileInfo, String> {
    let bytes = AnyChannel::read_file(&app, &path)?;
    parse_ota_file(&bytes)
}

pub fn get_bin(path: &str, is_ota: bool, app: &AppHandle) -> Result<Vec<u8>, String> {
    let data = AnyChannel::read_file(app, path)?;
    let firmware = if is_ota {
        parse_ota_file(&data)?;
        data.get(OTA_HEADER_SIZE..)
            .ok_or_else(|| "OTA 升级内容为空".to_string())?
            .to_vec()
    } else {
        normalize_firmware(path, data)?
    };

    if firmware.is_empty() {
        return Err("固件内容为空".to_string());
    }
    if firmware.len() > OTA_MAX_FIRMWARE_SIZE {
        return Err("固件超过 8 MiB".to_string());
    }
    Ok(firmware)
}

fn normalize_firmware(path: &str, data: Vec<u8>) -> Result<Vec<u8>, String> {
    let extension = firmware_extension(path);
    if extension.as_deref() == Some("bin") {
        return Ok(data);
    }
    if data.starts_with(b"\x7FELF") || matches_elf_extension(extension.as_deref()) {
        return normalize_elf(data);
    }
    if matches_text_extension(extension.as_deref()) {
        return normalize_text_firmware(data);
    }
    if extension.is_none() {
        if let Ok(firmware) = normalize_text_firmware(data) {
            return Ok(firmware);
        }
    }

    Err(format!(
        "不支持的固件格式{}；支持 BIN、Intel/Extended-Tek HEX、S-Record、TI-TXT、VMEM、ELF/AXF",
        extension
            .as_deref()
            .map(|extension| format!(" .{extension}"))
            .unwrap_or_default()
    ))
}

fn matches_text_extension(extension: Option<&str>) -> bool {
    extension.is_some_and(|extension| TEXT_FIRMWARE_EXTENSIONS.contains(&extension))
}

fn matches_elf_extension(extension: Option<&str>) -> bool {
    extension.is_some_and(|extension| ELF_FIRMWARE_EXTENSIONS.contains(&extension))
}

fn normalize_text_firmware(data: Vec<u8>) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(&data).map_err(|_| "文本固件必须是 UTF-8 编码".to_string())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let mut image = BinFile::new();
    image
        .add_strings(lines, false)
        .map_err(|error| format!("解析文本固件失败: {error}"))?;
    image_to_bin(image)
}

fn normalize_elf(data: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut image = BinFile::new();
    image
        .add_elf(&data, false)
        .map_err(|error| format!("解析 ELF/AXF 固件失败: {error}"))?;
    image_to_bin(image)
}

fn firmware_extension(path: &str) -> Option<String> {
    path.rsplit_once('.')?
        .1
        .split(['?', '#'])
        .next()
        .filter(|extension| !extension.is_empty())
        .map(str::to_ascii_lowercase)
}

fn image_to_bin(image: BinFile) -> Result<Vec<u8>, String> {
    let start = image
        .minimum_address()
        .ok_or_else(|| "固件中没有可升级的数据段".to_string())?;
    let end = image
        .maximum_address()
        .ok_or_else(|| "固件中没有可升级的数据段".to_string())?;
    let size = end
        .checked_sub(start)
        .ok_or_else(|| "固件地址范围无效".to_string())?;
    if size > OTA_MAX_FIRMWARE_SIZE {
        return Err("转换后的固件超过 8 MiB".to_string());
    }
    image
        .to_bytes(.., Some(0xff))
        .map_err(|error| format!("转换固件为 BIN 失败: {error}"))
}

pub fn ota_pack(
    input_path: String,
    output_path: String,
    request: OtaPackRequest,
    app: AppHandle,
) -> Result<(), String> {
    let firmware = get_bin(&input_path, false, &app)?;
    if firmware.len() > 1024 * 1024 {
        return Err("固件过大".to_string());
    }

    let total_size = OTA_HEADER_SIZE + firmware.len();
    let mut ota = vec![0u8; total_size];

    ota[4..8].copy_from_slice(&((total_size - 8) as u32).to_be_bytes());
    ota[8..12].copy_from_slice(&request.manufacturer);
    ota[12..14].copy_from_slice(&request.device_type);
    ota[14..16].copy_from_slice(&request.hardware_version);
    ota[16..18].copy_from_slice(&request.software_version);
    ota[18] = request.device_address;
    let description = request.description.as_bytes();
    let description_len = description.len().min(20);
    ota[32..32 + description_len].copy_from_slice(&description[..description_len]);
    ota[56..60].copy_from_slice(&OTA_CRC32.checksum(&firmware).to_be_bytes());
    ota[60..64].copy_from_slice(&(firmware.len() as u32).to_be_bytes());
    ota[64..].copy_from_slice(&firmware);

    let file_crc = OTA_CRC32.checksum(&ota[4..]);
    ota[0..4].copy_from_slice(&file_crc.to_be_bytes());

    let output_url = AnyChannel::file_url(&output_path)?;
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
    Ok(())
}

impl AnyChannel {
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
                let _ = app.emit(
                    "ota-progress",
                    (completed_packets * 100 / total_packets) as u8,
                );
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
                let _ = app.emit("ota-progress", 100);
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
            Self::Ble { ota_stop, .. } => ota_stop.as_ref(),
            Self::Serial { ota_stop, .. } => ota_stop.as_ref(),
            Self::Network { ota_stop, .. } => ota_stop.as_ref(),
        }
    }

    fn ensure_ota_running(&self) -> Result<(), String> {
        if self.ota_stop_flag().swap(false, Ordering::AcqRel) {
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

pub fn parse_ota_file(data: &[u8]) -> Result<OtaFileInfo, String> {
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

    let expected_file_size = (data.len() - 8) as u32;
    let expected_upgrade_size = (data.len() - OTA_HEADER_SIZE) as u32;

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
