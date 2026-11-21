//! The tools an assistant can call through MCP, working on photos without
//! a window: open, look at the styles, apply a style or adjustments, set
//! framing (rotation, straighten, crop, border, corners, caption), preview,
//! save and close. Every photo keeps its settings until it is closed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::border::{Border, BorderFit};
use crate::caption::{Align, Area, Place};
use crate::crop::{Aspect, CropRect};
use crate::export::Format;
use crate::scene::{Edits, Scene};

/// Where the tools find photos: the background session, or the open
/// Animagraph window (live mode, see mcp_live).
pub trait Workspace {
    /// "background" or "live".
    fn mode(&self) -> &'static str;
    fn folders(&self) -> Vec<PathBuf>;
    /// Take a newly read photo; returns its id.
    fn add_image(&mut self, path: PathBuf, name: String, img: image::RgbaImage) -> String;
    fn list(&self) -> Value;
    /// Change one photo: its picture and framing (scene) and its edits.
    fn edit(&mut self, id: &str, f: &mut dyn FnMut(&mut Scene, &mut Edits) -> Result<Value, String>) -> Result<Value, String>;
    fn close(&mut self, id: &str) -> Result<Value, String>;
    /// The list of imported presets changed (the window shows the new list).
    fn presets_changed(&mut self) {}
}

/// One photo opened by the assistant in the background.
struct Doc {
    path: PathBuf,
    scene: Scene,
    edits: Edits,
}

/// Photos open in a background MCP session, and the folders it may use.
pub struct Session {
    docs: BTreeMap<String, Doc>,
    next: u32,
    folders: Vec<PathBuf>,
}

/// What a tool gives back: text, and optionally a PNG picture.
pub struct Reply {
    pub text: String,
    pub png: Option<Vec<u8>>,
    pub is_error: bool,
}

impl Reply {
    fn ok(v: Value) -> Self {
        Self { text: serde_json::to_string_pretty(&v).unwrap_or_default(), png: None, is_error: false }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self { text: msg.into(), png: None, is_error: true }
    }
}

impl Session {
    pub fn new(folders: Vec<PathBuf>) -> Self {
        Self { docs: BTreeMap::new(), next: 1, folders }
    }

    pub fn call(&mut self, name: &str, args: &Value) -> Reply {
        call(self, name, args)
    }
}

impl Workspace for Session {
    fn mode(&self) -> &'static str {
        "background"
    }
    fn folders(&self) -> Vec<PathBuf> {
        self.folders.clone()
    }
    fn add_image(&mut self, path: PathBuf, name: String, img: image::RgbaImage) -> String {
        let id = format!("img{}", self.next);
        self.next += 1;
        self.docs.insert(id.clone(), Doc { path, scene: Scene::new(name, img), edits: Edits::default() });
        id
    }
    fn list(&self) -> Value {
        let list: Vec<Value> = self
            .docs
            .iter()
            .map(|(id, d)| json!({"image_id": id, "name": d.scene.name, "path": d.path.display().to_string(), "width": d.scene.full.width(), "height": d.scene.full.height()}))
            .collect();
        json!({ "mode": "background", "images": list })
    }
    fn edit(&mut self, id: &str, f: &mut dyn FnMut(&mut Scene, &mut Edits) -> Result<Value, String>) -> Result<Value, String> {
        let d = self.docs.get_mut(id).ok_or_else(|| format!("No open image with id {id}. Use open_image or list_open_images."))?;
        f(&mut d.scene, &mut d.edits)
    }
    fn close(&mut self, id: &str) -> Result<Value, String> {
        self.docs.remove(id).map(|_| json!({"closed": id})).ok_or_else(|| format!("No open image with id {id}."))
    }
}

/// Run one tool by name in a workspace.
pub fn call(ws: &mut dyn Workspace, name: &str, args: &Value) -> Reply {
    if let Some(r) = crate::mcp_masks::call(ws, name, args) {
        return match r {
            Ok(v) => Reply::ok(v),
            Err(e) => Reply::error(e),
        };
    }
    let result = match name {
        "allowed_folders" => Ok(json!({
            "mode": ws.mode(),
            "folders": ws.folders().iter().map(|f| f.display().to_string()).collect::<Vec<_>>(),
        })),
        "open_image" => open_image(ws, args),
        "list_open_images" => Ok(ws.list()),
        "list_styles" => Ok(list_styles()),
        "apply_style" => apply_style(ws, args),
        "set_adjustments" => set_adjustments(ws, args),
        "reset_adjustments" => with_doc(ws, args, &mut |_, e| {
            e.look = crate::finish::Look::default();
            Ok(json!({"reset": true}))
        }),
        "set_framing" => set_framing(ws, args),
        "get_settings" => with_doc(ws, args, &mut |s, e| Ok(settings_json(s, e))),
        "preview" => return preview(ws, args),
        "save_image" => save_image(ws, args),
        "close_image" => str_arg(args, "image_id").and_then(|id| ws.close(&id)),
        "import_preset" => import_preset(ws, args),
        _ => Err(format!("Unknown tool: {name}")),
    };
    match result {
        Ok(v) => Reply::ok(v),
        Err(e) => Reply::error(e),
    }
}

fn with_doc(
    ws: &mut dyn Workspace,
    args: &Value,
    f: &mut dyn FnMut(&mut Scene, &mut Edits) -> Result<Value, String>,
) -> Result<Value, String> {
    let id = str_arg(args, "image_id")?;
    ws.edit(&id, f)
}

/// Copy a Lightroom preset (.xmp, or a .dng preset from Lightroom mobile)
/// from an allowed folder into Animagraph's presets, as the Import button does.
fn import_preset(ws: &mut dyn Workspace, args: &Value) -> Result<Value, String> {
    let path = PathBuf::from(str_arg(args, "path")?);
    let real = crate::mcp_access::check_read(&path, &ws.folders())?;
    let p = crate::presets::import(&real)?;
    ws.presets_changed();
    Ok(json!({"imported": p.name, "use_with": "apply_style"}))
}

/// Read a photo from an allowed folder (JPEG, PNG, HEIC, RAW...).
pub fn read_photo(path: &Path, folders: &[PathBuf]) -> Result<(PathBuf, String, image::RgbaImage), String> {
    let real = crate::mcp_access::check_read(path, folders)?;
    let img = if crate::raw::needs_system_decoder(&real) {
        crate::raw::decode(&real)?
    } else {
        crate::app::open_upright(&real).map_err(|e| format!("Cannot read {}: {e}", real.display()))?.to_rgba8()
    };
    let name = real.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Photo".into());
    Ok((real, name, img))
}

fn open_image(ws: &mut dyn Workspace, args: &Value) -> Result<Value, String> {
    let path = PathBuf::from(str_arg(args, "path")?);
    let (real, name, img) = read_photo(&path, &ws.folders())?;
    let (w, h) = img.dimensions();
    let id = ws.add_image(real, name.clone(), img);
    Ok(json!({"image_id": id, "name": name, "width": w, "height": h, "mode": ws.mode()}))
}

fn apply_style(ws: &mut dyn Workspace, args: &Value) -> Result<Value, String> {
    let name = str_arg(args, "name")?;
    let strength = args.get("strength").and_then(Value::as_f64).map_or(1.0, |v| (v as f32 / 100.0).clamp(0.0, 1.0));
    let wanted = name.to_lowercase();
    let (look, kind) = if let Some(p) = crate::presets::list().into_iter().find(|p| p.name.to_lowercase() == wanted) {
        (p.look, "Lightroom preset")
    } else if let Some((_, l)) = crate::finish::Look::presets().into_iter().find(|(n, _)| n.to_lowercase() == wanted) {
        (l, "look")
    } else if let Some(f) = crate::film::all().into_iter().find(|f| f.name.to_lowercase() == wanted) {
        (crate::film::at_strength(&f.look, strength), "film simulation")
    } else {
        return Err(format!("No style named {name}. Use list_styles to see the names."));
    };
    with_doc(ws, args, &mut |_, e| {
        e.look = look;
        Ok(json!({"applied": name, "kind": kind}))
    })
}

fn set_adjustments(ws: &mut dyn Workspace, args: &Value) -> Result<Value, String> {
    with_doc(ws, args, &mut |_, e| {
        let l = &mut e.look;
        let mut changed = Vec::new();
        // (name, field, scale from the number the app shows, lowest, highest)
        let fields: [(&str, &mut f32, f32, f32, f32); 18] = [
            ("exposure", &mut l.exposure, 1.0, -2.0, 2.0),
            ("contrast", &mut l.contrast, 200.0, -100.0, 100.0),
            ("highlights", &mut l.highlights, 100.0, -100.0, 100.0),
            ("shadows", &mut l.shadows, 100.0, -100.0, 100.0),
            ("whites", &mut l.whites, 100.0, -100.0, 100.0),
            ("blacks", &mut l.blacks, 100.0, -100.0, 100.0),
            ("temperature", &mut l.warmth, 100.0, -100.0, 100.0),
            ("tint", &mut l.tint, 100.0, -100.0, 100.0),
            ("vibrance", &mut l.vibrance, 100.0, -100.0, 100.0),
            ("saturation", &mut l.saturation, 100.0, -100.0, 100.0),
            ("texture", &mut l.texture, 100.0, -100.0, 100.0),
            ("clarity", &mut l.clarity, 100.0, -100.0, 100.0),
            ("dehaze", &mut l.dehaze, 100.0, -100.0, 100.0),
            ("sharpening", &mut l.sharpen, 100.0, 0.0, 100.0),
            ("fade", &mut l.fade, 100.0, 0.0, 100.0),
            ("vignette", &mut l.vignette, 100.0, -100.0, 100.0),
            ("grain", &mut l.grain, 100.0, 0.0, 100.0),
            ("s_curve", &mut l.s_curve, 100.0, -100.0, 100.0),
        ];
        for (name, field, scale, lo, hi) in fields {
            if let Some(v) = args.get(name).and_then(Value::as_f64) {
                *field = (v as f32).clamp(lo, hi) / scale;
                changed.push(name);
            }
        }
        changed.extend(set_color_details(l, args)?);
        if changed.is_empty() {
            return Err("Give at least one adjustment, for example {\"exposure\": 0.3}.".into());
        }
        Ok(json!({"changed": changed}))
    })
}

fn set_framing(ws: &mut dyn Workspace, args: &Value) -> Result<Value, String> {
    with_doc(ws, args, &mut |scene, edits| {
        let mut done = Vec::new();
        if let Some(n) = args.get("rotate_quarter_turns").and_then(Value::as_i64) {
            for _ in 0..n.rem_euclid(4) {
                scene.rotate(true);
            }
            done.push("rotate_quarter_turns");
        }
        if let Some(deg) = args.get("straighten_degrees").and_then(Value::as_f64) {
            let from = scene.angle;
            scene.set_angle((deg as f32).clamp(-45.0, 45.0), false);
            scene.push_angle_step(from);
            done.push("straighten_degrees");
        }
        if let Some(r) = args.get("crop_ratio").and_then(Value::as_str) {
            let aspect = parse_aspect(r)?;
            let (w, h) = (scene.full.width(), scene.full.height());
            scene.checkpoint();
            scene.aspect = aspect;
            scene.crop = match aspect.ratio(w, h) {
                None => CropRect::FULL,
                Some(ratio) => CropRect::largest(ratio, [0.5, 0.5], w, h),
            };
            scene.crop_applied = !scene.crop.is_full();
            done.push("crop_ratio");
        }
        if let Some(b) = args.get("border") {
            set_border(&mut edits.border, b)?;
            done.push("border");
        }
        if let Some(c) = args.get("caption") {
            set_caption(&mut edits.caption, c)?;
            done.push("caption");
        }
        if done.is_empty() {
            return Err("Give at least one framing setting.".into());
        }
        Ok(json!({"changed": done}))
    })
}

fn preview(ws: &mut dyn Workspace, args: &Value) -> Reply {
    let long = args.get("long_side").and_then(Value::as_u64).unwrap_or(768).clamp(128, 2048) as u32;
    let mut png = Vec::new();
    let mut size = (0, 0);
    let r = with_doc(ws, args, &mut |scene, edits| {
        let img = crate::app::framed_still(&scene.full, scene.crop, edits, long);
        size = img.dimensions();
        png.clear();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| format!("Could not make the preview: {e}"))?;
        Ok(Value::Null)
    });
    match r {
        Ok(_) => Reply {
            text: format!("Preview: {} × {} px, with its style and framing.", size.0, size.1),
            png: Some(png),
            is_error: false,
        },
        Err(e) => Reply::error(e),
    }
}

fn save_image(ws: &mut dyn Workspace, args: &Value) -> Result<Value, String> {
    let path = PathBuf::from(str_arg(args, "path")?);
    let target = crate::mcp_access::check_write(&path, &ws.folders())?;
    let format = format_for(&target)?;
    let long = args.get("long_side").and_then(Value::as_u64).filter(|&v| v > 0).map_or(u32::MAX, |v| v as u32);
    with_doc(ws, args, &mut |scene, edits| {
        let img = crate::app::framed_still(&scene.full, scene.crop, edits, long);
        let saved = crate::export::save_still(format, &img, &target)?;
        Ok(json!({"saved": saved.display().to_string(), "width": img.width(), "height": img.height()}))
    })
}

fn str_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key).and_then(Value::as_str).map(String::from).ok_or_else(|| format!("Missing \"{key}\"."))
}

fn list_styles() -> Value {
    let presets: Vec<String> = crate::presets::list().into_iter().map(|p| p.name).collect();
    let looks: Vec<&str> = crate::finish::Look::presets().iter().map(|(n, _)| *n).collect();
    let film: Vec<Value> =
        crate::film::all().into_iter().map(|f| json!({"name": f.name, "group": f.group, "note": f.note})).collect();
    json!({"lightroom_presets": presets, "looks": looks, "film_simulations": film})
}

fn parse_aspect(s: &str) -> Result<Aspect, String> {
    let s = s.trim().to_lowercase();
    if s == "none" || s == "free" || s == "full" {
        return Ok(Aspect::Free);
    }
    Aspect::ALL
        .into_iter()
        .find(|a| a.label().to_lowercase() == s)
        .ok_or_else(|| format!("Unknown ratio {s}. Use one of: none, original, 1:1, 4:5, 3:4, 2:3, 9:16, 4:3, 3:2, 16:9."))
}

fn parse_color(v: &Value) -> Result<[u8; 3], String> {
    let s = v.as_str().ok_or("Colours are written like \"#f4f0e8\".")?;
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return Err(format!("Colour {s} should look like \"#f4f0e8\"."));
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| format!("Colour {s} is not valid."));
    Ok([c(0)?, c(2)?, c(4)?])
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn pct(v: &Value, key: &str, lo: f32, hi: f32) -> Option<f32> {
    v.get(key).and_then(Value::as_f64).map(|x| (x as f32).clamp(lo, hi) / 100.0)
}

fn set_border(b: &mut Border, v: &Value) -> Result<(), String> {
    if let Some(style) = v.get("style").and_then(Value::as_str) {
        let style = style.trim().to_lowercase();
        let fit = match style.as_str() {
            "even" => BorderFit::Even,
            "keep_shape" | "keep shape" => BorderFit::KeepShape,
            "polaroid" => BorderFit::Polaroid,
            other => BorderFit::Ratio(parse_aspect(other).ok().filter(|a| crate::border::RATIO_CHOICES.contains(a)).ok_or_else(
                || format!("Unknown border style {other}. Use even, keep_shape, polaroid or a ratio such as 4:5."),
            )?),
        };
        if fit == BorderFit::Polaroid && b.fit != BorderFit::Polaroid {
            let keep = *b;
            *b = Border::polaroid();
            b.radius = keep.radius;
            b.mounts = keep.mounts;
        }
        b.fit = fit;
        b.enabled = true;
    }
    if let Some(on) = v.get("enabled").and_then(Value::as_bool) {
        b.enabled = on;
    }
    if let Some(x) = pct(v, "thickness_percent", 0.0, 25.0) {
        b.size = x;
        b.enabled = true;
    }
    if let Some(c) = v.get("color") {
        b.color = parse_color(c)?;
    }
    if let Some(x) = pct(v, "rounded_corners_percent", 0.0, 25.0) {
        b.radius = x;
    }
    if let Some(on) = v.get("corner_mounts").and_then(Value::as_bool) {
        b.mounts = on;
    }
    if let Some(x) = pct(v, "mount_size_percent", 3.0, 25.0) {
        b.mount_size = x;
    }
    if let Some(c) = v.get("mount_color") {
        b.mount_color = parse_color(c)?;
    }
    if let Some(x) = v.get("polaroid_bottom").and_then(Value::as_f64) {
        b.bottom = (x as f32).clamp(0.5, 8.0);
        b.target = None;
    }
    Ok(())
}

fn set_caption(c: &mut crate::caption::Caption, v: &Value) -> Result<(), String> {
    if let Some(t) = v.get("text").and_then(Value::as_str) {
        c.text = t.to_string();
        c.enabled = !t.trim().is_empty();
    }
    if let Some(on) = v.get("enabled").and_then(Value::as_bool) {
        c.enabled = on;
    }
    if let Some(x) = pct(v, "size_percent", 2.0, 25.0) {
        c.size = x;
    }
    if let Some(x) = pct(v, "margin_percent", 0.0, 40.0) {
        c.margin = x;
    }
    if let Some(x) = v.get("opacity_percent").and_then(Value::as_f64) {
        c.opacity = (x as f32 / 100.0).clamp(0.1, 1.0);
    }
    if let Some(col) = v.get("color") {
        c.color = parse_color(col)?;
    }
    if let Some(s) = v.get("shadow").and_then(Value::as_bool) {
        c.shadow = s;
    }
    if let Some(p) = v.get("place").and_then(Value::as_str) {
        c.place = match p.to_lowercase().as_str() {
            "top" => Place::Top,
            "middle" => Place::Middle,
            "bottom" => Place::Bottom,
            other => return Err(format!("Unknown place {other}. Use top, middle or bottom.")),
        };
    }
    if let Some(a) = v.get("align").and_then(Value::as_str) {
        c.align = match a.to_lowercase().as_str() {
            "left" => Align::Left,
            "center" | "centre" => Align::Center,
            "right" => Align::Right,
            other => return Err(format!("Unknown align {other}. Use left, center or right.")),
        };
    }
    if let Some(w) = v.get("where").and_then(Value::as_str) {
        c.area = match w.to_lowercase().as_str() {
            "picture" | "on_picture" => Area::Picture,
            "border" | "in_border" => Area::Border,
            other => return Err(format!("Unknown where {other}. Use picture or border.")),
        };
    }
    if let Some(f) = v.get("font").and_then(Value::as_str) {
        let wanted = f.to_lowercase();
        let found = crate::fonts::added_fonts()
            .into_iter()
            .chain(crate::fonts::installed_fonts())
            .find(|e| e.name.to_lowercase() == wanted)
            .ok_or_else(|| format!("No font named {f} on this computer."))?;
        c.font = found;
    }
    Ok(())
}

fn settings_json(scene: &Scene, edits: &Edits) -> Value {
    let l = &edits.look;
    let b = &edits.border;
    let c = &edits.caption;
    // One decimal, as f64 so the numbers print cleanly.
    let r = |v: f32, s: f32| ((v * s) as f64 * 10.0).round() / 10.0;
    json!({
        "name": scene.name,
        "size": [scene.full.width(), scene.full.height()],
        "adjustments": {
            "exposure": r(l.exposure, 1.0), "contrast": r(l.contrast, 200.0), "highlights": r(l.highlights, 100.0),
            "shadows": r(l.shadows, 100.0), "whites": r(l.whites, 100.0), "blacks": r(l.blacks, 100.0),
            "temperature": r(l.warmth, 100.0), "tint": r(l.tint, 100.0), "vibrance": r(l.vibrance, 100.0),
            "saturation": r(l.saturation, 100.0), "texture": r(l.texture, 100.0), "clarity": r(l.clarity, 100.0),
            "dehaze": r(l.dehaze, 100.0), "sharpening": r(l.sharpen, 100.0), "fade": r(l.fade, 100.0),
            "vignette": r(l.vignette, 100.0), "grain": r(l.grain, 100.0), "s_curve": r(l.s_curve, 100.0),
        },
        "tone_curve": curves_json(l),
        "hsl": hsl_json(l),
        "color_grading": grading_json(l),
        "details": details_json(l),
        "framing": {
            "straighten_degrees": scene.angle,
            "crop_ratio": scene.aspect.label(),
            "border": {
                "enabled": b.enabled, "style": match b.fit { BorderFit::Even => "even".to_string(), BorderFit::KeepShape => "keep_shape".into(), BorderFit::Polaroid => "polaroid".into(), BorderFit::Ratio(a) => a.label().into() },
                "thickness_percent": r(b.size, 100.0), "color": hex(b.color),
                "rounded_corners_percent": r(b.radius, 100.0), "corner_mounts": b.mounts,
                "mount_size_percent": r(b.mount_size, 100.0), "mount_color": hex(b.mount_color),
            },
            "caption": {
                "enabled": c.enabled, "text": c.text, "font": c.font.name, "size_percent": r(c.size, 100.0),
                "color": hex(c.color), "place": format!("{:?}", c.place).to_lowercase(),
                "align": format!("{:?}", c.align).to_lowercase(),
                "where": match c.area { Area::Picture => "picture", Area::Border => "border" },
            },
        },
    })
}

// ----- Tone curve, HSL, colour grading and details --------------------------
// The same layout is reported by get_settings and accepted by set_adjustments,
// so an assistant can read a look and write it back.

const CURVE_NAMES: [&str; 4] = ["all", "red", "green", "blue"];
const GRADE_NAMES: [&str; 4] = ["shadows", "midtones", "highlights", "global"];

fn one(v: f32) -> f64 {
    (v as f64 * 10.0).round() / 10.0
}

/// Curves as points (input, output) from 0 to 255, like Lightroom.
fn curves_json(l: &crate::finish::Look) -> Value {
    let mut m = serde_json::Map::new();
    for (name, c) in CURVE_NAMES.iter().zip(&l.curves) {
        let pts: Vec<Value> = c.points().iter().map(|p| json!([(p[0] * 255.0).round(), (p[1] * 255.0).round()])).collect();
        m.insert((*name).into(), Value::Array(pts));
    }
    Value::Object(m)
}

/// Hue, saturation and luminance per colour band, -100 to 100.
fn hsl_json(l: &crate::finish::Look) -> Value {
    let mut m = serde_json::Map::new();
    for (name, b) in crate::develop::HSL_NAMES.iter().zip(&l.hsl) {
        m.insert(name.to_lowercase(), json!({"hue": one(b[0] * 100.0), "saturation": one(b[1] * 100.0), "luminance": one(b[2] * 100.0)}));
    }
    Value::Object(m)
}

/// Colour grading wheels: hue 0 to 360, saturation 0 to 100, luminance -100
/// to 100; blending 0 to 100 and balance -100 to 100.
fn grading_json(l: &crate::finish::Look) -> Value {
    let mut m = serde_json::Map::new();
    for (name, g) in GRADE_NAMES.iter().zip(&l.grade) {
        m.insert((*name).into(), json!({"hue": one(g[0]), "saturation": one(g[1] * 100.0), "luminance": one(g[2] * 100.0)}));
    }
    m.insert("blending".into(), json!(one(l.grade_blend * 100.0)));
    m.insert("balance".into(), json!(one(l.grade_balance * 100.0)));
    Value::Object(m)
}

/// Finer settings, mostly set by presets and film looks.
fn details_json(l: &crate::finish::Look) -> Value {
    let rgb = |c: [f32; 3]| json!([one(c[0] * 1000.0), one(c[1] * 1000.0), one(c[2] * 1000.0)]);
    json!({
        "s_curve_pivot": one(l.pivot * 100.0),
        "vignette_midpoint": one(l.vignette_mid * 100.0),
        "vignette_feather": one(l.vignette_feather * 100.0),
        "vignette_roundness": one(l.vignette_round * 100.0),
        "grain_size": one(l.grain_size),
        "grain_roughness": one(l.grain_rough * 100.0),
        "split_tone_shadows": rgb(l.tint_shadows),
        "split_tone_highlights": rgb(l.tint_highlights),
        "green_shift": one(l.green_shift * 100.0),
        "green_curve": one(l.green_curve * 100.0),
    })
}

/// Read tone_curve, hsl, color_grading and details from set_adjustments
/// arguments (any part may be left out). Returns what changed.
fn set_color_details(l: &mut crate::finish::Look, args: &Value) -> Result<Vec<&'static str>, String> {
    let mut changed = Vec::new();
    let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64).map(|x| x as f32);
    if let Some(c) = args.get("tone_curve") {
        for (i, name) in CURVE_NAMES.iter().enumerate() {
            let Some(pts) = c.get(*name) else { continue };
            let pts = pts.as_array().ok_or(format!("tone_curve.{name} should be a list of [input, output] points"))?;
            let mut v = Vec::new();
            for p in pts {
                let xy = p.as_array().filter(|a| a.len() == 2).ok_or(format!("tone_curve.{name}: each point is [input, output]"))?;
                let x = xy[0].as_f64().ok_or("curve points are numbers 0 to 255")? as f32 / 255.0;
                let y = xy[1].as_f64().ok_or("curve points are numbers 0 to 255")? as f32 / 255.0;
                v.push([x, y]);
            }
            if v.len() < 2 || v.len() > crate::develop::MAX_POINTS {
                return Err(format!("tone_curve.{name} needs 2 to {} points", crate::develop::MAX_POINTS));
            }
            l.curves[i] = crate::develop::Curve::from_points(&v);
        }
        changed.push("tone_curve");
    }
    if let Some(h) = args.get("hsl") {
        for (i, name) in crate::develop::HSL_NAMES.iter().enumerate() {
            let Some(b) = h.get(name.to_lowercase()) else { continue };
            for (k, key) in ["hue", "saturation", "luminance"].iter().enumerate() {
                if let Some(x) = num(b, key) {
                    l.hsl[i][k] = x.clamp(-100.0, 100.0) / 100.0;
                }
            }
        }
        changed.push("hsl");
    }
    if let Some(g) = args.get("color_grading") {
        for (i, name) in GRADE_NAMES.iter().enumerate() {
            let Some(w) = g.get(*name) else { continue };
            if let Some(x) = num(w, "hue") {
                l.grade[i][0] = x.rem_euclid(360.0);
            }
            if let Some(x) = num(w, "saturation") {
                l.grade[i][1] = x.clamp(0.0, 100.0) / 100.0;
            }
            if let Some(x) = num(w, "luminance") {
                l.grade[i][2] = x.clamp(-100.0, 100.0) / 100.0;
            }
        }
        if let Some(x) = num(g, "blending") {
            l.grade_blend = x.clamp(0.0, 100.0) / 100.0;
        }
        if let Some(x) = num(g, "balance") {
            l.grade_balance = x.clamp(-100.0, 100.0) / 100.0;
        }
        changed.push("color_grading");
    }
    if let Some(d) = args.get("details") {
        let pct = |k: &str, lo: f32, hi: f32| num(d, k).map(|x| x.clamp(lo, hi) / 100.0);
        if let Some(x) = pct("s_curve_pivot", 10.0, 90.0) { l.pivot = x; }
        if let Some(x) = pct("vignette_midpoint", 0.0, 100.0) { l.vignette_mid = x; }
        if let Some(x) = pct("vignette_feather", 0.0, 100.0) { l.vignette_feather = x; }
        if let Some(x) = pct("vignette_roundness", -100.0, 100.0) { l.vignette_round = x; }
        if let Some(x) = num(d, "grain_size") { l.grain_size = x.clamp(1.0, 4.0); }
        if let Some(x) = pct("grain_roughness", 0.0, 100.0) { l.grain_rough = x; }
        if let Some(x) = pct("green_shift", 0.0, 100.0) { l.green_shift = x; }
        if let Some(x) = pct("green_curve", -100.0, 100.0) { l.green_curve = x; }
        for (key, field) in [("split_tone_shadows", &mut l.tint_shadows), ("split_tone_highlights", &mut l.tint_highlights)] {
            if let Some(a) = d.get(key).and_then(Value::as_array).filter(|a| a.len() == 3) {
                for (k, v) in a.iter().enumerate() {
                    if let Some(x) = v.as_f64() {
                        field[k] = (x as f32).clamp(-100.0, 100.0) / 1000.0;
                    }
                }
            }
        }
        changed.push("details");
    }
    Ok(changed)
}

fn format_for(path: &Path) -> Result<Format, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "heic" | "heif" => Ok(Format::Heif),
        "jpg" | "jpeg" => Ok(Format::Jpeg),
        "png" => Ok(Format::Png),
        _ => Err("Save as .heic, .jpg or .png.".into()),
    }
}

/// Tool descriptions for tools/list.
pub fn tool_list() -> Value {
    let mut all = base_tool_list();
    if let Some(a) = all.as_array_mut() {
        a.extend(crate::mcp_masks::tool_list());
    }
    all
}

fn base_tool_list() -> Value {
    let id = json!({"type": "string", "description": "Id from open_image, such as img1"});
    let num = |d: &str| json!({"type": "number", "description": d});
    let color = |d: &str| json!({"type": "string", "description": format!("{d}, written like #f4f0e8")});
    json!([
        {"name": "allowed_folders", "description": "Folders Animagraph lets the assistant open from and save into. Change them in the app: View > Assistant access (MCP).",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "open_image", "description": "Open a photo (JPEG, PNG, HEIC, RAW...) from an allowed folder. Returns an image_id used by the other tools.",
         "inputSchema": {"type": "object", "properties": {"path": {"type": "string", "description": "Full path of the photo"}}, "required": ["path"]}},
        {"name": "list_open_images", "description": "Photos opened in this session.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "list_styles", "description": "Names of the imported Lightroom presets, built-in looks and film simulations.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "apply_style", "description": "Apply a Lightroom preset, look or film simulation by name (replaces the current adjustments).",
         "inputSchema": {"type": "object", "properties": {"image_id": id, "name": {"type": "string"}, "strength": num("Film simulations only: 0 to 100, default 100")}, "required": ["image_id", "name"]}},
        {"name": "set_adjustments", "description": "Change Edit settings, in the same units as the app: exposure in stops (-2 to 2); sharpening, fade and grain 0 to 100; all others -100 to 100. Only the ones given change.",
         "inputSchema": {"type": "object", "properties": {
            "image_id": id, "exposure": num("stops, -2 to 2"), "contrast": num("-100 to 100"), "highlights": num("-100 to 100"), "shadows": num("-100 to 100"),
            "whites": num("-100 to 100"), "blacks": num("-100 to 100"), "temperature": num("-100 cool to 100 warm"), "tint": num("-100 green to 100 magenta"),
            "vibrance": num("-100 to 100"), "saturation": num("-100 to 100"), "texture": num("-100 to 100"), "clarity": num("-100 to 100"),
            "dehaze": num("-100 to 100"), "sharpening": num("0 to 100"), "fade": num("0 to 100"), "vignette": num("-100 to 100, positive darkens edges"),
            "grain": num("0 to 100"), "s_curve": num("-100 to 100"),
            "tone_curve": {"type": "object", "description": "Point curves for all, red, green, blue: lists of [input, output] from 0 to 255 (2 to 16 points), as get_settings reports them"},
            "hsl": {"type": "object", "description": "Per colour band (red, orange, yellow, green, aqua, blue, purple, magenta): {hue, saturation, luminance}, each -100 to 100"},
            "color_grading": {"type": "object", "description": "shadows, midtones, highlights, global: {hue 0-360, saturation 0-100, luminance -100 to 100}; plus blending 0-100 and balance -100 to 100"},
            "details": {"type": "object", "description": "s_curve_pivot, vignette_midpoint, vignette_feather, vignette_roundness, grain_size (1-4), grain_roughness, green_shift, green_curve, split_tone_shadows and split_tone_highlights ([r, g, b], -100 to 100)"}},
            "required": ["image_id"]}},
        {"name": "reset_adjustments", "description": "Remove all colour adjustments and styles.",
         "inputSchema": {"type": "object", "properties": {"image_id": id}, "required": ["image_id"]}},
        {"name": "set_framing", "description": "Rotate, straighten, crop, and set the border (with rounded corners and corner mounts) and caption. Only the parts given change.",
         "inputSchema": {"type": "object", "properties": {
            "image_id": id,
            "rotate_quarter_turns": {"type": "integer", "description": "Quarter turns to the right (negative to the left)"},
            "straighten_degrees": num("-45 to 45, positive is clockwise"),
            "crop_ratio": {"type": "string", "description": "none, original, 1:1, 4:5, 3:4, 2:3, 9:16, 4:3, 3:2 or 16:9 (largest centred crop)"},
            "border": {"type": "object", "properties": {
                "enabled": {"type": "boolean"}, "style": {"type": "string", "description": "even, keep_shape, polaroid, or a ratio such as 4:5"},
                "thickness_percent": num("0 to 25, of the photo's shorter side"), "color": color("Border colour"),
                "rounded_corners_percent": num("0 to 25"), "corner_mounts": {"type": "boolean"}, "mount_size_percent": num("3 to 25"),
                "mount_color": color("Mount colour"), "polaroid_bottom": num("Polaroid bottom strip, 0.5 to 8 times the sides")}},
            "caption": {"type": "object", "properties": {
                "text": {"type": "string"}, "enabled": {"type": "boolean"}, "font": {"type": "string", "description": "Font name installed on this computer"},
                "size_percent": num("2 to 25"), "margin_percent": num("0 to 40"), "opacity_percent": num("10 to 100"), "color": color("Text colour"),
                "shadow": {"type": "boolean"}, "place": {"type": "string", "description": "top, middle or bottom"},
                "align": {"type": "string", "description": "left, center or right"}, "where": {"type": "string", "description": "picture or border"}}}},
            "required": ["image_id"]}},
        {"name": "get_settings", "description": "Current settings of an open photo: basic adjustments, tone curve, HSL, colour grading, finer details (vignette, grain, split toning) and framing. set_adjustments accepts the same layout.",
         "inputSchema": {"type": "object", "properties": {"image_id": id}, "required": ["image_id"]}},
        {"name": "preview", "description": "A small picture of the photo with its style and framing, to look at the result.",
         "inputSchema": {"type": "object", "properties": {"image_id": id, "long_side": num("Pixels on the longest side, 128 to 2048, default 768")}, "required": ["image_id"]}},
        {"name": "save_image", "description": "Save the finished photo into an allowed folder. The format follows the extension: .heic, .jpg or .png.",
         "inputSchema": {"type": "object", "properties": {"image_id": id, "path": {"type": "string", "description": "Full path to save to"}, "long_side": num("Pixels on the longest side; leave out for full size")}, "required": ["image_id", "path"]}},
        {"name": "import_preset", "description": "Import a Lightroom preset (.xmp file, or a .dng preset from Lightroom mobile) from an allowed folder. It is copied into Animagraph's presets and can then be used with apply_style.",
         "inputSchema": {"type": "object", "properties": {"path": {"type": "string", "description": "Full path of the .xmp or .dng preset"}}, "required": ["path"]}},
        {"name": "close_image", "description": "Close an open photo and forget its settings.",
         "inputSchema": {"type": "object", "properties": {"image_id": id}, "required": ["image_id"]}}
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_adjust_frame_and_save_inside_an_allowed_folder() {
        let dir = std::env::temp_dir().join(format!("animagraph_mcp_tools_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("in.png");
        image::RgbaImage::from_pixel(64, 48, image::Rgba([120, 120, 120, 255])).save(&src).unwrap();
        let mut s = Session::new(vec![dir.clone()]);
        let r = s.call("open_image", &json!({"path": src.display().to_string()}));
        assert!(!r.is_error, "{}", r.text);
        assert!(!s.call("set_adjustments", &json!({"image_id": "img1", "exposure": 0.5, "contrast": 20})).is_error);
        let r = s.call(
            "set_framing",
            &json!({"image_id": "img1", "crop_ratio": "1:1", "border": {"style": "even", "thickness_percent": 10, "color": "#ffffff", "rounded_corners_percent": 5}}),
        );
        assert!(!r.is_error, "{}", r.text);
        assert!(s.call("preview", &json!({"image_id": "img1"})).png.is_some());
        // Curves, HSL and grading: set, then read back the same values.
        let r = s.call(
            "set_adjustments",
            &json!({"image_id": "img1",
                    "tone_curve": {"all": [[0, 10], [128, 140], [255, 250]]},
                    "hsl": {"blue": {"hue": -20, "saturation": 15, "luminance": -10}},
                    "color_grading": {"shadows": {"hue": 210, "saturation": 25, "luminance": -5}, "balance": 20}}),
        );
        assert!(!r.is_error, "{}", r.text);
        let got: Value = serde_json::from_str(&s.call("get_settings", &json!({"image_id": "img1"})).text).unwrap();
        assert_eq!(got["tone_curve"]["all"], json!([[0.0, 10.0], [128.0, 140.0], [255.0, 250.0]]));
        assert_eq!(got["hsl"]["blue"]["hue"], json!(-20.0));
        assert_eq!(got["color_grading"]["shadows"]["hue"], json!(210.0));
        assert_eq!(got["color_grading"]["balance"], json!(20.0));
        let out = dir.join("out.png");
        let r = s.call("save_image", &json!({"image_id": "img1", "path": out.display().to_string()}));
        assert!(!r.is_error, "{}", r.text);
        let saved = image::open(&out).unwrap();
        // 48 x 48 crop plus a 10% border on each side.
        assert_eq!((saved.width(), saved.height()), (58, 58));
        // Outside the allowed folder: refused.
        let r = s.call("open_image", &json!({"path": "/etc/hosts"}));
        assert!(r.is_error);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
