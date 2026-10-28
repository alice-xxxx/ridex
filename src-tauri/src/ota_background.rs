#[cfg(target_os = "android")]
pub async fn set_active(active: bool) -> Result<(), String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    tauri::wry::prelude::dispatch(move |env, activity, _webview| {
        let result = env
            .call_method(activity, "setOtaBackground", "(Z)Z", &[active.into()])
            .and_then(|value| value.z());
        let result = match result {
            Ok(true) => Ok(()),
            Ok(false) => Err("无法设置 OTA 后台服务".to_string()),
            Err(error) => {
                let _ = env.exception_clear();
                Err(format!("调用 OTA 后台服务失败：{error}"))
            }
        };
        let _ = sender.send(result);
    });
    receiver.await.map_err(|error| error.to_string())?
}

#[cfg(not(target_os = "android"))]
pub async fn set_active(_active: bool) -> Result<(), String> {
    Ok(())
}
