//! Mobile fallback for platforms where the serial transport is disabled.

use crate::modbus;

use std::fmt;

use tauri::AppHandle;

pub struct SerialChannel;

#[derive(Debug)]
pub struct SerialError;

impl fmt::Display for SerialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("serial transport is not supported on this mobile platform")
    }
}

impl std::error::Error for SerialError {}

impl SerialChannel {
    pub fn discover(_app: &AppHandle) -> Result<Vec<(String, String)>, SerialError> {
        Err(SerialError)
    }

    pub async fn connect(
        _app: AppHandle,
        _path: impl Into<String>,
        _baud_rate: u32,
    ) -> Result<Self, SerialError> {
        Err(SerialError)
    }

    pub async fn disconnect(&mut self) -> Result<(), SerialError> {
        Ok(())
    }

    pub async fn modbus_tx(
        &self,
        _request: &modbus::Protocol,
        _timeout_ms: u64,
    ) -> Result<modbus::TransactionResult, SerialError> {
        Err(SerialError)
    }
}
