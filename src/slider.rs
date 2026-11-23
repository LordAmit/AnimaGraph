//! How slider values are shown and typed: up to two digits after the decimal
//! point. Only the text is rounded; a value that is not touched (from a
//! preset, for example) keeps its full precision.

use eframe::egui::{self, Slider};

pub trait SliderDigits: Sized {
    /// Shown as it is stored.
    fn two_digits(self) -> Self {
        self.two_digits_of(1.0)
    }

    /// A fraction shown in percent.
    fn percent(self) -> Self;

    /// Shown as value × `scale`.
    fn two_digits_of(self, scale: f64) -> Self;

    /// A frame number shown and typed in seconds.
    fn frame_seconds(self, fps: f32) -> Self;
}

impl SliderDigits for Slider<'_> {
    fn percent(self) -> Self {
        self.two_digits_of(100.0).suffix("%")
    }

    fn two_digits_of(self, scale: f64) -> Self {
        self.custom_formatter(move |v, _| text(v * scale)).custom_parser(move |t| parse(t).map(|v| v / scale))
    }

    fn frame_seconds(self, fps: f32) -> Self {
        let fps = fps as f64;
        self.custom_formatter(move |v, _| format!("{:.2}", v / fps))
            .custom_parser(move |t| number(t).map(|s| (s * fps).round()))
            .suffix("s")
    }
}

/// Make a row's number boxes wide enough for "-99.99", so the label after
/// them stays in place while the number changes.
pub fn steady(ui: &mut egui::Ui) {
    ui.spacing_mut().interact_size.x = 62.0;
}

/// At most two digits after the point, without zeros at the end.
pub fn text(v: f64) -> String {
    let t = format!("{v:.2}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".into() } else { t.into() }
}

/// A typed number, rounded to two digits.
fn parse(t: &str) -> Option<f64> {
    number(t).map(|v| (v * 100.0).round() / 100.0)
}

/// A typed number; a unit after it, a decimal comma and a typographic minus
/// are understood.
fn number(t: &str) -> Option<f64> {
    let t: String = t.replace(',', ".").replace('−', "-").split_whitespace().collect();
    t.trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.').parse().ok().filter(|v: &f64| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_two_digits_are_shown() {
        for (v, t) in [(0.0, "0"), (12.0, "12"), (12.5, "12.5"), (12.346, "12.35"), (-0.001, "0"), (-7.25, "-7.25"), (100.0, "100"), (0.05, "0.05")] {
            assert_eq!(text(v), t);
        }
    }

    #[test]
    fn typed_numbers_keep_two_digits() {
        for (t, v) in [("12.34", 12.34), ("12.345678", 12.35), ("12,5", 12.5), (" -3.1 % ", -3.1), ("−2", -2.0), ("0.25×", 0.25), ("7°", 7.0), ("1.5 s", 1.5), (".5", 0.5)] {
            assert_eq!(parse(t), Some(v), "{t}");
        }
        for t in ["", "abc", "%", "1.2.3"] {
            assert_eq!(parse(t), None, "{t}");
        }
    }

    #[test]
    fn what_is_shown_can_be_typed_back() {
        for scale in [1.0, 100.0, 200.0] {
            for i in -10000..=10000 {
                let shown = i as f64 / 100.0;
                // The sliders store 32-bit values.
                let stored = (parse(&text(shown)).unwrap() / scale) as f32;
                assert_eq!(text(stored as f64 * scale), text(shown));
            }
        }
    }

    /// One drawn frame of `add` with these events.
    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, add: &mut dyn FnMut(&mut egui::Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 200.0))),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| add(ui));
        out.textures_delta.clear();
    }

    /// Click the slider's number box, type `typed` and press Enter.
    fn type_into(typed: &str, add: &mut dyn FnMut(&mut egui::Ui) -> egui::Rect) {
        use egui::{Event, Key, Modifiers, PointerButton, vec2};
        let ctx = egui::Context::default();
        let mut rect = egui::Rect::NOTHING;
        let mut draw = |ui: &mut egui::Ui| rect = add(ui);
        frame(&ctx, vec![], &mut draw);
        frame(&ctx, vec![], &mut draw);
        // The number box comes right after the rail.
        let pos = rect.left_center() + vec2(ctx.global_style().spacing.slider_width + 20.0, 0.0);
        let mut draw = |ui: &mut egui::Ui| _ = add(ui);
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        frame(&ctx, vec![Event::PointerMoved(pos)], &mut draw);
        frame(&ctx, vec![button(true)], &mut draw);
        frame(&ctx, vec![button(false)], &mut draw);
        frame(&ctx, vec![], &mut draw);
        frame(&ctx, vec![Event::Text(typed.into())], &mut draw);
        let enter = Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
        frame(&ctx, vec![enter], &mut draw);
        frame(&ctx, vec![], &mut draw);
    }

    #[test]
    fn typing_into_a_slider_keeps_two_digits() {
        let mut v = 0.0f32;
        type_into("12.34", &mut |ui| ui.add(Slider::new(&mut v, -1.0..=1.0).two_digits_of(100.0)).rect);
        assert!((v - 0.1234).abs() < 1e-6, "{v}");
        let mut v = 0.0f32;
        type_into("6.25", &mut |ui| ui.add(Slider::new(&mut v, 0.0..=0.25).percent()).rect);
        assert!((v - 0.0625).abs() < 1e-6, "{v}");
        // 1.25 seconds at 24 frames a second.
        let mut v = 0usize;
        type_into("1.25", &mut |ui| ui.add(Slider::new(&mut v, 0..=100).frame_seconds(24.0)).rect);
        assert_eq!(v, 30);
    }

    #[test]
    fn drawing_a_slider_leaves_its_value_alone() {
        let mut v = 0.123_456_78f32;
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&ctx, vec![], &mut |ui| drop(ui.add(Slider::new(&mut v, -1.0..=1.0).two_digits_of(100.0))));
        }
        assert_eq!(v, 0.123_456_78);
    }
}
