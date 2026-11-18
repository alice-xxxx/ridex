use btleplug::{
    api::{Central, Manager as _, Peripheral as _, RetrievePeripheralsOptions},
    platform::{Adapter, Manager},
};
use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::Device;

pub async fn connected_devices(
    adapter: &OnceCell<Adapter>,
    service: Uuid,
) -> Result<Vec<Device>, String> {
    let adapter = adapter
        .get_or_try_init(|| async {
            Manager::new()
                .await
                .map_err(|error| error.to_string())?
                .adapters()
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .next()
                .ok_or_else(|| "没有可用的蓝牙适配器".to_string())
        })
        .await?;
    // Service-only retrieval selects system-connected devices without an advertising scan.
    let peripherals = adapter
        .retrieve_peripherals(RetrievePeripheralsOptions {
            identifiers: None,
            services: Some(vec![service]),
        })
        .await
        .map_err(|error| error.to_string())?;
    let mut devices = Vec::with_capacity(peripherals.len());
    for peripheral in peripherals {
        #[cfg(target_vendor = "apple")]
        let address = peripheral.id().to_string();
        #[cfg(not(target_vendor = "apple"))]
        let address = peripheral.address().to_string();
        let properties = peripheral
            .properties()
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let name = [properties.local_name, properties.advertisement_name]
            .into_iter()
            .flatten()
            .find(|name| !name.trim().is_empty())
            .unwrap_or_else(|| address.clone());
        devices.push(Device { address, name });
    }
    Ok(devices)
}
