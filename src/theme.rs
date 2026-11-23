//! Dark creative-tool look: near-black canvas, soft charcoal panels,
//! rounded controls and a single bright accent for primary actions.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const BG: Color32 = Color32::from_rgb(12, 12, 15);
pub const PANEL: Color32 = Color32::from_rgb(22, 22, 27);
pub const PANEL_RAISED: Color32 = Color32::from_rgb(31, 31, 38);
pub const BORDER: Color32 = Color32::from_rgb(44, 44, 54);
pub const TEXT: Color32 = Color32::from_rgb(232, 232, 238);
pub const TEXT_DIM: Color32 = Color32::from_rgb(140, 140, 156);
pub const ACCENT: Color32 = Color32::from_rgb(64, 132, 255);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(38, 66, 120);
pub const MASK_TINT: Color32 = Color32::from_rgb(255, 94, 160);
pub const ARROW: Color32 = Color32::from_rgb(255, 214, 92);
pub const PIN: Color32 = Color32::from_rgb(120, 230, 170);
pub const DANGER: Color32 = Color32::from_rgb(240, 96, 96);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = PANEL;
        v.window_fill = PANEL_RAISED;
        v.extreme_bg_color = BG;
        v.faint_bg_color = PANEL_RAISED;
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = ACCENT_SOFT;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.window_corner_radius = CornerRadius::same(12);
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.hyperlink_color = ACCENT;
        v.slider_trailing_fill = true;

        let r = CornerRadius::same(8);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = PANEL;
        w.noninteractive.weak_bg_fill = PANEL;
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
        w.noninteractive.corner_radius = r;

        // bg_fill is the slider track and checkbox box; keep it visible on cards.
        w.inactive.bg_fill = Color32::from_rgb(56, 56, 70);
        w.inactive.weak_bg_fill = PANEL_RAISED;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = r;

        w.hovered.bg_fill = Color32::from_rgb(42, 42, 52);
        w.hovered.weak_bg_fill = Color32::from_rgb(42, 42, 52);
        w.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(70, 70, 86));
        w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
        w.hovered.corner_radius = r;

        w.active.bg_fill = ACCENT_SOFT;
        w.active.weak_bg_fill = ACCENT_SOFT;
        w.active.bg_stroke = Stroke::new(1.0, ACCENT);
        w.active.fg_stroke = Stroke::new(1.0, TEXT);
        w.active.corner_radius = r;

        w.open = w.active.clone();

        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.slider_width = 84.0;
        style.spacing.interact_size.y = 26.0;

        use egui::{FontFamily::Proportional, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(17.0, Proportional)),
            (TextStyle::Body, FontId::new(13.5, Proportional)),
            (TextStyle::Button, FontId::new(13.5, Proportional)),
            (TextStyle::Small, FontId::new(11.0, Proportional)),
            (TextStyle::Monospace, FontId::new(12.0, egui::FontFamily::Monospace)),
        ]
        .into();
    });
}

/// A rounded card used to group controls in the side panel.
pub fn card() -> egui::Frame {
    egui::Frame::default()
        .fill(PANEL_RAISED)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(12))
}

/// Small gray caption above a group of controls.
pub fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .size(11.0)
            .color(TEXT_DIM)
            .strong(),
    );
}

/// The large accent button used for the main action.
pub fn primary_button(
    ui: &mut egui::Ui,
    icon: Option<crate::icons::Icon>,
    text: &str,
    enabled: bool,
) -> egui::Response {
    let fill = if enabled { ACCENT } else { ACCENT_SOFT };
    let label = egui::RichText::new(text).size(15.0).strong().color(Color32::WHITE);
    let button = match icon {
        Some(i) => egui::Button::image_and_text(i.image(18.0, Color32::WHITE), label),
        None => egui::Button::new(label),
    }
    .fill(fill)
    .corner_radius(CornerRadius::same(10))
    .min_size(egui::vec2(ui.available_width(), 42.0));
    ui.add_enabled(enabled, button)
}

/// Place a widget on top of something already drawn (for example a small
/// button on a thumbnail) without moving where the next item goes.
pub fn overlay(ui: &mut egui::Ui, rect: egui::Rect, widget: impl egui::Widget) -> egui::Response {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    child.add(widget)
}
