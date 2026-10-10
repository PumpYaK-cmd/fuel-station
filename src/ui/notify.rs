#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::{APP_ICON, APP_ICON_SIZE};
use crate::store::Event;

pub fn toast_text(e: &Event) -> Option<String> {
    let card = e.card_key.as_deref()?;
    match e.kind.as_str() {
        "submit_review" => Some(format!("{card} ready for review")),
        "ship" => Some(format!("{card} shipped")),
        "review_result" if !e.message.ends_with("Review: clean") => {
            let (_, findings) = e.message.rsplit_once("Review: ")?;
            Some(format!("{card} review: {findings}"))
        }
        "card_move" if e.message.ends_with("-> Review") || e.message.ends_with("-> Done") => {
            Some(e.message.clone())
        }
        _ => None,
    }
}

#[cfg(windows)]
const APP_ID: &str = "FuelStation.Station";

#[cfg(windows)]
static REGISTERED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
pub fn register_app_id() -> Result<(), String> {
    let (key, _) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .create_subkey(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))
        .map_err(|e| e.to_string())?;
    key.set_value("DisplayName", &"Fuel Station")
        .map_err(|e| e.to_string())?;
    REGISTERED.store(true, Ordering::Relaxed);
    Ok(())
}

pub fn toast(title: &str, body: &str) {
    let (title, body) = (title.to_string(), body.to_string());
    std::thread::spawn(move || {
        let mut notification = notify_rust::Notification::new();
        notification.summary(&title).body(&body);
        #[cfg(windows)]
        if REGISTERED.load(Ordering::Relaxed) {
            notification.app_id(APP_ID);
        }
        #[cfg(not(windows))]
        notification.appname("Fuel Station");
        let _ = notification.show();
    });
}

pub struct Tray {
    _icon: TrayIcon,
    show: MenuId,
    quit: MenuId,
}

pub enum TrayAction {
    Show,
    Quit,
}

pub fn tray() -> Result<Tray, String> {
    let show = MenuItem::new("Show", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::with_items(&[&show, &quit]).map_err(|e| e.to_string())?;
    let icon = TrayIconBuilder::new()
        .with_icon(icon()?)
        .with_tooltip("Fuel Station")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Tray {
        _icon: icon,
        show: show.id().clone(),
        quit: quit.id().clone(),
    })
}

fn icon() -> Result<Icon, String> {
    Icon::from_rgba(APP_ICON.to_vec(), APP_ICON_SIZE, APP_ICON_SIZE).map_err(|e| e.to_string())
}

pub fn tray_messages(tray: &Tray) -> Vec<TrayAction> {
    let mut actions = Vec::new();
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            actions.push(TrayAction::Show);
        }
    }
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == tray.show {
            actions.push(TrayAction::Show);
        } else if event.id == tray.quit {
            actions.push(TrayAction::Quit);
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Actor, Role};

    fn event(kind: &str, message: &str) -> Event {
        Event {
            id: 1,
            project_key: "FS".to_string(),
            card_key: Some("FS-7".to_string()),
            actor: Actor {
                role: Role::Truck,
                name: "truck".to_string(),
            },
            kind: kind.to_string(),
            message: message.to_string(),
            created_at: 0,
        }
    }

    #[test]
    fn toast_text_maps_review_and_done() {
        let cases = [
            (
                "submit_review",
                "Submitted FS-7 for review (a..b)",
                Some("FS-7 ready for review"),
            ),
            ("ship", "Shipped FS-7 at abc", Some("FS-7 shipped")),
            (
                "review_result",
                "FS-7: Review: 2 findings",
                Some("FS-7 review: 2 findings"),
            ),
            ("review_result", "FS-7: Review: clean", None),
            (
                "card_move",
                "Moved FS-7 Doing -> Review",
                Some("Moved FS-7 Doing -> Review"),
            ),
            (
                "card_move",
                "Moved FS-7 Review -> Done",
                Some("Moved FS-7 Review -> Done"),
            ),
            ("card_move", "Moved FS-7 Ready -> Doing", None),
            ("report", "gate green", None),
        ];
        for (kind, message, expected) in cases {
            assert_eq!(
                toast_text(&event(kind, message)).as_deref(),
                expected,
                "{kind}: {message}"
            );
        }
    }
}
