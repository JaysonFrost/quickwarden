#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod context;
mod matcher;
mod output;
mod vault;

use eframe::egui;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Bitwarden Quick")
            .with_inner_size([560.0, 360.0])
            .with_always_on_top()
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "bwquick",
        options,
        Box::new(|cc| {
            let hotkeys = GlobalHotKeyManager::new()?;
            hotkeys.register(HotKey::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::Space,
            ))?;
            let (tx, rx) = std::sync::mpsc::channel();
            let egui_ctx = cc.egui_ctx.clone();
            let hotkey_tx = tx.clone();
            GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                if e.state == HotKeyState::Pressed {
                    let _ = hotkey_tx.send(app::Msg::Show);
                    egui_ctx.request_repaint();
                }
            }));
            Ok(Box::new(app::App::new(
                cc.egui_ctx.clone(),
                tx,
                rx,
                hotkeys,
            )))
        }),
    )
}
