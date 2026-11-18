fn main() {
    tauri_plugin::Builder::new(&["connected_devices"])
        .android_path("android")
        .build();
}
