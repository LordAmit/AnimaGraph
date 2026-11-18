//! SVG icons bundled into the program. Each icon is drawn with white
//! strokes and tinted to the color it is shown in.

use eframe::egui::{self, Color32, ImageSource, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Box,
    Brush,
    Eraser,
    Arrow,
    Pin,
    Hand,
    Undo,
    Reset,
    SidebarRight,
    Layout,
    Redo,
    Fit,
    Edit,
    Play,
    Pause,
    Image,
    Download,
    Snow,
    Sparkle,
    Loop,
    Plus,
    Trash,
    Film,
    Sidebar,
    Bounce,
    ZoomIn,
    Crop,
    Eyedropper,
    Pointer,
    ChevronDown,
    ChevronRight,
    RotateLeft,
    RotateRight,
    ZoomOut,
    Logo,
}

impl Icon {
    pub fn source(self) -> ImageSource<'static> {
        match self {
            Icon::Box => egui::include_image!("../assets/icons/box.svg"),
            Icon::Brush => egui::include_image!("../assets/icons/brush.svg"),
            Icon::Eraser => egui::include_image!("../assets/icons/eraser.svg"),
            Icon::Arrow => egui::include_image!("../assets/icons/arrow.svg"),
            Icon::Pin => egui::include_image!("../assets/icons/pin.svg"),
            Icon::Hand => egui::include_image!("../assets/icons/hand.svg"),
            Icon::Undo => egui::include_image!("../assets/icons/undo.svg"),
            Icon::Reset => egui::include_image!("../assets/icons/reset.svg"),
            Icon::SidebarRight => egui::include_image!("../assets/icons/sidebar_right.svg"),
            Icon::Layout => egui::include_image!("../assets/icons/layout.svg"),
            Icon::Redo => egui::include_image!("../assets/icons/redo.svg"),
            Icon::Fit => egui::include_image!("../assets/icons/fit.svg"),
            Icon::Edit => egui::include_image!("../assets/icons/edit.svg"),
            Icon::Play => egui::include_image!("../assets/icons/play.svg"),
            Icon::Pause => egui::include_image!("../assets/icons/pause.svg"),
            Icon::Image => egui::include_image!("../assets/icons/image.svg"),
            Icon::Download => egui::include_image!("../assets/icons/download.svg"),
            Icon::Snow => egui::include_image!("../assets/icons/snow.svg"),
            Icon::Sparkle => egui::include_image!("../assets/icons/sparkle.svg"),
            Icon::Loop => egui::include_image!("../assets/icons/loop.svg"),
            Icon::Plus => egui::include_image!("../assets/icons/plus.svg"),
            Icon::Trash => egui::include_image!("../assets/icons/trash.svg"),
            Icon::Film => egui::include_image!("../assets/icons/film.svg"),
            Icon::Sidebar => egui::include_image!("../assets/icons/sidebar.svg"),
            Icon::Bounce => egui::include_image!("../assets/icons/bounce.svg"),
            Icon::ZoomIn => egui::include_image!("../assets/icons/zoom_in.svg"),
            Icon::ZoomOut => egui::include_image!("../assets/icons/zoom_out.svg"),
            Icon::Crop => egui::include_image!("../assets/icons/crop.svg"),
            Icon::RotateLeft => egui::include_image!("../assets/icons/rotate_left.svg"),
            Icon::RotateRight => egui::include_image!("../assets/icons/rotate_right.svg"),
            Icon::ChevronDown => egui::include_image!("../assets/icons/chevron_down.svg"),
            Icon::ChevronRight => egui::include_image!("../assets/icons/chevron_right.svg"),
            Icon::Pointer => egui::include_image!("../assets/icons/pointer.svg"),
            Icon::Eyedropper => egui::include_image!("../assets/icons/eyedropper.svg"),
            Icon::Logo => egui::include_image!("../assets/icons/logo.svg"),
        }
    }

    /// The icon at `size` points, tinted with `color` (the logo keeps its own colors).
    pub fn image(self, size: f32, color: Color32) -> egui::Image<'static> {
        let img = egui::Image::new(self.source()).fit_to_exact_size(Vec2::splat(size));
        if self == Icon::Logo { img } else { img.tint(color) }
    }
}

/// A button with an icon in front of its text.
pub fn button(icon: Icon, text: impl Into<egui::WidgetText>, color: Color32) -> egui::Button<'static> {
    egui::Button::image_and_text(icon.image(15.0, color), text)
}

/// The logo drawn at `size` × `size` pixels (RGBA, unmultiplied alpha).
pub fn logo_rgba(size: u32) -> Option<Vec<u8>> {
    let svg = include_bytes!("../assets/icons/logo.svg");
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect(),
    )
}

/// The logo as the window and Dock icon (256 × 256).
pub fn app_icon() -> Option<egui::IconData> {
    let size = 256u32;
    Some(egui::IconData { rgba: logo_rgba(size)?, width: size, height: size })
}

/// Write the logo at every size a macOS app icon needs, named the way
/// `iconutil` expects (`icon_16x16.png`, `icon_16x16@2x.png`, ...).
pub fn write_iconset(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for base in [16u32, 32, 128, 256, 512] {
        for (scale, suffix) in [(1u32, ""), (2, "@2x")] {
            let px = base * scale;
            let rgba = logo_rgba(px).ok_or("Could not draw the logo")?;
            let img = image::RgbaImage::from_raw(px, px, rgba).ok_or("Bad logo size")?;
            img.save(dir.join(format!("icon_{base}x{base}{suffix}.png"))).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn iconset_has_every_size() {
        let dir = std::env::temp_dir().join(format!("cg_iconset_{}", std::process::id()));
        super::write_iconset(&dir).unwrap();
        let n = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(n, 10);
        let big = image::open(dir.join("icon_512x512@2x.png")).unwrap();
        assert_eq!((big.width(), big.height()), (1024, 1024));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn app_icon_renders() {
        let icon = super::app_icon().expect("logo renders");
        assert_eq!((icon.width, icon.height), (256, 256));
        // The tile is opaque in the middle and see-through in the corner.
        let px = |x: usize, y: usize| icon.rgba[(y * 256 + x) * 4 + 3];
        assert_eq!(px(128, 128), 255);
        assert_eq!(px(0, 0), 0);
    }

    /// Writes the icon for a manual look: `ICON_PNG=path cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn write_icon_png() {
        let Ok(path) = std::env::var("ICON_PNG") else { return };
        let icon = super::app_icon().unwrap();
        image::RgbaImage::from_raw(icon.width, icon.height, icon.rgba).unwrap().save(path).unwrap();
    }
}
