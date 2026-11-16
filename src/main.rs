#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod context;
mod hello;
mod keys;
mod matcher;
mod output;
mod theme;
mod totp;
mod vault;
mod views;

use eframe::egui;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Quickwarden")
            .with_inner_size([780.0, 470.0])
            .with_always_on_top()
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "quickwarden",
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
