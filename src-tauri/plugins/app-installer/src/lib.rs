use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

#[cfg(target_os = "android")]
use tauri::{AppHandle, Manager};

#[cfg(target_os = "android")]
struct AppInstaller<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[cfg(target_os = "android")]
pub async fn install<R: Runtime>(
    app: &AppHandle<R>,
    path: &std::path::Path,
) -> Result<bool, String> {
    app.state::<AppInstaller<R>>()
        .0
        .run_mobile_plugin_async::<serde_json::Value>(
            "install",
            serde_json::json!({ "path": path.to_string_lossy() }),
        )
        .await
        .map(|_| true)
        .map_err(|error| error.to_string())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("app-installer")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            app.manage(AppInstaller(api.register_android_plugin(
                "com.xyz.appinstaller",
                "AppInstallerPlugin",
            )?));
            #[cfg(not(target_os = "android"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}
