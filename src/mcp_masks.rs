//! Mask tools for assistants (MCP): list a photo's masks, add a linear,
//! radial or sky mask, change one, and remove one. Painting a brush mask by
//! hand stays in the window. Positions are fractions (0..1) of the whole
//! picture, x to the right and y down; adjustments use the same numbers as
//! set_adjustments.

use serde_json::{Value, json};

use crate::mask::{Mask, RangeKind, Shape};
use crate::mcp_tools::Workspace;

pub fn call(ws: &mut dyn Workspace, name: &str, args: &Value) -> Option<Result<Value, String>> {
    Some(match name {
        "list_masks" => with_masks(ws, args, &mut |masks, _| Ok(json!({"masks": masks.iter().enumerate().map(|(i, m)| mask_json(i, m)).collect::<Vec<_>>()}))),
        "add_mask" => add_mask(ws, args),
        "set_mask" => set_mask(ws, args),
        "remove_mask" => with_masks(ws, args, &mut |masks, _| {
            let i = index(args, masks.len())?;
            let m = masks.remove(i);
            Ok(json!({"removed": m.name}))
        }),
        _ => return None,
    })
}

fn with_masks(
    ws: &mut dyn Workspace,
    args: &Value,
    f: &mut dyn FnMut(&mut Vec<Mask>, &image::RgbaImage) -> Result<Value, String>,
) -> Result<Value, String> {
    let id = args.get("image_id").and_then(Value::as_str).ok_or("Missing \"image_id\".")?.to_string();
    ws.edit(&id, &mut |scene, edits| f(&mut edits.masks, &scene.full))
}

fn index(args: &Value, len: usize) -> Result<usize, String> {
    let i = args.get("index").and_then(Value::as_u64).ok_or("Missing \"index\" (from list_masks).")? as usize;
    if i < len { Ok(i) } else { Err(format!("No mask {i}; this photo has {len}.")) }
}

fn add_mask(ws: &mut dyn Workspace, args: &Value) -> Result<Value, String> {
    let kind = args.get("kind").and_then(Value::as_str).unwrap_or("").to_lowercase();
    with_masks(ws, args, &mut |masks, work| {
        let n = masks.len() + 1;
        let mut m = match kind.as_str() {
            "linear" => Mask::linear(format!("Linear {n}")),
            "radial" => Mask::radial(format!("Radial {n}")),
            "sky" => Mask::sky(format!("Sky {n}"), work),
            _ => return Err("kind must be linear, radial or sky (brush masks are painted in the window).".into()),
        };
        apply(&mut m, args)?;
        masks.push(m);
        let i = masks.len() - 1;
        Ok(json!({"added": mask_json(i, &masks[i])}))
    })
}

fn set_mask(ws: &mut dyn Workspace, args: &Value) -> Result<Value, String> {
    with_masks(ws, args, &mut |masks, _| {
        let i = index(args, masks.len())?;
        apply(&mut masks[i], args)?;
        Ok(json!({"mask": mask_json(i, &masks[i])}))
    })
}

fn point(v: &Value) -> Option<[f32; 2]> {
    let a = v.as_array().filter(|a| a.len() == 2)?;
    Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32])
}

/// Change a mask from tool arguments (any part may be left out).
fn apply(m: &mut Mask, args: &Value) -> Result<(), String> {
    if let Some(s) = args.get("name").and_then(Value::as_str) {
        m.name = s.to_string();
    }
    if let Some(b) = args.get("visible").and_then(Value::as_bool) {
        m.visible = b;
    }
    if let Some(b) = args.get("invert").and_then(Value::as_bool) {
        m.invert = b;
    }
    if let Some(l) = args.get("linear") {
        let Shape::Linear { start, end } = &mut m.shape else { return Err("This mask is not linear.".into()) };
        if let Some(p) = l.get("start").and_then(point) {
            *start = p;
        }
        if let Some(p) = l.get("end").and_then(point) {
            *end = p;
        }
    }
    if let Some(r) = args.get("radial") {
        let Shape::Radial { center, radius, feather } = &mut m.shape else { return Err("This mask is not radial.".into()) };
        if let Some(p) = r.get("center").and_then(point) {
            *center = p;
        }
        if let Some(p) = r.get("radius").and_then(point) {
            *radius = [p[0].max(0.005), p[1].max(0.005)];
        }
        if let Some(f) = r.get("feather_percent").and_then(Value::as_f64) {
            *feather = (f as f32 / 100.0).clamp(0.01, 1.0);
        }
    }
    if let Some(r) = args.get("range") {
        let num = |k: &str| r.get(k).and_then(Value::as_f64).map(|x| x as f32);
        if let Some(k) = r.get("kind").and_then(Value::as_str) {
            m.range.kind = match k.to_lowercase().as_str() {
                "all" | "off" | "none" => RangeKind::Off,
                "brightness" | "luminance" => RangeKind::Luminance,
                "colour" | "color" => RangeKind::Color,
                other => return Err(format!("Unknown range kind {other}. Use all, brightness or colour.")),
            };
        }
        if let Some(x) = num("from") {
            m.range.lum[0] = (x / 100.0).clamp(0.0, 1.0);
        }
        if let Some(x) = num("to") {
            m.range.lum[1] = (x / 100.0).clamp(0.0, 1.0);
        }
        if let Some(x) = num("hue") {
            m.range.hue = x.rem_euclid(360.0);
        }
        if let Some(x) = num("width") {
            m.range.hue_width = x.clamp(10.0, 180.0);
        }
        if let Some(x) = num("smoothness") {
            m.range.smooth = (x / 100.0).clamp(0.0, 1.0);
        }
    }
    if let Some(a) = args.get("adjustments") {
        let l = &mut m.local;
        let fields: [(&str, &mut f32, f32, f32, f32); 12] = [
            ("exposure", &mut l.exposure, 1.0, -2.0, 2.0),
            ("contrast", &mut l.contrast, 200.0, -100.0, 100.0),
            ("highlights", &mut l.highlights, 100.0, -100.0, 100.0),
            ("shadows", &mut l.shadows, 100.0, -100.0, 100.0),
            ("whites", &mut l.whites, 100.0, -100.0, 100.0),
            ("blacks", &mut l.blacks, 100.0, -100.0, 100.0),
            ("temperature", &mut l.temperature, 100.0, -100.0, 100.0),
            ("tint", &mut l.tint, 100.0, -100.0, 100.0),
            ("saturation", &mut l.saturation, 100.0, -100.0, 100.0),
            ("clarity", &mut l.clarity, 100.0, -100.0, 100.0),
            ("texture", &mut l.texture, 100.0, -100.0, 100.0),
            ("dehaze", &mut l.dehaze, 100.0, -100.0, 100.0),
        ];
        for (name, field, scale, lo, hi) in fields {
            if let Some(v) = a.get(name).and_then(Value::as_f64) {
                *field = (v as f32).clamp(lo, hi) / scale;
            }
        }
    }
    Ok(())
}

fn mask_json(i: usize, m: &Mask) -> Value {
    let one = |v: f32| (v as f64 * 1000.0).round() / 1000.0;
    let pct = |v: f32, s: f32| ((v * s) as f64 * 10.0).round() / 10.0;
    let shape = match &m.shape {
        Shape::Linear { start, end } => json!({"linear": {"start": [one(start[0]), one(start[1])], "end": [one(end[0]), one(end[1])]}}),
        Shape::Radial { center, radius, feather } => json!({"radial": {
            "center": [one(center[0]), one(center[1])], "radius": [one(radius[0]), one(radius[1])], "feather_percent": pct(*feather, 100.0)}}),
        Shape::Brush { map, .. } => {
            let covered = map.iter().filter(|v| **v > 0.5).count() as f32 / map.len().max(1) as f32;
            json!({"brush": {"covered_percent": pct(covered, 100.0)}})
        }
    };
    let r = &m.range;
    let l = &m.local;
    json!({
        "index": i, "name": m.name, "kind": m.kind_name().to_lowercase(), "visible": m.visible, "invert": m.invert,
        "shape": shape,
        "range": {
            "kind": match r.kind { RangeKind::Off => "all", RangeKind::Luminance => "brightness", RangeKind::Color => "colour" },
            "from": pct(r.lum[0], 100.0), "to": pct(r.lum[1], 100.0), "hue": pct(r.hue, 1.0), "width": pct(r.hue_width, 1.0),
            "smoothness": pct(r.smooth, 100.0),
        },
        "adjustments": {
            "exposure": pct(l.exposure, 1.0), "contrast": pct(l.contrast, 200.0), "highlights": pct(l.highlights, 100.0),
            "shadows": pct(l.shadows, 100.0), "whites": pct(l.whites, 100.0), "blacks": pct(l.blacks, 100.0),
            "temperature": pct(l.temperature, 100.0), "tint": pct(l.tint, 100.0), "saturation": pct(l.saturation, 100.0),
            "clarity": pct(l.clarity, 100.0), "texture": pct(l.texture, 100.0), "dehaze": pct(l.dehaze, 100.0),
        },
    })
}

/// Tool descriptions, added to tools/list.
pub fn tool_list() -> Vec<Value> {
    let id = json!({"type": "string", "description": "Id from open_image or list_open_images"});
    let idx = json!({"type": "integer", "description": "Mask number from list_masks (0 is the first)"});
    let pt = |d: &str| json!({"type": "array", "items": {"type": "number"}, "description": d});
    let common = json!({
        "name": {"type": "string"},
        "visible": {"type": "boolean"},
        "invert": {"type": "boolean", "description": "Change everything outside the mask instead"},
        "linear": {"type": "object", "description": "Linear masks: full effect at start, none at end",
            "properties": {"start": pt("[x, y], 0 to 1 of the picture"), "end": pt("[x, y], 0 to 1")}},
        "radial": {"type": "object", "description": "Radial masks: an oval, full effect inside",
            "properties": {"center": pt("[x, y], 0 to 1"), "radius": pt("[half-width, half-height] as fractions of the picture's width and height"),
                           "feather_percent": {"type": "number", "description": "1 to 100, how much of the oval fades out"}}},
        "range": {"type": "object", "description": "Narrow the mask to some tones or colours",
            "properties": {"kind": {"type": "string", "description": "all, brightness or colour"},
                           "from": {"type": "number", "description": "brightness 0-100"}, "to": {"type": "number", "description": "brightness 0-100"},
                           "hue": {"type": "number", "description": "colour hue 0-360"}, "width": {"type": "number", "description": "hue band width 10-180"},
                           "smoothness": {"type": "number", "description": "0-100"}}},
        "adjustments": {"type": "object", "description": "exposure (stops, -2 to 2); contrast, highlights, shadows, whites, blacks, temperature, tint, saturation, clarity, texture, dehaze (-100 to 100)"},
    });
    let with = |extra: Value, required: Vec<&str>| {
        let mut props = common.as_object().cloned().unwrap_or_default();
        for (k, v) in extra.as_object().cloned().unwrap_or_default() {
            props.insert(k, v);
        }
        json!({"type": "object", "properties": props, "required": required})
    };
    vec![
        json!({"name": "list_masks", "description": "The masks of an open photo: kind, position, range and adjustments.",
               "inputSchema": {"type": "object", "properties": {"image_id": id}, "required": ["image_id"]}}),
        json!({"name": "add_mask", "description": "Add a mask for local edits: linear (a gradient), radial (an oval) or sky (estimated from the picture). Give its position, range and adjustments in the same call, or later with set_mask.",
               "inputSchema": with(json!({"image_id": id, "kind": {"type": "string", "description": "linear, radial or sky"}}), vec!["image_id", "kind"])}),
        json!({"name": "set_mask", "description": "Change a mask: name, visibility, invert, position, range or adjustments. Only the parts given change.",
               "inputSchema": with(json!({"image_id": id, "index": idx}), vec!["image_id", "index"])}),
        json!({"name": "remove_mask", "description": "Remove a mask (undo in the window brings it back).",
               "inputSchema": {"type": "object", "properties": {"image_id": id, "index": idx}, "required": ["image_id", "index"]}}),
    ]
}

#[cfg(test)]
mod tests {
    use crate::mcp_tools::Session;
    use serde_json::{Value, json};

    #[test]
    fn add_change_list_and_remove_masks() {
        let dir = std::env::temp_dir().join(format!("animagraph_mcp_masks_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("in.png");
        image::RgbaImage::from_pixel(64, 48, image::Rgba([120, 120, 120, 255])).save(&src).unwrap();
        let mut s = Session::new(vec![dir.clone()]);
        assert!(!s.call("open_image", &json!({"path": src.display().to_string()})).is_error);
        let r = s.call("add_mask", &json!({"image_id": "img1", "kind": "radial",
            "radial": {"center": [0.25, 0.5], "radius": [0.2, 0.3]}, "adjustments": {"exposure": 1.0}}));
        assert!(!r.is_error, "{}", r.text);
        let r = s.call("set_mask", &json!({"image_id": "img1", "index": 0, "range": {"kind": "brightness", "from": 20, "to": 80}}));
        assert!(!r.is_error, "{}", r.text);
        let list: Value = serde_json::from_str(&s.call("list_masks", &json!({"image_id": "img1"})).text).unwrap();
        assert_eq!(list["masks"][0]["adjustments"]["exposure"], json!(1.0));
        assert_eq!(list["masks"][0]["range"]["kind"], "brightness");
        // The mask changes the saved picture on its side only.
        let out = dir.join("out.png");
        assert!(!s.call("save_image", &json!({"image_id": "img1", "path": out.display().to_string()})).is_error);
        let img = image::open(&out).unwrap().to_rgba8();
        assert!(img.get_pixel(16, 24)[0] > 130 && img.get_pixel(60, 24)[0] < 125);
        assert!(s.call("add_mask", &json!({"image_id": "img1", "kind": "brush"})).is_error);
        assert!(!s.call("remove_mask", &json!({"image_id": "img1", "index": 0})).is_error);
        assert!(s.call("remove_mask", &json!({"image_id": "img1", "index": 0})).is_error);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
