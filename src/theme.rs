use eframe::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub selected: Color32,
    pub hover: Color32,
    pub field: Color32,
    pub danger: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: Color32::from_rgb(0x1c, 0x1d, 0x22),
            panel: Color32::from_rgb(0x23, 0x24, 0x2a),
            border: Color32::from_rgb(0x34, 0x36, 0x3e),
            text: Color32::from_rgb(0xec, 0xed, 0xf1),
            muted: Color32::from_rgb(0x95, 0x98, 0xa3),
            accent: Color32::from_rgb(0x4c, 0x8d, 0xff),
            accent_soft: Color32::from_rgb(0x26, 0x34, 0x52),
            selected: Color32::from_rgb(0x2b, 0x3a, 0x5c),
            hover: Color32::from_rgb(0x29, 0x2a, 0x31),
            field: Color32::from_rgb(0x2b, 0x2c, 0x33),
            danger: Color32::from_rgb(0xff, 0x7a, 0x7a),
        }
    } else {
        Palette {
            bg: Color32::from_rgb(0xff, 0xff, 0xff),
            panel: Color32::from_rgb(0xf6, 0xf7, 0xf9),
            border: Color32::from_rgb(0xe2, 0xe4, 0xe9),
            text: Color32::from_rgb(0x1b, 0x1c, 0x21),
            muted: Color32::from_rgb(0x6a, 0x6d, 0x77),
            accent: Color32::from_rgb(0x1f, 0x6f, 0xeb),
            accent_soft: Color32::from_rgb(0xe4, 0xee, 0xff),
            selected: Color32::from_rgb(0xe1, 0xeb, 0xfd),
            hover: Color32::from_rgb(0xf1, 0xf2, 0xf5),
            field: Color32::from_rgb(0xff, 0xff, 0xff),
            danger: Color32::from_rgb(0xd1, 0x24, 0x2f),
        }
    }
}

const AVATAR_COLORS: [Color32; 8] = [
    Color32::from_rgb(0x4c, 0x8d, 0xff),
    Color32::from_rgb(0x8b, 0x5c, 0xf6),
    Color32::from_rgb(0xec, 0x48, 0x99),
    Color32::from_rgb(0xf9, 0x73, 0x16),
    Color32::from_rgb(0x10, 0xb9, 0x81),
    Color32::from_rgb(0x06, 0xb6, 0xd4),
    Color32::from_rgb(0xea, 0xb3, 0x08),
    Color32::from_rgb(0xef, 0x44, 0x44),
];

pub fn avatar(p: &Painter, rect: Rect, name: &str) {
    let hash = name
        .bytes()
        .fold(5381u32, |h, b| h.wrapping_mul(33) ^ u32::from(b));
    let color = AVATAR_COLORS[hash as usize % AVATAR_COLORS.len()];
    p.rect_filled(rect, rect.width() * 0.28, color);
    let letter: String = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map_or("?".into(), |c| c.to_uppercase().collect());
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        letter,
        FontId::proportional(rect.height() * 0.48),
        Color32::WHITE,
    );
}

pub fn search_icon(p: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(2.0, color);
    let c = center + vec2(-2.0, -2.0);
    p.circle_stroke(c, 6.5, stroke);
    p.line_segment([c + vec2(4.8, 4.8), c + vec2(9.5, 9.5)], stroke);
}

pub fn lock_icon(p: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(2.4, color);
    let body = Rect::from_center_size(center + vec2(0.0, 5.0), vec2(22.0, 17.0));
    p.rect_filled(body, 4.0, color);
    let points: Vec<Pos2> = (0..=16)
        .map(|i| {
            let a = std::f32::consts::PI * (1.0 + i as f32 / 16.0);
            pos2(center.x + 7.0 * a.cos(), body.top() - 5.0 + 7.0 * a.sin())
        })
        .collect();
    p.add(Shape::line(points, stroke));
    p.line_segment(
        [
            pos2(center.x - 7.0, body.top() - 5.0),
            pos2(center.x - 7.0, body.top()),
        ],
        stroke,
    );
    p.line_segment(
        [
            pos2(center.x + 7.0, body.top() - 5.0),
            pos2(center.x + 7.0, body.top()),
        ],
        stroke,
    );
}

pub fn ring(p: &Painter, center: Pos2, radius: f32, fraction: f32, color: Color32, track: Color32) {
    p.circle_stroke(center, radius, Stroke::new(2.5, track));
    let steps = 40;
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2
                + std::f32::consts::TAU * fraction * i as f32 / steps as f32;
            center + radius * Vec2::angled(a)
        })
        .collect();
    p.add(Shape::line(points, Stroke::new(2.5, color)));
}

pub fn keycap(ui: &mut Ui, key: &str, action: &str, pal: Palette) {
    let font = FontId::proportional(11.5);
    let key_galley = ui
        .painter()
        .layout_no_wrap(key.into(), font.clone(), pal.text);
    let action_galley = ui.painter().layout_no_wrap(action.into(), font, pal.muted);
    let size = vec2(
        key_galley.size().x + 10.0 + 5.0 + action_galley.size().x,
        20.0,
    );
    let (rect, _) = ui.allocate_exact_size(size, eframe::egui::Sense::hover());
    let cap = Rect::from_min_size(rect.min, vec2(key_galley.size().x + 10.0, 20.0));
    ui.painter().rect(
        cap,
        5.0,
        pal.field,
        Stroke::new(1.0, pal.border),
        StrokeKind::Inside,
    );
    ui.painter()
        .galley(cap.center() - key_galley.size() / 2.0, key_galley, pal.text);
    let at = pos2(
        cap.right() + 5.0,
        rect.center().y - action_galley.size().y / 2.0,
    );
    ui.painter().galley(at, action_galley, pal.muted);
}

pub fn elide(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!(
            "{}…",
            text.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}
