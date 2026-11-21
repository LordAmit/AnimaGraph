//! Animagraph: turn a still photo or a short video into a looping
//! animation. Paint what moves (or what freezes), set the direction of
//! motion, then render a seamless loop on this computer.

mod app;
mod applelog;
mod automask;
mod border;
mod canvas;
mod caption;
mod colour;
mod corners;
mod crop;
mod icons;
mod layout;
mod layout_view;
#[cfg(test)]
mod lr_match;
#[cfg(target_os = "macos")]
mod macvideo;
mod mcp_access;
mod mcp_live;
mod mcp_server;
mod mcp_tools;
mod mcp_window;
#[cfg(target_os = "macos")]
mod macexport;
mod develop;
mod export;
mod files_panel;
mod film;
mod finish;
mod fonts;
mod motion;
mod palette;
mod panels;
mod preset_dialog;
mod presets;
mod raw;
mod render;
mod sample;
mod scene;
mod scope;
mod settings;
mod stabilize;
mod straighten;
mod theme;
mod video;

use eframe::egui;

fn main() -> eframe::Result<()> {
    // `--write-iconset <folder>`: write the app icon images and quit
    // (used by make-mac-app.sh to build the .app icon).
    let args: Vec<String> = std::env::args().collect();
    // `--mcp`: background mode for assistants (local MCP server, no window).
    if args.iter().any(|a| a == "--mcp") {
        std::process::exit(mcp_server::run());
    }
    if let Some(i) = args.iter().position(|a| a == "--write-iconset") {
        let Some(dir) = args.get(i + 1) else {
            eprintln!("Usage: --write-iconset <folder>");
            std::process::exit(2);
        };
        match icons::write_iconset(std::path::Path::new(dir)) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("Could not write the icons: {e}");
                std::process::exit(1);
            }
        }
    }
    let screenshot = parse_screenshot_arg();
    let open = parse_open_args();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Animagraph")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([980.0, 640.0]);
    if let Some(icon) = icons::app_icon() {
        viewport = viewport.with_icon(icon);
    }

    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = &mut options;

    eframe::run_native(
        "Animagraph",
        options,
        Box::new(move |cc| Ok(Box::new(app::StudioApp::new(cc, screenshot, open)))),
    )
}

/// Reads `--screenshot <file.png>` from the command line.
fn parse_screenshot_arg() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--screenshot" {
            return args.next().map(std::path::PathBuf::from);
        }
        if let Some(rest) = arg.strip_prefix("--screenshot=") {
            return Some(std::path::PathBuf::from(rest));
        }
    }
    None
}

/// Files named on the command line are opened at start (photo, video, or
/// several photos as a clip).
fn parse_open_args() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--screenshot" {
            args.next();
        } else if !arg.starts_with("--") {
            out.push(std::path::PathBuf::from(arg));
        }
    }
    out
}
