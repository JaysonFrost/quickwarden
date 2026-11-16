use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Sense, Spinner, Stroke, StrokeKind,
    TextEdit, Ui, UiBuilder, pos2, vec2,
};

use crate::app::{Action, App, Field, Mode, Pane, fields};
use crate::matcher::host;
use crate::theme::{self, Palette, elide};
use crate::totp;

const RADIUS: f32 = 14.0;
const HEADER: f32 = 62.0;
const FOOTER: f32 = 38.0;
const LIST_WIDTH: f32 = 310.0;
const MOD: &str = if cfg!(target_os = "macos") {
    "Cmd"
} else {
    "Ctrl"
};

impl App {
    pub(crate) fn draw(&mut self, ui: &mut Ui, results: &[(u8, usize)]) {
        let pal = theme::palette(ui.visuals().dark_mode);
        let full = ui.max_rect();
        ui.painter().rect(
            full,
            RADIUS,
            pal.bg,
            Stroke::new(1.0, pal.border),
            StrokeKind::Inside,
        );
        if self.mode == Mode::Ready {
            self.vault_view(ui, full, results, pal);
        } else {
            self.lock_view(ui, full, pal);
        }
    }

    fn search_field(&mut self, ui: &mut Ui, rect: Rect, hint: &str, password: bool, pal: Palette) {
        let enabled = !self.busy && matches!(self.mode, Mode::Ready | Mode::Locked);
        let edit = TextEdit::singleline(&mut self.query)
            .frame(egui::Frame::NONE)
            .password(password)
            .hint_text(egui::RichText::new(hint).color(pal.muted))
            .font(FontId::proportional(19.0))
            .text_color(pal.text)
            .align(Align2::LEFT_CENTER)
            .desired_width(rect.width());
        let response = ui.put(rect, |ui: &mut Ui| ui.add_enabled(enabled, edit));
        if enabled && !response.has_focus() {
            response.request_focus();
        }
        if response.changed() {
            self.reset_selection();
            self.scroll_to_selected = true;
        }
    }

    fn vault_view(&mut self, ui: &mut Ui, full: Rect, results: &[(u8, usize)], pal: Palette) {
        let header = Rect::from_min_max(full.min, pos2(full.right(), full.top() + HEADER));
        let footer = Rect::from_min_max(pos2(full.left(), full.bottom() - FOOTER), full.max);
        let body = Rect::from_min_max(
            pos2(full.left(), header.bottom()),
            pos2(full.right(), footer.top()),
        );
        let list = Rect::from_min_max(body.min, pos2(body.left() + LIST_WIDTH, body.bottom()));
        let details = Rect::from_min_max(pos2(list.right(), body.top()), body.max);
        let p = ui.painter().clone();

        theme::search_icon(&p, pos2(header.left() + 28.0, header.center().y), pal.muted);
        let badge = self.context_label();
        let badge_width = if badge.is_empty() { 0.0 } else { 210.0 };
        let field = Rect::from_min_max(
            pos2(header.left() + 50.0, header.top() + 12.0),
            pos2(header.right() - 20.0 - badge_width, header.bottom() - 12.0),
        );
        self.search_field(ui, field, "Search Bitwarden", false, pal);
        if !badge.is_empty() {
            let galley =
                p.layout_no_wrap(elide(&badge, 26), FontId::proportional(12.0), pal.accent);
            let pill = Rect::from_min_size(
                pos2(
                    header.right() - 18.0 - galley.size().x - 20.0,
                    header.center().y - 12.0,
                ),
                vec2(galley.size().x + 20.0, 24.0),
            );
            p.rect_filled(pill, 12.0, pal.accent_soft);
            p.galley(pill.center() - galley.size() / 2.0, galley, pal.accent);
        }
        p.hline(
            full.x_range(),
            header.bottom(),
            Stroke::new(1.0, pal.border),
        );
        p.rect_filled(details, 0.0, pal.panel);
        p.vline(list.right(), body.y_range(), Stroke::new(1.0, pal.border));

        let mut action = None;
        ui.scope_builder(
            UiBuilder::new().max_rect(list.shrink2(vec2(0.0, 6.0))),
            |ui| {
                action = action.take().or(self.list(ui, results, pal));
            },
        );
        if let Some(n) = self.current(results) {
            ui.scope_builder(UiBuilder::new().max_rect(details.shrink(22.0)), |ui| {
                action = action.take().or(self.details(ui, n, pal));
            });
        } else {
            p.text(
                details.center(),
                Align2::CENTER_CENTER,
                "No matching logins",
                FontId::proportional(14.0),
                pal.muted,
            );
        }
        self.footer(ui, footer, pal);
        if let Some(action) = action {
            self.run(action);
        }
    }

    fn context_label(&self) -> String {
        let h = host(&self.ctx.url);
        if !h.is_empty() {
            format!("Suggestions for {h}")
        } else if !self.ctx.app.is_empty() {
            format!("Suggestions for {}", self.ctx.app)
        } else {
            String::new()
        }
    }

    fn list(&mut self, ui: &mut Ui, results: &[(u8, usize)], pal: Palette) -> Option<Action> {
        let suggested = results.iter().take_while(|r| r.0 > 0).count();
        let mut action = None;
        egui::ScrollArea::vertical()
            .id_salt("items")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (row, &(_, n)) in results.iter().enumerate() {
                    if suggested > 0 && (row == 0 || row == suggested) {
                        section(ui, if row == 0 { "SUGGESTED" } else { "ALL ITEMS" }, pal);
                    }
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::click());
                    let item = &self.items[n];
                    let selected = row == self.selected;
                    let fill = match (selected, self.pane) {
                        (true, Pane::List) => pal.selected,
                        (true, Pane::Fields) => pal.hover,
                        _ if response.hovered() => pal.hover,
                        _ => Color32::TRANSPARENT,
                    };
                    let p = ui.painter();
                    let card = rect.shrink2(vec2(8.0, 1.0));
                    p.rect_filled(card, 9.0, fill);
                    let icon = Rect::from_center_size(
                        pos2(card.left() + 24.0, card.center().y),
                        vec2(32.0, 32.0),
                    );
                    theme::avatar(p, icon, &item.name);
                    let x = icon.right() + 12.0;
                    p.text(
                        pos2(x, card.center().y - 9.0),
                        Align2::LEFT_CENTER,
                        elide(&item.name, 28),
                        FontId::proportional(14.5),
                        pal.text,
                    );
                    let subtitle = if item.username.is_empty() {
                        "no username".to_string()
                    } else {
                        elide(&item.username, 34)
                    };
                    p.text(
                        pos2(x, card.center().y + 10.0),
                        Align2::LEFT_CENTER,
                        subtitle,
                        FontId::proportional(12.0),
                        pal.muted,
                    );
                    if selected && self.scroll_to_selected {
                        response.scroll_to_me(None);
                    }
                    if response.clicked() {
                        self.selected = row;
                        self.pane = Pane::List;
                    }
                    if response.double_clicked() {
                        action = Some(Action::Fill(n, None));
                    }
                }
            });
        self.scroll_to_selected = false;
        action
    }

    fn details(&mut self, ui: &mut Ui, n: usize, pal: Palette) -> Option<Action> {
        let item = self.items[n].clone();
        let top = ui.max_rect();
        let p = ui.painter().clone();
        let icon = Rect::from_min_size(top.min, vec2(46.0, 46.0));
        theme::avatar(&p, icon, &item.name);
        p.text(
            pos2(icon.right() + 14.0, icon.center().y - 10.0),
            Align2::LEFT_CENTER,
            elide(&item.name, 30),
            FontId::proportional(18.0),
            pal.text,
        );
        let site = item.uris.first().map(|u| host(u)).unwrap_or_default();
        p.text(
            pos2(icon.right() + 14.0, icon.center().y + 12.0),
            Align2::LEFT_CENTER,
            elide(&site, 40),
            FontId::proportional(12.5),
            pal.muted,
        );
        ui.add_space(64.0);

        let mut action = None;
        let item_fields = fields(&item);
        let mut needs_tick = false;
        for (i, field) in item_fields.iter().copied().enumerate() {
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
            let focused = self.pane == Pane::Fields && self.field == i;
            let p = ui.painter();
            let fill = if response.hovered() || focused {
                pal.hover
            } else {
                pal.field
            };
            let stroke = if focused {
                Stroke::new(1.5, pal.accent)
            } else {
                Stroke::new(1.0, pal.border)
            };
            p.rect(rect, 10.0, fill, stroke, StrokeKind::Inside);
            p.text(
                pos2(rect.left() + 14.0, rect.top() + 17.0),
                Align2::LEFT_CENTER,
                field.label(&item),
                FontId::proportional(11.5),
                pal.muted,
            );

            let copy = Rect::from_min_size(
                pos2(rect.right() - 62.0, rect.center().y - 13.0),
                vec2(50.0, 26.0),
            );
            let copy_response = ui.interact(copy, ui.scope_id().with(("copy", i)), Sense::click());
            let value_pos = pos2(rect.left() + 14.0, rect.top() + 38.0);
            match field {
                Field::Password => {
                    p.text(
                        value_pos,
                        Align2::LEFT_CENTER,
                        "••••••••••••",
                        FontId::proportional(15.0),
                        pal.text,
                    );
                }
                Field::Totp => match totp::current(&item.totp) {
                    Some(code) => {
                        needs_tick = true;
                        p.text(
                            value_pos,
                            Align2::LEFT_CENTER,
                            totp::pretty(&code.code),
                            FontId::monospace(17.0),
                            pal.accent,
                        );
                        let fraction = code.remaining as f32 / code.period.max(1) as f32;
                        let color = if code.remaining <= 5 {
                            pal.danger
                        } else {
                            pal.accent
                        };
                        let center = pos2(copy.left() - 56.0, rect.center().y);
                        theme::ring(p, center, 9.0, fraction, color, pal.border);
                        p.text(
                            center,
                            Align2::CENTER_CENTER,
                            code.remaining.to_string(),
                            FontId::proportional(9.5),
                            pal.muted,
                        );
                    }
                    None => {
                        p.text(
                            value_pos,
                            Align2::LEFT_CENTER,
                            "unsupported secret",
                            FontId::proportional(13.0),
                            pal.danger,
                        );
                    }
                },
                Field::Username => {
                    p.text(
                        value_pos,
                        Align2::LEFT_CENTER,
                        elide(&item.username, 34),
                        FontId::proportional(15.0),
                        pal.text,
                    );
                }
            }
            let key = format!("{MOD} {}", field.shortcut());
            p.text(
                pos2(copy.left() - 12.0, rect.top() + 17.0),
                Align2::RIGHT_CENTER,
                key,
                FontId::proportional(11.0),
                pal.muted,
            );
            let copy_fill = if copy_response.hovered() {
                pal.accent
            } else {
                pal.accent_soft
            };
            let copy_text = if copy_response.hovered() {
                Color32::WHITE
            } else {
                pal.accent
            };
            p.rect_filled(copy, 8.0, copy_fill);
            p.text(
                copy.center(),
                Align2::CENTER_CENTER,
                "Copy",
                FontId::proportional(12.0),
                copy_text,
            );

            if copy_response.clicked() {
                action = Some(Action::Copy(n, field));
            } else if response.clicked() {
                action = Some(Action::Fill(n, Some(field)));
            }
            if response.hovered() || copy_response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.add_space(8.0);
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Click a field to type it into the app, or Copy it.")
                .size(11.5)
                .color(pal.muted),
        );
        if needs_tick {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
        action
    }

    fn footer(&self, ui: &mut Ui, rect: Rect, pal: Palette) {
        let radius = CornerRadius {
            nw: 0,
            ne: 0,
            sw: RADIUS as u8,
            se: RADIUS as u8,
        };
        ui.painter()
            .rect_filled(rect.shrink(1.0), radius, pal.panel);
        ui.painter()
            .hline(rect.x_range(), rect.top(), Stroke::new(1.0, pal.border));
        ui.scope_builder(
            UiBuilder::new().max_rect(rect.shrink2(vec2(16.0, 9.0))),
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    if !self.error.is_empty() {
                        ui.label(
                            egui::RichText::new(&self.error)
                                .size(12.0)
                                .color(pal.danger),
                        );
                        return;
                    }
                    let (enter, arrow) = if self.pane == Pane::Fields {
                        ("Fill field", "Back")
                    } else {
                        ("Fill login", "Fields")
                    };
                    theme::keycap(ui, "Enter", enter, pal);
                    theme::keycap(
                        ui,
                        if self.pane == Pane::Fields {
                            "←"
                        } else {
                            "→"
                        },
                        arrow,
                        pal,
                    );
                    theme::keycap(ui, &format!("{MOD} 1·2·3"), "User · Pass · Code", pal);
                    theme::keycap(ui, &format!("{MOD} C"), "Copy", pal);
                    theme::keycap(ui, "Esc", "Close", pal);
                });
            },
        );
    }

    fn lock_view(&mut self, ui: &mut Ui, full: Rect, pal: Palette) {
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
                "Press Enter to unlock",
                FontId::proportional(12.0),
                pal.muted,
            );
        } else if self.mode != Mode::LoggedOut {
            ui.put(
                Rect::from_center_size(field.center(), vec2(26.0, 26.0)),
                Spinner::new().size(24.0).color(pal.accent),
            );
        }
    }
}

fn section(ui: &mut Ui, title: &str, pal: Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().text(
        pos2(rect.left() + 18.0, rect.center().y + 3.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(10.5),
        pal.muted,
    );
}
