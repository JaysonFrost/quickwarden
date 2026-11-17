use eframe::egui::{Align2, FontId, Rect, Sense, Spinner, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::app::{App, Mode};
use crate::hello;
use crate::theme::{self, Palette, elide};
use crate::views::MOD;

impl App {
    pub(crate) fn lock_view(&mut self, ui: &mut Ui, full: Rect, pal: Palette) {
        let p = ui.painter().clone();
        let center = full.center();
        let badge = pos2(center.x, center.y - 96.0);
        p.circle_filled(badge, 34.0, pal.accent_soft);
        theme::lock_icon(&p, badge, pal.accent);

        let (title, subtitle) = match self.mode {
            Mode::LoggedOut => (
                "Not logged in",
                "Run `bw login` in a terminal, then press Ctrl+Shift+Space",
            ),
            Mode::Checking => ("Bitwarden", "Checking vault…"),
            _ if self.hello_prompt => (
                "Windows Hello",
                "Confirm with your face, fingerprint or PIN",
            ),
            _ if self.busy => ("Unlocking…", "Decrypting your vault"),
            _ => ("Unlock Bitwarden", "Enter your master password"),
        };
        p.text(
            pos2(center.x, center.y - 38.0),
            Align2::CENTER_CENTER,
            title,
            FontId::proportional(21.0),
            pal.text,
        );
        let (sub, color) = if self.error.is_empty() {
            (subtitle.to_string(), pal.muted)
        } else {
            (self.error.clone(), pal.danger)
        };
        p.text(
            pos2(center.x, center.y - 12.0),
            Align2::CENTER_CENTER,
            elide(&sub, 70),
            FontId::proportional(13.0),
            color,
        );

        let field = Rect::from_center_size(pos2(center.x, center.y + 32.0), vec2(340.0, 46.0));
        if self.mode == Mode::Locked && !self.busy {
            p.rect(
                field,
                11.0,
                pal.field,
                Stroke::new(1.5, pal.accent),
                StrokeKind::Inside,
            );
            self.search_field(
                ui,
                field.shrink2(vec2(16.0, 8.0)),
                "Master password",
                true,
                pal,
            );
            p.text(
                pos2(center.x, field.bottom() + 22.0),
                Align2::CENTER_CENTER,
                "Press Enter or paste to unlock",
                FontId::proportional(12.0),
                pal.muted,
            );
            if hello::enrolled() {
                let button = Rect::from_center_size(
                    pos2(center.x, field.bottom() + 62.0),
                    vec2(250.0, 34.0),
                );
                let response = ui.interact(button, ui.scope_id().with("hello"), Sense::click());
                let fill = if response.hovered() {
                    pal.selected
                } else {
                    pal.accent_soft
                };
                p.rect_filled(button, 9.0, fill);
                p.text(
                    button.center(),
                    Align2::CENTER_CENTER,
                    format!("Unlock with Windows Hello   {MOD}+H"),
                    FontId::proportional(13.0),
                    pal.accent,
                );
                if response.clicked() {
                    self.hello_unlock();
                }
            }
        } else if self.mode != Mode::LoggedOut {
            ui.put(
                Rect::from_center_size(field.center(), vec2(26.0, 26.0)),
                Spinner::new().size(24.0).color(pal.accent),
            );
        }
    }
}
