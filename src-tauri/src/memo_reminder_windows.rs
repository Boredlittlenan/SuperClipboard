#[cfg(not(windows))]
use crate::memo_reminders::ReminderItem;

#[cfg(windows)]
mod desktop {
    use crate::memo_reminders::{deep_link, notify_scheduler, ReminderItem};
    use tauri::Manager;
    use windows::{
        core::HSTRING,
        Data::Xml::Dom::XmlDocument,
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, WPARAM},
            UI::{
                Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
                WindowsAndMessaging::{
                    PBT_APMRESUMEAUTOMATIC, WM_NCDESTROY, WM_POWERBROADCAST, WM_TIMECHANGE,
                },
            },
        },
        UI::Notifications::{ToastNotification, ToastNotificationManager},
    };

    const SUBCLASS_ID: usize = 1;

    unsafe extern "system" fn window_message(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        id: usize,
        _: usize,
    ) -> LRESULT {
        if message == WM_TIMECHANGE
            || (message == WM_POWERBROADCAST && wparam.0 == PBT_APMRESUMEAUTOMATIC as usize)
        {
            notify_scheduler();
        } else if message == WM_NCDESTROY {
            let _ = unsafe { RemoveWindowSubclass(hwnd, Some(window_message), id) };
        }
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    pub fn watch_resume(app: &tauri::AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            if let Ok(hwnd) = window.hwnd() {
                let hwnd = HWND(hwnd.0);
                if !unsafe { SetWindowSubclass(hwnd, Some(window_message), SUBCLASS_ID, 0) }
                    .as_bool()
                {
                    log::warn!("Could not subscribe to Windows power messages for memo reminders");
                }
            }
        }
    }

    fn toast_text(value: &str) -> String {
        value
            .chars()
            .filter(|c| !c.is_control() && *c != '\u{fffe}' && *c != '\u{ffff}')
            .take(120)
            .collect::<String>()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    pub fn show(
        app: &tauri::AppHandle,
        items: &[ReminderItem],
        include_title: bool,
        chinese: bool,
    ) {
        let Some(first) = items.first() else { return };
        let heading = if chinese {
            "备忘录提醒"
        } else {
            "Memo reminder"
        };
        let (target, body) = if items.len() == 1 {
            let memo_title = first.title.trim();
            let body = if include_title && !memo_title.is_empty() {
                memo_title.to_owned()
            } else if chinese {
                "点击查看备忘录".to_owned()
            } else {
                "Open the memo".to_owned()
            };
            (first.token.as_str(), body)
        } else {
            let count = items.len();
            let body = if chinese {
                format!("有 {count} 条备忘录提醒待处理")
            } else {
                format!("{count} memo reminders to review")
            };
            ("list", body)
        };
        let app_id = app.config().identifier.clone();
        let tag = first.token[..16].to_string();
        let data = format!(
            r#"<toast activationType="protocol" launch="{}"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
            deep_link(target),
            heading,
            toast_text(&body),
        );
        if let Err(error) = post_toast(&app_id, &data, &tag) {
            log::warn!("Windows memo reminder notification could not be shown: {error}");
        }
    }

    fn post_toast(app_id: &str, data: &str, tag: &str) -> windows::core::Result<()> {
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))?;
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from(data))?;
        let toast = ToastNotification::CreateToastNotification(&xml)?;
        toast.SetGroup(&HSTRING::from("memo-reminders"))?;
        toast.SetTag(&HSTRING::from(tag))?;
        // Windows applies notification permissions when posting. Querying Setting first can
        // fail before the application has posted its first notification.
        notifier.Show(&toast)
    }
}

#[cfg(windows)]
pub use desktop::{show, watch_resume};

#[cfg(not(windows))]
pub fn show(_: &tauri::AppHandle, _: &[ReminderItem], _: bool, _: bool) {}

#[cfg(not(windows))]
pub fn watch_resume(_: &tauri::AppHandle) {}
