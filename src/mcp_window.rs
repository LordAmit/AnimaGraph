//! "Assistant access (MCP)" window: which folders an assistant may use, and
//! the lines to paste into Claude Code or Claude Desktop to connect it.

use eframe::egui::{self, Color32, Vec2};

use crate::app::StudioApp;
use crate::theme;

/// Marks "no folders allowed" (an empty list means the default folders).
const NONE: &str = "(none)";

/// The command an assistant runs to start Animagraph in background mode.
fn exe_path() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "animagraph".into())
}

pub fn claude_code_command() -> String {
    format!("claude mcp add animagraph -- \"{}\" --mcp", exe_path())
}

pub fn claude_desktop_json() -> String {
    let exe = serde_json::to_string(&exe_path()).unwrap_or_else(|_| "\"animagraph\"".into());
    format!("{{\n  \"mcpServers\": {{\n    \"animagraph\": {{\n      \"command\": {exe},\n      \"args\": [\"--mcp\"]\n    }}\n  }}\n}}")
}

pub fn window(app: &mut StudioApp, ctx: &egui::Context) {
    if !app.mcp_window_open {
        return;
    }
    let mut close = false;
    // A dialog above everything else (the floating canvas bars included).
    let modal = egui::Modal::new(egui::Id::unique("mcp_window")).show(ctx, |ui| {
            ui.set_width(540.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(egui::RichText::new("Assistant access (MCP)").size(16.0).strong());
            ui.label(
                egui::RichText::new(
                    "An assistant such as Claude can open, style, frame and save photos with Animagraph in the \
                     background (no window). It runs on this computer only and can use only the folders below.",
                )
                .color(theme::TEXT_DIM),
            );

            ui.checkbox(&mut app.prefs.mcp_live, "Let the assistant work in this window (live)")
                .on_hover_text(
                    "On: while Animagraph is open, the assistant opens and changes files here and you see each change \
                     (undo works as usual). Off: it always works in the background.",
                );
            ui.label(egui::RichText::new("Allowed folders").strong());
            let defaults = app.prefs.mcp_folders.is_empty();
            let shown: Vec<String> = crate::mcp_access::allowed_folders(&app.prefs)
                .iter()
                .map(|p| p.display().to_string())
                .filter(|s| s != NONE)
                .collect();
            let mut remove = None;
            for (i, f) in shown.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(f).monospace().size(12.0));
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if shown.is_empty() {
                ui.label(egui::RichText::new("No folders: the assistant cannot open or save anything.").color(theme::TEXT_DIM));
            }
            if let Some(i) = remove {
                // Removing from the defaults turns them into a list of its own.
                let mut list = shown.clone();
                list.remove(i);
                // An empty list means "the defaults", so "none" is kept as a marker.
                app.prefs.mcp_folders = if list.is_empty() { vec![NONE.into()] } else { list };
            }
            ui.horizontal(|ui| {
                if ui.button("Add folder…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().set_title("Allow a folder for the assistant").pick_folder() {
                        let mut list = if defaults { shown.clone() } else { app.prefs.mcp_folders.clone() };
                        list.retain(|s| s != NONE);
                        let d = dir.display().to_string();
                        if !list.contains(&d) {
                            list.push(d);
                        }
                        app.prefs.mcp_folders = list;
                    }
                }
                if !defaults && ui.button("Use default folders").on_hover_text("Pictures, Desktop and Downloads").clicked() {
                    app.prefs.mcp_folders.clear();
                }
            });
            ui.label(
                egui::RichText::new("Changes apply the next time the assistant starts Animagraph.")
                    .size(11.5)
                    .color(theme::TEXT_DIM),
            );

            ui.separator();
            ui.label(egui::RichText::new("Connect Claude Code").strong());
            let cmd = claude_code_command();
            code_box(ui, &cmd);
            if ui.button("Copy command").clicked() {
                ctx.copy_text(cmd.clone());
                app.status = "Copied the Claude Code command.".into();
            }
            ui.label(egui::RichText::new("Connect Claude Desktop").strong());
            ui.label(
                egui::RichText::new("Settings > Developer > Edit Config, then add this to claude_desktop_config.json:")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            );
            let js = claude_desktop_json();
            code_box(ui, &js);
            if ui.button("Copy settings").clicked() {
                ctx.copy_text(js.clone());
                app.status = "Copied the Claude Desktop settings.".into();
            }
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let done = egui::Button::new(egui::RichText::new("Close").color(Color32::WHITE).strong())
                    .fill(theme::ACCENT)
                    .min_size(Vec2::new(90.0, 30.0));
                if ui.add(done).clicked() {
                    close = true;
                }
            });
        });
    // Keep the dialog above bars that appear while it is open (a loop finishing).
    ctx.move_to_top(modal.backdrop_response.layer_id);
    ctx.move_to_top(modal.response.layer_id);
    if close || modal.should_close() {
        app.mcp_window_open = false;
    }
}

fn code_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::default()
        .fill(Color32::from_rgb(14, 14, 18))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(text).monospace().size(12.0));
        });
}
