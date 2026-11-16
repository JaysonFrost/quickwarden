use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Key, Modifiers, ViewportCommand};
use global_hotkey::GlobalHotKeyManager;

use crate::context::{self, Ctx};
use crate::vault::{self, Item};
use crate::{matcher, output};

const LOCK_AFTER: Duration = Duration::from_secs(15 * 60);
const FILL_DELAY: Duration = Duration::from_millis(250);
const HELP: &str =
    "Enter fill · Shift+Enter password · Ctrl/⌘ +C password +U user +T TOTP +L lock +Q quit";

pub enum Msg {
    Show,
    Status(Result<String, String>),
    Unlocked(Result<(String, Vec<Item>), String>),
    Loaded(Result<Vec<Item>, String>),
    Totp(Result<String, String>),
    Url(isize, String),
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Checking,
    Locked,
    Ready,
    LoggedOut,
}

pub struct App {
    egui: egui::Context,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    _hotkeys: GlobalHotKeyManager,
    session: Option<String>,
    items: Vec<Item>,
    ctx: Ctx,
    query: String,
    selected: usize,
    scroll_to_selected: bool,
    mode: Mode,
    busy: bool,
    status: String,
    visible: bool,
    quitting: bool,
    last_used: Instant,
    pending_fill: Option<(Item, bool, Instant)>,
}

impl App {
    pub fn new(
        egui: egui::Context,
        tx: Sender<Msg>,
        rx: Receiver<Msg>,
        hotkeys: GlobalHotKeyManager,
    ) -> Self {
        let mut app = Self {
            egui,
            tx,
            rx,
            _hotkeys: hotkeys,
            session: vault::env_session(),
            items: Vec::new(),
            ctx: Ctx::default(),
            query: String::new(),
            selected: 0,
            scroll_to_selected: false,
            mode: Mode::Checking,
            busy: false,
            status: String::new(),
            visible: true,
            quitting: false,
            last_used: Instant::now(),
            pending_fill: None,
        };
        app.check_status();
        app
    }

    fn spawn(&mut self, work: impl FnOnce() -> Msg + Send + 'static) {
        self.busy = true;
        let (tx, egui) = (self.tx.clone(), self.egui.clone());
        std::thread::spawn(move || {
            let _ = tx.send(work());
            egui.request_repaint();
        });
    }

    fn check_status(&mut self) {
        self.mode = Mode::Checking;
        self.status = "Checking vault...".into();
        let session = self.session.clone();
        self.spawn(move || Msg::Status(vault::status(session.as_deref())));
    }

    fn handle(&mut self, msg: Msg) {
        if !matches!(msg, Msg::Show | Msg::Url(..)) {
            self.busy = false;
        }
        match msg {
            Msg::Show => self.show(),
            Msg::Status(Ok(s)) if s == "unlocked" && self.session.is_some() => {
                self.status = "Loading vault...".into();
                let session = self.session.clone().unwrap_or_default();
                self.spawn(move || Msg::Loaded(vault::load(&session)));
            }
            Msg::Status(Ok(s)) if s == "unauthenticated" => {
                self.mode = Mode::LoggedOut;
                self.status =
                    "Run `bw login` in a terminal, then press Ctrl+Shift+Space again".into();
            }
            Msg::Status(Ok(_)) => self.set_locked("Master password, Enter to unlock"),
            Msg::Status(Err(e)) => self.set_locked(&e),
            Msg::Unlocked(Ok((session, items))) => {
                self.session = Some(session);
                self.set_ready(items);
            }
            Msg::Loaded(Ok(items)) => self.set_ready(items),
            Msg::Unlocked(Err(e)) | Msg::Loaded(Err(e)) => {
                self.session = None;
                self.set_locked(&e);
            }
            Msg::Totp(Ok(code)) => self.copy(code, "TOTP"),
            Msg::Totp(Err(e)) => self.status = e,
            Msg::Url(handle, url) => {
                if handle == self.ctx.handle && !url.is_empty() {
                    self.ctx.url = url;
                }
            }
        }
    }

    fn set_locked(&mut self, status: &str) {
        self.mode = Mode::Locked;
        self.items.clear();
        self.query.clear();
        self.status = status.to_string();
    }

    fn set_ready(&mut self, items: Vec<Item>) {
        self.mode = Mode::Ready;
        self.items = items;
        self.query.clear();
        self.selected = 0;
        self.status = HELP.into();
    }

    fn show(&mut self) {
        if !self.visible {
            self.ctx = context::active();
            let ctx = self.ctx.clone();
            let (tx, egui) = (self.tx.clone(), self.egui.clone());
            std::thread::spawn(move || {
                let _ = tx.send(Msg::Url(ctx.handle, context::url(&ctx)));
                egui.request_repaint();
            });
            self.query.clear();
            self.selected = 0;
            self.visible = true;
            self.egui.send_viewport_cmd(ViewportCommand::Visible(true));
        }
        self.last_used = Instant::now();
        self.egui.send_viewport_cmd(ViewportCommand::Focus);
        if matches!(self.mode, Mode::Locked | Mode::LoggedOut)
            && self.session.is_none()
            && !self.busy
        {
            self.check_status();
        }
    }

    fn hide(&mut self) {
        if self.visible {
            context::focus(&self.ctx);
            self.visible = false;
            self.egui.send_viewport_cmd(ViewportCommand::Visible(false));
        }
    }

    fn lock(&mut self) {
        if let Some(session) = self.session.take() {
            std::thread::spawn(move || vault::lock(&session));
        }
        self.set_locked("Locked. Master password, Enter to unlock");
    }

    fn copy(&mut self, value: String, what: &str) {
        if value.is_empty() {
            self.status = format!("No {what}");
            return;
        }
        match output::copy_secret(value) {
            Ok(()) => self.hide(),
            Err(e) => self.status = e,
        }
    }

    fn submit(&mut self, current: Option<usize>, full: bool) {
        match self.mode {
            Mode::Locked if !self.busy && !self.query.is_empty() => {
                let password = std::mem::take(&mut self.query);
                self.status = "Unlocking...".into();
                self.spawn(move || {
                    Msg::Unlocked(
                        vault::unlock(&password)
                            .and_then(|s| vault::load(&s).map(|items| (s, items))),
                    )
                });
            }
            Mode::Ready => {
                if let Some(n) = current {
                    self.pending_fill =
                        Some((self.items[n].clone(), full, Instant::now() + FILL_DELAY));
                    self.hide();
                    self.egui.request_repaint_after(FILL_DELAY);
                }
            }
            _ => {}
        }
    }

    fn keys(&mut self, ui: &mut egui::Ui, results: &[(u8, usize)]) {
        let current = results.get(self.selected).map(|&(_, n)| n);
        let pressed = ui.input_mut(|i| {
            let copy = i.events.iter().any(|e| matches!(e, Event::Copy));
            i.events.retain(|e| !matches!(e, Event::Copy));
            [
                i.consume_key(Modifiers::SHIFT, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                copy,
                i.consume_key(Modifiers::COMMAND, Key::U),
                i.consume_key(Modifiers::COMMAND, Key::T),
                i.consume_key(Modifiers::COMMAND, Key::L),
                i.consume_key(Modifiers::COMMAND, Key::Q),
            ]
        });
        let [
            shift_enter,
            enter,
            escape,
            up,
            down,
            copy,
            user,
            totp,
            lock,
            quit,
        ] = pressed;
        if shift_enter || enter {
            self.submit(current, !shift_enter);
        }
        if escape {
            self.hide();
        }
        if (up || down) && !results.is_empty() {
            self.selected = if up {
                self.selected.saturating_sub(1)
            } else {
                (self.selected + 1).min(results.len() - 1)
            };
            self.scroll_to_selected = true;
        }
        if let Some(n) = current.filter(|_| self.mode == Mode::Ready) {
            if copy {
                self.copy(self.items[n].password.clone(), "password");
            }
            if user {
                self.copy(self.items[n].username.clone(), "username");
            }
            if totp && !self.busy {
                let (session, id) = (
                    self.session.clone().unwrap_or_default(),
                    self.items[n].id.clone(),
                );
                self.status = "Getting TOTP...".into();
                self.spawn(move || Msg::Totp(vault::totp(&session, &id)));
            }
        }
        if lock && self.mode == Mode::Ready {
            self.lock();
        }
        if quit {
            self.quitting = true;
            if let Some(session) = self.session.take() {
                vault::lock(&session);
            }
            self.egui.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle(msg);
        }
        if let Some((_, _, at)) = &self.pending_fill {
            let now = Instant::now();
            if now >= *at {
                if let Some((item, full, _)) = self.pending_fill.take()
                    && let Err(e) = output::type_login(&item, full)
                {
                    self.status = e;
                }
            } else {
                ctx.request_repaint_after(*at - now);
            }
        }
        if !self.visible && self.session.is_some() {
            if self.last_used.elapsed() > LOCK_AFTER {
                self.lock();
            } else {
                ctx.request_repaint_after(Duration::from_secs(30));
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.hide();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let results = if self.mode == Mode::Ready {
            matcher::search(&self.items, &self.query, &self.ctx)
        } else {
            Vec::new()
        };
        self.selected = self.selected.min(results.len().saturating_sub(1));
        self.keys(ui, &results);

        egui::CentralPanel::default().show(ui, |ui| {
            let hint = match self.mode {
                Mode::Ready => "Search logins",
                Mode::Locked => "Master password",
                Mode::Checking => "Checking vault...",
                Mode::LoggedOut => "Not logged in",
            };
            let input = egui::TextEdit::singleline(&mut self.query)
                .password(self.mode != Mode::Ready)
                .hint_text(hint)
                .font(egui::TextStyle::Heading)
                .desired_width(f32::INFINITY);
            let enabled = !self.busy && matches!(self.mode, Mode::Ready | Mode::Locked);
            let response = ui.add_enabled(enabled, input);
            if enabled && !response.has_focus() {
                response.request_focus();
            }
            if response.changed() {
                self.selected = 0;
            }
            ui.add_space(6.0);

            let list_height = ui.available_height() - 24.0;
            let mut fill = None;
            egui::ScrollArea::vertical()
                .max_height(list_height)
                .auto_shrink(false)
                .show(ui, |ui| {
                    for (row, &(score, n)) in results.iter().enumerate() {
                        let item = &self.items[n];
                        let star = if score > 0 { "★ " } else { "    " };
                        let text = format!("{star}{}  —  {}", item.name, item.username);
                        let r = ui.add_sized(
                            [ui.available_width(), 22.0],
                            egui::Button::selectable(row == self.selected, text),
                        );
                        if row == self.selected && self.scroll_to_selected {
                            r.scroll_to_me(None);
                        }
                        if r.clicked() {
                            self.selected = row;
                        }
                        if r.double_clicked() {
                            fill = Some(n);
                        }
                    }
                });
            self.scroll_to_selected = false;
            if fill.is_some() {
                self.submit(fill, true);
            }
            ui.separator();
            ui.small(&self.status);
        });
    }
}
