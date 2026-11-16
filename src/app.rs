use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Key, Modifiers, ViewportCommand};
use global_hotkey::GlobalHotKeyManager;

use crate::context::{self, Ctx};
use crate::output::{self, Part};
use crate::vault::{self, Item};
use crate::{matcher, totp};

const LOCK_AFTER: Duration = Duration::from_secs(15 * 60);
const FILL_DELAY: Duration = Duration::from_millis(250);

pub enum Msg {
    Show,
    Status(Result<String, String>),
    Unlocked(Result<(String, Vec<Item>), String>),
    Loaded(Result<Vec<Item>, String>),
    Url(isize, String),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Checking,
    Locked,
    Ready,
    LoggedOut,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Pane {
    List,
    Fields,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Field {
    Username,
    Password,
    Totp,
}

pub enum Action {
    Fill(usize, Option<Field>),
    Copy(usize, Field),
}

impl Field {
    pub fn raw(self, item: &Item) -> &str {
        match self {
            Field::Username => &item.username,
            Field::Password => &item.password,
            Field::Totp => &item.totp,
        }
    }

    pub fn value(self, item: &Item) -> String {
        match self {
            Field::Totp => totp::current(&item.totp)
                .map(|c| c.code)
                .unwrap_or_default(),
            _ => self.raw(item).to_string(),
        }
    }

    pub fn label(self, item: &Item) -> &'static str {
        match self {
            Field::Username if item.username.contains('@') => "email",
            Field::Username => "username",
            Field::Password => "password",
            Field::Totp => "one-time password",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Field::Username => "1",
            Field::Password => "2",
            Field::Totp => "3",
        }
    }
}

pub fn fields(item: &Item) -> Vec<Field> {
    [Field::Username, Field::Password, Field::Totp]
        .into_iter()
        .filter(|f| !f.raw(item).is_empty())
        .collect()
}

pub struct App {
    egui: egui::Context,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    _hotkeys: GlobalHotKeyManager,
    session: Option<String>,
    pub(crate) items: Vec<Item>,
    pub(crate) ctx: Ctx,
    pub(crate) query: String,
    pub(crate) selected: usize,
    pub(crate) pane: Pane,
    pub(crate) field: usize,
    pub(crate) scroll_to_selected: bool,
    pub(crate) mode: Mode,
    pub(crate) busy: bool,
    pub(crate) error: String,
    visible: bool,
    had_focus: bool,
    quitting: bool,
    last_used: Instant,
    pending_fill: Option<(Vec<Part>, Instant)>,
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
            pane: Pane::List,
            field: 0,
            scroll_to_selected: false,
            mode: Mode::Checking,
            busy: false,
            error: String::new(),
            visible: true,
            had_focus: false,
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
        self.error.clear();
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
                let session = self.session.clone().unwrap_or_default();
                self.spawn(move || Msg::Loaded(vault::load(&session)));
            }
            Msg::Status(Ok(s)) if s == "unauthenticated" => self.mode = Mode::LoggedOut,
            Msg::Status(Ok(_)) => self.set_locked(""),
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
            Msg::Url(handle, url) => {
                if handle == self.ctx.handle && !url.is_empty() {
                    self.ctx.url = url;
                }
            }
        }
    }

    fn set_locked(&mut self, error: &str) {
        self.mode = Mode::Locked;
        self.items.clear();
        self.query.clear();
        self.error = error.to_string();
    }

    fn set_ready(&mut self, items: Vec<Item>) {
        self.mode = Mode::Ready;
        self.items = items;
        self.query.clear();
        self.error.clear();
        self.reset_selection();
    }

    pub(crate) fn reset_selection(&mut self) {
        self.selected = 0;
        self.pane = Pane::List;
        self.field = 0;
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
            self.reset_selection();
            self.visible = true;
            self.had_focus = false;
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

    fn hide(&mut self, refocus: bool) {
        if self.visible {
            if refocus {
                context::focus(&self.ctx);
            }
            self.visible = false;
            self.egui.send_viewport_cmd(ViewportCommand::Visible(false));
        }
    }

    fn lock(&mut self) {
        if let Some(session) = self.session.take() {
            std::thread::spawn(move || vault::lock(&session));
        }
        self.set_locked("");
    }

    pub(crate) fn unlock(&mut self) {
        if self.mode == Mode::Locked && !self.busy && !self.query.is_empty() {
            let password = std::mem::take(&mut self.query);
            self.error.clear();
            self.spawn(move || {
                Msg::Unlocked(
                    vault::unlock(&password).and_then(|s| vault::load(&s).map(|items| (s, items))),
                )
            });
        }
    }

    pub(crate) fn run(&mut self, action: Action) {
        match action {
            Action::Fill(n, field) => {
                let item = &self.items[n];
                let parts = match field {
                    None if !item.username.is_empty() => {
                        vec![
                            Part::Text(item.username.clone()),
                            Part::Tab,
                            Part::Text(item.password.clone()),
                        ]
                    }
                    None => vec![Part::Text(item.password.clone())],
                    Some(Field::Totp) => vec![Part::Totp(item.totp.clone())],
                    Some(f) => vec![Part::Text(f.value(item))],
                };
                self.pending_fill = Some((parts, Instant::now() + FILL_DELAY));
                self.hide(true);
                self.egui.request_repaint_after(FILL_DELAY);
            }
            Action::Copy(n, field) => {
                let value = field.value(&self.items[n]);
                if value.is_empty() {
                    self.error = format!("No {}", field.label(&self.items[n]));
                    return;
                }
                match output::copy_secret(value) {
                    Ok(()) => self.hide(true),
                    Err(e) => self.error = e,
                }
            }
        }
    }

    pub(crate) fn results(&self) -> Vec<(u8, usize)> {
        if self.mode == Mode::Ready {
            matcher::search(&self.items, &self.query, &self.ctx)
        } else {
            Vec::new()
        }
    }

    pub(crate) fn current(&self, results: &[(u8, usize)]) -> Option<usize> {
        results.get(self.selected).map(|&(_, n)| n)
    }

    fn keys(&mut self, ctx: &egui::Context, results: &[(u8, usize)]) {
        let pressed = ctx.input_mut(|i| {
            let copy = i.events.iter().any(|e| matches!(e, Event::Copy));
            i.events.retain(|e| !matches!(e, Event::Copy));
            [
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
            if let Some(session) = self.session.take() {
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
        if self.mode == Mode::Locked && (enter || shift_enter) {
            self.unlock();
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

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle(msg);
        }
        if let Some((_, at)) = &self.pending_fill {
            let now = Instant::now();
            if now >= *at {
                if let Some((parts, _)) = self.pending_fill.take()
                    && let Err(e) = output::type_parts(&parts)
                {
                    self.error = e;
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
        if self.visible {
            let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
            if self.had_focus && !focused && self.pending_fill.is_none() {
                self.hide(false);
            }
            self.had_focus = focused;
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.hide(true);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let results = self.results();
        self.selected = self.selected.min(results.len().saturating_sub(1));
        self.keys(ui.ctx(), &results);
        if self.visible {
            self.draw(ui, &results);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }
}
