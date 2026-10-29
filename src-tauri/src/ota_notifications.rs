use std::sync::{mpsc, OnceLock};

use tauri::AppHandle;
use tauri_plugin_notification::{NotificationExt, PermissionState};

enum Event {
    Permission(tokio::sync::oneshot::Sender<()>),
    Start,
    Focused(bool),
    Finish(Result<(), String>, tokio::sync::oneshot::Sender<()>),
}

static EVENTS: OnceLock<mpsc::Sender<Event>> = OnceLock::new();

// Mobile notification calls wait for the main thread. Keep them off the UI and OTA threads,
// and process them in order so background reminders cannot overwrite the result.
pub fn init(app: AppHandle) -> std::io::Result<()> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("ota-notifications".into())
        .spawn(move || {
            let mut allowed = false;
            let mut background = false;
            let mut active = false;
            for event in receiver {
                match event {
                    Event::Permission(reply) => {
                        allowed = match app.notification().request_permission() {
                            Ok(PermissionState::Granted) => true,
                            Ok(state) => {
                                tauri_plugin_log::log::warn!("OTA 通知未授权：{state:?}");
                                false
                            }
                            Err(error) => {
                                tauri_plugin_log::log::warn!("申请 OTA 通知权限失败：{error}");
                                false
                            }
                        };
                        let _ = reply.send(());
                        continue;
                    }
                    Event::Start => active = true,
                    Event::Focused(focused) => {
                        if background == !focused {
                            continue;
                        }
                        background = !focused;
                        // Android's notification also belongs to the foreground service.
                        #[cfg(target_os = "ios")]
                        if focused && allowed {
                            let _ = app.notification().remove_active(vec![1]);
                        }
                    }
                    Event::Finish(result, reply) => {
                        let was_active = std::mem::replace(&mut active, false);
                        if allowed && was_active {
                            #[cfg(any(target_os = "android", target_os = "ios"))]
                            let _ = app.notification().remove_active(vec![1]);
                            if background {
                                let (title, body): (&str, String) = match result {
                                    Ok(()) => (
                                        "OTA 升级完成",
                                        "固件已更新，请回到软件查看设备状态".into(),
                                    ),
                                    Err(error) if error == "OTA 已停止" => {
                                        ("OTA 已终止", "固件传输已停止".into())
                                    }
                                    Err(error) => {
                                        ("OTA 升级失败", error.chars().take(160).collect())
                                    }
                                };
                                let result = app
                                    .notification()
                                    .builder()
                                    .id(2)
                                    .title(title)
                                    .body(body)
                                    .auto_cancel()
                                    .show();
                                if let Err(error) = result {
                                    tauri_plugin_log::log::warn!("发送 OTA 结果通知失败：{error}");
                                }
                            }
                        }
                        let _ = reply.send(());
                        continue;
                    }
                }
                if active && allowed && background {
                    let notification = app
                        .notification()
                        .builder()
                        .id(1)
                        .title("OTA 正在升级")
                        .body("固件正在更新，请保持设备连接，返回软件可查看进度")
                        .ongoing();
                    #[cfg(target_os = "android")]
                    let notification = notification.channel_id("ota");
                    if let Err(error) = notification.show() {
                        tauri_plugin_log::log::warn!("发送 OTA 后台通知失败：{error}");
                    }
                }
            }
        })?;
    let _ = EVENTS.set(sender);
    Ok(())
}

fn send(event: Event) {
    if let Some(sender) = EVENTS.get() {
        let _ = sender.send(event);
    }
}

pub async fn request_permission() {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    send(Event::Permission(sender));
    let _ = receiver.await;
}

pub fn start() {
    send(Event::Start);
}

pub fn focused(focused: bool) {
    send(Event::Focused(focused));
}

pub async fn finish(result: Result<(), String>) {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    send(Event::Finish(result, sender));
    // Give the notification time to reach iOS before ending the background task,
    // without making OTA cleanup depend on an unresponsive system dialog or IPC.
    if tokio::time::timeout(std::time::Duration::from_secs(2), receiver)
        .await
        .is_err()
    {
        tauri_plugin_log::log::warn!("发送 OTA 结果通知等待超时");
    }
}
