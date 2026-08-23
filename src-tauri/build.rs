use std::path::PathBuf;

const DEV_AUTH_URL: &str = "https://auth.868881.xyz/";
const DEV_AUTH_PUBLIC_KEY: &str =
    "BCaxkVkuoC1UpjuEH998UZNTbvk2-clb4TNKs2JnVmq7LtRoV2d9flyXufot1fSlD2l5MVpAj0LzNgW58k_Mhxw";

fn main() {
    let release = std::env::var("PROFILE").as_deref() == Ok("release");

    let temporary = temporary_settings();
    let auth_url = setting("RIDEX_AUTH_URL", &temporary);
    let auth_key = setting("RIDEX_AUTH_PUBLIC_KEY", &temporary);
    let (auth_url, auth_key) = if release {
        let url = auth_url.expect("missing release build configuration: RIDEX_AUTH_URL");
        assert!(
            url.starts_with("https://"),
            "RIDEX_AUTH_URL must use HTTPS in release builds"
        );
        let key = auth_key.expect("missing release build configuration: RIDEX_AUTH_PUBLIC_KEY");
        assert!(
            valid_key(&key),
            "RIDEX_AUTH_PUBLIC_KEY must be a P-256 SEC1 Base64URL key"
        );
        (url, key)
    } else {
        (
            auth_url.unwrap_or_else(|| DEV_AUTH_URL.to_string()),
            auth_key.unwrap_or_else(|| DEV_AUTH_PUBLIC_KEY.to_string()),
        )
    };
    let generated = format!(
        "pub const URL: &str = {auth_url:?};\npub const PUBLIC_KEY: &str = {auth_key:?};\n"
    );
    let output = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("auth_config.rs");
    std::fs::write(output, generated).unwrap();
    tauri_build::build()
}

fn setting(name: &str, temporary: &[(String, String)]) -> Option<String> {
    println!("cargo:rerun-if-env-changed={name}");
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            temporary
                .iter()
                .find(|(key, value)| key == name && !value.trim().is_empty())
                .map(|(_, value)| value.clone())
        })
}

fn temporary_settings() -> Vec<(String, String)> {
    let path =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join(".garow-build.env");
    println!("cargo:rerun-if-changed={}", path.display());

    std::fs::read_to_string(path)
        .ok()
        .into_iter()
        .flat_map(|contents| {
            contents
                .lines()
                .filter_map(|line| line.split_once('='))
                .map(|(name, value)| (name.trim().to_string(), value.to_string()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn valid_key(key: &str) -> bool {
    key.len() == 87
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}
