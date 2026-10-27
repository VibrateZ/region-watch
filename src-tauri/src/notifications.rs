use std::{collections::VecDeque, path::Path, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager};
use windows::{
    core::HSTRING,
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    UI::Notifications::{
        NotificationSetting, ToastFailedEventArgs, ToastNotification, ToastNotificationManager,
    },
};

// Retain recent toast objects so asynchronous failure handlers stay alive.
#[derive(Default)]
pub struct Notifications(Mutex<VecDeque<ToastNotification>>);

fn register_identity(id: &str, name: &str, data_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| format!("无法创建通知图标目录：{e}"))?;
    let icon = data_dir.join("notification.ico");
    let bytes = include_bytes!("../icons/icon.ico");
    if std::fs::read(&icon).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::write(&icon, bytes).map_err(|e| format!("无法保存通知图标：{e}"))?;
    }
    // Per-user registration works for portable executables without elevation.
    let key = windows_registry::CURRENT_USER
        .create(format!(r"Software\Classes\AppUserModelId\{id}"))
        .map_err(|e| format!("无法注册通知身份：{e}"))?;
    key.set_string("DisplayName", name)
        .map_err(|e| e.to_string())?;
    key.set_string("IconUri", icon.to_string_lossy().as_ref())
        .map_err(|e| e.to_string())?;
    key.set_string("IconBackgroundColor", "0")
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn setting_result(setting: NotificationSetting) -> Result<(), String> {
    match setting {
        NotificationSetting::Enabled => Ok(()),
        NotificationSetting::DisabledForApplication => {
            Err("此应用的通知已关闭，请在 Windows 设置 → 系统 → 通知中开启".into())
        }
        NotificationSetting::DisabledForUser => {
            Err("Windows 已关闭当前用户的通知，请检查系统通知设置".into())
        }
        NotificationSetting::DisabledByGroupPolicy => Err("通知已被 Windows 组策略禁用".into()),
        NotificationSetting::DisabledByManifest => Err("Windows 未认可此应用的通知身份".into()),
        other => Err(format!("Windows 通知不可用（状态 {}）", other.0)),
    }
}

fn create_toast(
    id: &str,
    body: &str,
    on_failure: impl Fn(String) + Send + Sync + 'static,
) -> Result<ToastNotification, String> {
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(id))
        .map_err(|e| e.to_string())?;
    setting_result(notifier.Setting().map_err(|e| e.to_string())?)?;
    let xml = XmlDocument::new().map_err(|e| e.to_string())?;
    xml.LoadXml(&HSTRING::from("<toast><visual><binding template=\"ToastGeneric\"><text>区域变化监测</text><text></text></binding></visual></toast>"))
        .map_err(|e| e.to_string())?;
    let text = xml
        .GetElementsByTagName(&HSTRING::from("text"))
        .map_err(|e| e.to_string())?
        .Item(1)
        .map_err(|e| e.to_string())?;
    text.SetInnerText(&HSTRING::from(body))
        .map_err(|e| e.to_string())?;
    let toast = ToastNotification::CreateToastNotification(&xml).map_err(|e| e.to_string())?;
    toast
        .Failed(
            &TypedEventHandler::<ToastNotification, ToastFailedEventArgs>::new(move |_, args| {
                let message = match args.as_ref().and_then(|args| args.ErrorCode().ok()) {
                    Some(code) => format!("Windows 拒绝显示通知：{code:?}"),
                    None => "Windows 未能显示通知".into(),
                };
                on_failure(message);
                Ok(())
            }),
        )
        .map_err(|e| e.to_string())?;
    notifier.Show(&toast).map_err(|e| e.to_string())?;
    Ok(toast)
}

pub fn send(app: &AppHandle, body: &str) -> Result<(), String> {
    let config = app.config();
    let data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    register_identity(
        &config.identifier,
        config.product_name.as_deref().unwrap_or("区域变化监测器"),
        &data_dir,
    )?;
    let failure_app = app.clone();
    let toast = create_toast(&config.identifier, body, move |message| {
        eprintln!("通知失败：{message}");
        let _ = failure_app.emit("notification-error", message);
    })?;
    let state = app.state::<Notifications>();
    let mut recent = state.0.lock().map_err(|e| e.to_string())?;
    recent.push_back(toast);
    while recent.len() > 16 {
        recent.pop_front();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_disabled_notification_settings() {
        assert!(setting_result(NotificationSetting::Enabled).is_ok());
        for setting in [
            NotificationSetting::DisabledForApplication,
            NotificationSetting::DisabledForUser,
            NotificationSetting::DisabledByGroupPolicy,
            NotificationSetting::DisabledByManifest,
            NotificationSetting(99),
        ] {
            assert!(setting_result(setting).is_err());
        }
    }

    #[test]
    #[ignore = "sends a real Windows notification and registers the application for the current user"]
    fn windows_notification_smoke() {
        let data_dir = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
            .join("com.local.regionwatch");
        register_identity("com.local.regionwatch", "区域变化监测器", &data_dir).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let _toast = create_toast(
            "com.local.regionwatch",
            "便携版通知测试：Windows 已接受通知请求。",
            move |error| {
                let _ = tx.send(error);
            },
        )
        .unwrap();
        match rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
            other => panic!("Notification failed or failure handler was dropped: {other:?}"),
        }
    }
}
