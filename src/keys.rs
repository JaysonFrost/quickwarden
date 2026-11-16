use eframe::egui::{self, Event, Key, Modifiers, ViewportCommand};

use crate::app::{Action, App, Field, Mode, Pane, fields};
use crate::{hello, vault};

impl App {
    pub(crate) fn keys(&mut self, ctx: &egui::Context, results: &[(u8, usize)]) {
        let pressed = ctx.input_mut(|i| {
            let copy = i.events.iter().any(|e| matches!(e, Event::Copy));
            i.events.retain(|e| !matches!(e, Event::Copy));
            let pasted = i
                .events
                .iter()
                .any(|e| matches!(e, Event::Paste(p) if !p.is_empty()));
            [
                pasted,
                i.consume_key(Modifiers::COMMAND, Key::H),
                i.consume_key(Modifiers::SHIFT, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowRight)
                    || i.consume_key(Modifiers::NONE, Key::Tab),
                i.consume_key(Modifiers::NONE, Key::ArrowLeft)
                    || i.consume_key(Modifiers::SHIFT, Key::Tab),
                copy,
                i.consume_key(Modifiers::COMMAND, Key::Num1),
                i.consume_key(Modifiers::COMMAND, Key::Num2),
                i.consume_key(Modifiers::COMMAND, Key::Num3),
                i.consume_key(Modifiers::COMMAND, Key::L),
                i.consume_key(Modifiers::COMMAND, Key::Q),
            ]
        });
        let [
            pasted,
            use_hello,
            shift_enter,
            enter,
            escape,
            up,
            down,
            right,
            left,
            copy,
            f1,
            f2,
            f3,
            lock,
            quit,
        ] = pressed;

        if quit {
            self.quitting = true;
            if let Some(session) = self.session.take()
                && !hello::enrolled()
            {
                vault::lock(&session);
            }
            self.egui.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        if escape {
            if self.pane == Pane::Fields {
                self.pane = Pane::List;
            } else {
                self.hide(true);
            }
        }
        if self.mode == Mode::Locked {
            if enter || shift_enter {
                self.unlock();
            }
            self.paste_unlock = pasted;
            if use_hello {
                self.hello_unlock();
            }
        }
        if self.mode != Mode::Ready {
            return;
        }
        if lock {
            self.lock();
            return;
        }
        let Some(n) = self.current(results) else {
            return;
        };
        let item_fields = fields(&self.items[n]);
        self.field = self.field.min(item_fields.len().saturating_sub(1));
        match self.pane {
            Pane::List => {
                if up || down {
                    self.selected = if up {
                        self.selected.saturating_sub(1)
                    } else {
                        (self.selected + 1).min(results.len() - 1)
                    };
                    self.scroll_to_selected = true;
                }
                if right && !item_fields.is_empty() {
                    self.pane = Pane::Fields;
                    self.field = 0;
                }
            }
            Pane::Fields => {
                if up {
                    self.field = self.field.saturating_sub(1);
                }
                if down {
                    self.field = (self.field + 1).min(item_fields.len().saturating_sub(1));
                }
                if left {
                    self.pane = Pane::List;
                }
            }
        }
        let focused_field = item_fields
            .get(self.field)
            .copied()
            .filter(|_| self.pane == Pane::Fields);
        if shift_enter {
            self.run(Action::Fill(n, Some(Field::Password)));
        } else if enter {
            self.run(Action::Fill(n, focused_field));
        } else if copy {
            self.run(Action::Copy(n, focused_field.unwrap_or(Field::Password)));
        } else if let Some(f) = [
            (f1, Field::Username),
            (f2, Field::Password),
            (f3, Field::Totp),
        ]
        .into_iter()
        .find_map(|(hit, f)| hit.then_some(f))
        .filter(|f| item_fields.contains(f))
        {
            self.run(Action::Fill(n, Some(f)));
        }
    }
}
