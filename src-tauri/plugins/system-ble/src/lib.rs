use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, Runtime,
};
use uuid::Uuid;

#[cfg(not(target_os = "android"))]
mod desktop;

#[cfg(not(target_os = "android"))]
struct SystemBle(tokio::sync::OnceCell<btleplug::platform::Adapter>);

#[cfg(target_os = "android")]
struct SystemBle<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Device {
    pub address: String,
    pub name: String,
}

pub use commands::connected_devices;

mod commands {
    use super::*;

    /// Queries devices only; establishing a connection belongs to the BLE client.
    #[tauri::command]
    pub async fn connected_devices<R: Runtime>(
        app: AppHandle<R>,
        service_uuid: String,
    ) -> Result<Vec<Device>, String> {
        let service = Uuid::parse_str(&service_uuid).map_err(|_| "服务 UUID 无效".to_string())?;
        #[cfg(target_os = "android")]
        {
            #[derive(Deserialize)]
            struct Response {
                devices: Vec<Device>,
            }
            app.state::<SystemBle<R>>()
                .0
                .run_mobile_plugin_async::<Response>(
                    "connectedDevices",
                    serde_json::json!({ "serviceUuid": service.to_string() }),
                )
                .await
                .map(|response| response.devices)
                .map_err(|error| error.to_string())
        }
        #[cfg(not(target_os = "android"))]
        desktop::connected_devices(&app.state::<SystemBle>().0, service).await
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("system-ble")
        .invoke_handler(tauri::generate_handler![commands::connected_devices])
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            app.manage(SystemBle(
                api.register_android_plugin("com.xyz.systemble", "SystemBlePlugin")?,
            ));
            #[cfg(not(target_os = "android"))]
            {
                let _ = api;
                app.manage(SystemBle(tokio::sync::OnceCell::new()));
            }
            Ok(())
        })
        .build()
}
