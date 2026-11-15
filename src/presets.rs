//! Lightroom presets: `.xmp` files, or `.dng` files with the settings
//! embedded (the usual way mobile presets are shared). Imported presets are
//! copied into the app's data folder (`presets/`) and kept between sessions.
//!
//! Lightroom has many more controls than this app, so each setting is
//! translated to the nearest Look setting. Results are close, not identical.

use std::path::{Path, PathBuf};

use crate::finish::Look;

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub path: PathBuf,
    pub look: Look,
}

pub fn presets_dir() -> PathBuf {
    crate::fonts::data_dir().join("presets")
}

fn is_preset_file(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("xmp" | "dng")
    )
}

/// The XMP text inside an `.xmp` file or embedded in a `.dng` file.
fn xmp_text(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let start = text.find("<x:xmpmeta")?;
    let end = text[start..].find("</x:xmpmeta>")? + start + "</x:xmpmeta>".len();
    Some(text[start..end].to_string())
}

/// A `crs:` setting, written either as an attribute (`crs:Exposure2012="+0.5"`)
/// or as an element (`<crs:Exposure2012>+0.5</crs:Exposure2012>`).
fn crs(xmp: &str, key: &str) -> Option<String> {
    let attr = format!("crs:{key}=\"");
    if let Some(i) = xmp.find(&attr) {
        let rest = &xmp[i + attr.len()..];
        return rest.find('"').map(|j| rest[..j].trim().to_string());
    }
    let open = format!("<crs:{key}>");
    let close = format!("</crs:{key}>");
    let i = xmp.find(&open)? + open.len();
    let j = xmp[i..].find(&close)? + i;
    Some(xmp[i..j].trim().to_string())
}

fn num(xmp: &str, key: &str) -> Option<f32> {
    crs(xmp, key).and_then(|v| v.trim_start_matches('+').parse::<f32>().ok())
}

/// The settings without an embedded profile (`<crs:Look>…</crs:Look>`).
/// Profiles are not used, and the name and settings inside one belong to
/// the profile, not to the preset.
fn without_profile(xmp: &str) -> String {
    let mut out = String::with_capacity(xmp.len());
    let mut rest = xmp;
    loop {
        let found = rest.match_indices("<crs:Look").find(|(i, _)| {
            matches!(rest.as_bytes().get(i + "<crs:Look".len()), Some(b'>' | b' ' | b'\n' | b'\r' | b'\t' | b'/'))
        });
        let Some((i, _)) = found else { break };
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        // A self-closing element, or one with a closing tag.
        let end = match (after.find("/>"), after.find('>'), after.find("</crs:Look>")) {
            (Some(s), Some(g), _) if s + 1 == g => s + 2,
            (_, _, Some(c)) => c + "</crs:Look>".len(),
            _ => after.len(),
        };
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// The preset's own name, if it has one.
fn preset_name(xmp: &str) -> Option<String> {
    let block = crs(xmp, "Name")?;
    // <rdf:Alt><rdf:li xml:lang="x-default">Name</rdf:li></rdf:Alt>
    let li = block.find("<rdf:li").and_then(|i| block[i..].find('>').map(|j| i + j + 1));
    let name = match li {
        Some(i) => block[i..].split("</rdf:li>").next().unwrap_or("").to_string(),
        None => block,
    };
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// Points (0..255) of a point tone curve, e.g. `ToneCurvePV2012` or
/// `ToneCurvePV2012Red`.
fn curve_points(xmp: &str, key: &str) -> Vec<[f32; 2]> {
    // The element must match exactly (ToneCurvePV2012 vs ToneCurvePV2012Red).
    let open = format!("<crs:{key}>");
    let close = format!("</crs:{key}>");
    let Some(i) = xmp.find(&open).map(|i| i + open.len()) else { return Vec::new() };
    let Some(j) = xmp[i..].find(&close).map(|j| j + i) else { return Vec::new() };
    xmp[i..j]
        .split("<rdf:li>")
        .skip(1)
        .filter_map(|li| {
            let v = li.split("</rdf:li>").next()?;
            let mut it = v.split(',').map(|n| n.trim().parse::<f32>());
            Some([it.next()?.ok()? / 255.0, it.next()?.ok()? / 255.0])
        })
        .collect()
}

/// The parametric tone curve (Shadows, Darks, Lights, Highlights regions)
/// as an offset at tone `x`.
fn parametric_offset(xmp: &str, x: f32) -> f32 {
    let g = |k: &str| num(xmp, k).unwrap_or(0.0) / 100.0;
    let s1 = num(xmp, "ParametricShadowSplit").unwrap_or(25.0) / 100.0;
    let s2 = num(xmp, "ParametricMidtoneSplit").unwrap_or(50.0) / 100.0;
    let s3 = num(xmp, "ParametricHighlightSplit").unwrap_or(75.0) / 100.0;
    // A smooth bump over each region, wider than the region so they blend.
    let bump = |a: f32, b: f32| {
        let (c, half) = ((a + b) / 2.0, (b - a).max(0.05));
        let t = ((x - c) / half).abs();
        if t >= 1.0 { 0.0 } else { 0.5 * (1.0 + (std::f32::consts::PI * t).cos()) }
    };
    0.2 * (g("ParametricShadows") * bump(0.0, s1)
        + g("ParametricDarks") * bump(s1, s2)
        + g("ParametricLights") * bump(s2, s3)
        + g("ParametricHighlights") * bump(s3, 1.0))
}

/// The all-channel curve: the parametric curve followed by the point curve.
fn master_curve(xmp: &str) -> crate::develop::Curve {
    use crate::develop::Curve;
    let point = Curve::from_points(&curve_points(xmp, "ToneCurvePV2012"));
    let has_parametric = ["ParametricShadows", "ParametricDarks", "ParametricLights", "ParametricHighlights"]
        .iter()
        .any(|k| num(xmp, k).unwrap_or(0.0) != 0.0);
    if !has_parametric {
        return point;
    }
    let samples: Vec<[f32; 2]> = (0..16)
        .map(|i| {
            let x = i as f32 / 15.0;
            let y = (x + parametric_offset(xmp, x)).clamp(0.0, 1.0);
            [x, point.eval(y)]
        })
        .collect();
    Curve::from_points(&samples)
}

/// Translate Lightroom settings to a Look, one control to its equivalent.
pub fn look_from_xmp(xmp: &str) -> Look {
    use crate::develop::Curve;
    let xmp = &without_profile(xmp);
    let mut l = Look::default();
    let get = |k: &str| num(xmp, k).unwrap_or(0.0);

    // Light
    l.exposure = get("Exposure2012").clamp(-2.0, 2.0);
    l.contrast = (get("Contrast2012") / 100.0 * 0.5).clamp(-0.5, 0.5);
    l.highlights = (get("Highlights2012") / 100.0).clamp(-1.0, 1.0);
    l.shadows = (get("Shadows2012") / 100.0).clamp(-1.0, 1.0);
    l.whites = (get("Whites2012") / 100.0).clamp(-1.0, 1.0);
    l.blacks = (get("Blacks2012") / 100.0).clamp(-1.0, 1.0);

    // Presence. Texture, sharpening and saturation strengths were set by
    // comparing with a photo edited in Lightroom (Texture +25, Sharpening 50,
    // Saturation -15): Lightroom's texture and sharpening are stronger and
    // its saturation slider weaker than the app's controls of the same number.
    l.clarity = (get("Clarity2012") / 100.0).clamp(-1.0, 1.0);
    l.texture = (get("Texture") / 100.0 * 2.0).clamp(-1.0, 1.0);
    l.sharpen = (get("Sharpness") / 100.0).clamp(0.0, 1.0);
    l.dehaze = (get("Dehaze") / 100.0).clamp(-1.0, 1.0);
    l.vibrance = (get("Vibrance") / 100.0).clamp(-1.0, 1.0);
    l.saturation = (get("Saturation") / 100.0 * 0.5).clamp(-1.0, 1.0);

    // White balance: relative values for JPEG presets, absolute for RAW
    // presets. "As Shot" and "Auto" keep the photo's own white balance.
    let wb = crs(xmp, "WhiteBalance").unwrap_or_default().to_ascii_lowercase();
    let keep_wb = wb == "as shot" || wb == "auto";
    if let Some(t) = num(xmp, "IncrementalTemperature") {
        l.warmth = (t / 100.0).clamp(-1.0, 1.0);
    } else if let Some(k) = num(xmp, "Temperature").filter(|_| !keep_wb) {
        // Color temperature changes are even steps in mireds (1,000,000 / K),
        // measured from daylight (5500 K).
        let mired = |k: f32| 1.0e6 / k.max(1000.0);
        l.warmth = ((mired(5500.0) - mired(k)) / 130.0).clamp(-1.0, 1.0);
    }
    if let Some(t) = num(xmp, "IncrementalTint") {
        l.tint = (t / 100.0).clamp(-1.0, 1.0);
    } else if let Some(t) = num(xmp, "Tint").filter(|_| !keep_wb) {
        l.tint = (t / 150.0).clamp(-1.0, 1.0);
    }

    // Tone curves
    l.curves = [
        master_curve(xmp),
        Curve::from_points(&curve_points(xmp, "ToneCurvePV2012Red")),
        Curve::from_points(&curve_points(xmp, "ToneCurvePV2012Green")),
        Curve::from_points(&curve_points(xmp, "ToneCurvePV2012Blue")),
    ];

    // HSL per color band
    for (i, name) in crate::develop::HSL_NAMES.iter().enumerate() {
        l.hsl[i] = [
            (get(&format!("HueAdjustment{name}")) / 100.0).clamp(-1.0, 1.0),
            (get(&format!("SaturationAdjustment{name}")) / 100.0).clamp(-1.0, 1.0),
            (get(&format!("LuminanceAdjustment{name}")) / 100.0).clamp(-1.0, 1.0),
        ];
    }

    // Color grading (newer presets) or split toning (older presets).
    let pair = |hue: &str, sat: &str| Some([num(xmp, hue)?, (num(xmp, sat)? / 100.0).clamp(0.0, 1.0)]);
    let shadows = pair("ColorGradeShadowHue", "ColorGradeShadowSat")
        .or_else(|| pair("SplitToningShadowHue", "SplitToningShadowSaturation"));
    let mids = pair("ColorGradeMidtoneHue", "ColorGradeMidtoneSat");
    let highs = pair("ColorGradeHighlightHue", "ColorGradeHighlightSat")
        .or_else(|| pair("SplitToningHighlightHue", "SplitToningHighlightSaturation"));
    let global = pair("ColorGradeGlobalHue", "ColorGradeGlobalSat");
    let lum = |k: &str| (get(k) / 100.0).clamp(-1.0, 1.0);
    let range = |hs: Option<[f32; 2]>, lum: f32| {
        let [h, s] = hs.unwrap_or([0.0; 2]);
        [h, s, lum]
    };
    l.grade = [
        range(shadows, lum("ColorGradeShadowLum")),
        range(mids, lum("ColorGradeMidtoneLum")),
        range(highs, lum("ColorGradeHighlightLum")),
        range(global, lum("ColorGradeGlobalLum")),
    ];
    l.grade_balance = (get("SplitToningBalance") / 100.0).clamp(-1.0, 1.0);
    l.grade_blend = num(xmp, "ColorGradeBlending")
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
        .unwrap_or(crate::finish::DEFAULT_BLEND);

    // Black and white, grain, vignette
    if crs(xmp, "ConvertToGrayscale").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
        l.saturation = -1.0;
        l.vibrance = 0.0;
        // In black and white the color panel becomes the gray mix: how
        // light each color turns out.
        for (i, name) in crate::develop::HSL_NAMES.iter().enumerate() {
            l.hsl[i] = [0.0, 0.0, (get(&format!("GrayMixer{name}")) / 100.0).clamp(-1.0, 1.0)];
        }
    }
    l.grain = (get("GrainAmount") / 100.0 * 0.8).clamp(0.0, 1.0);
    if let Some(size) = num(xmp, "GrainSize") {
        l.grain_size = (1.0 + size / 50.0).clamp(1.0, 4.0);
    }
    if let Some(rough) = num(xmp, "GrainFrequency") {
        l.grain_rough = (rough / 100.0).clamp(0.0, 1.0);
    }
    // Lightroom's vignette is much stronger and starts nearer the center
    // than the app's slider of the same number. Measured against a photo
    // edited there: -20 with midpoint 45 and feather 60 darkens the corners
    // by about a third, starting about a quarter of the way out.
    let amount = -get("PostCropVignetteAmount") / 100.0;
    l.vignette = if amount > 0.0 {
        ((1.0 - (-2.4 * amount).exp()) / 0.65).min(1.0)
    } else {
        amount.max(-1.0)
    };
    let mid = num(xmp, "PostCropVignetteMidpoint").unwrap_or(50.0) / 100.0;
    l.vignette_mid = ((0.62 * mid - 0.15) / 0.6).clamp(0.0, 1.0);
    if let Some(feather) = num(xmp, "PostCropVignetteFeather") {
        l.vignette_feather = (feather / 100.0 + 0.07).clamp(0.0, 1.0);
    }
    l.vignette_round = (get("PostCropVignetteRoundness") / 100.0).clamp(-1.0, 1.0);
    l
}

/// Read a preset file. `None` when it holds no Lightroom settings.
pub fn read(path: &Path) -> Option<Preset> {
    let bytes = std::fs::read(path).ok()?;
    let xmp = without_profile(&xmp_text(&bytes)?);
    if !xmp.contains("crs:") {
        return None;
    }
    let name = preset_name(&xmp).unwrap_or_else(|| {
        path.file_stem().map(|s| s.to_string_lossy().replace(['_', '-'], " ")).unwrap_or_default()
    });
    Some(Preset { name, path: path.to_path_buf(), look: look_from_xmp(&xmp) })
}

/// Presets already imported, in name order.
pub fn list() -> Vec<Preset> {
    let Ok(entries) = std::fs::read_dir(presets_dir()) else { return Vec::new() };
    let mut out: Vec<Preset> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_preset_file(p))
        .filter_map(|p| read(&p))
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Copy a preset into the app's presets folder.
pub fn import(src: &Path) -> Result<Preset, String> {
    if !is_preset_file(src) {
        return Err("Choose a Lightroom preset: an .xmp file, or a .dng preset file.".into());
    }
    if read(src).is_none() {
        return Err(format!(
            "{} has no Lightroom settings in it.",
            src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
        ));
    }
    let dir = presets_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(src.file_name().ok_or("No file name")?);
    std::fs::copy(src, &dest).map_err(|e| e.to_string())?;
    read(&dest).ok_or_else(|| "Could not read the copied preset.".into())
}

/// Delete an imported preset (only inside the app's presets folder).
pub fn remove(p: &Preset) -> Result<(), String> {
    if !p.path.starts_with(presets_dir()) {
        return Err("Only imported presets can be removed here.".into());
    }
    std::fs::remove_file(&p.path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const XMP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description crs:Exposure2012="+0.50" crs:Contrast2012="+20"
     crs:Highlights2012="-40" crs:Shadows2012="+30" crs:Saturation="-15"
     crs:IncrementalTemperature="+25" crs:GrainAmount="30"
     crs:PostCropVignetteAmount="-20" crs:HueAdjustmentGreen="+60"
     crs:SplitToningShadowHue="200" crs:SplitToningShadowSaturation="30"
     crs:ColorGradeShadowLum="-20" crs:ColorGradeGlobalHue="40"
     crs:ColorGradeGlobalSat="10" crs:ColorGradeBlending="80"
     crs:Texture="+15" crs:Sharpness="60" crs:GrainFrequency="70"
     crs:PostCropVignetteMidpoint="30" crs:PostCropVignetteFeather="80"
     crs:PostCropVignetteRoundness="-40"
     xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/">
   <crs:Name><rdf:Alt><rdf:li xml:lang="x-default">Teal Leaves</rdf:li></rdf:Alt></crs:Name>
   <crs:ToneCurvePV2012><rdf:Seq><rdf:li>0, 25</rdf:li><rdf:li>64, 52</rdf:li><rdf:li>192, 205</rdf:li><rdf:li>255, 255</rdf:li></rdf:Seq></crs:ToneCurvePV2012>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    #[test]
    fn translates_lightroom_settings() {
        let xmp = xmp_text(XMP.as_bytes()).unwrap();
        assert_eq!(preset_name(&xmp).as_deref(), Some("Teal Leaves"));
        let l = look_from_xmp(&xmp);
        assert!((l.exposure - 0.5).abs() < 1e-6);
        assert!((l.contrast - 0.1).abs() < 1e-6);
        assert!((l.highlights + 0.4).abs() < 1e-6);
        assert!((l.shadows - 0.3).abs() < 1e-6);
        assert!((l.warmth - 0.25).abs() < 1e-6);
        assert!(l.saturation < 0.0 && l.grain > 0.0 && l.vignette > 0.0);
        // Green hue +60 goes to the HSL green band.
        assert!((l.hsl[3][0] - 0.6).abs() < 1e-6);
        // The point curve is carried over exactly.
        let c = &l.curves[0];
        assert_eq!(c.n, 4);
        assert!((c.eval(0.0) - 25.0 / 255.0).abs() < 1e-4, "lifted black point");
        assert!((c.eval(64.0 / 255.0) - 52.0 / 255.0).abs() < 1e-4);
        // Split toning becomes shadow color grading.
        assert!((l.grade[0][0] - 200.0).abs() < 1e-6 && (l.grade[0][1] - 0.3).abs() < 1e-6);
        // Luminance, global and blending are read too.
        assert!((l.grade[0][2] + 0.2).abs() < 1e-6);
        assert!((l.grade[3][0] - 40.0).abs() < 1e-6 && (l.grade[3][1] - 0.1).abs() < 1e-6);
        assert!((l.grade_blend - 0.8).abs() < 1e-6);
        // Texture, sharpening, grain roughness and the vignette outline.
        // Strengths are calibrated against a photo edited in Lightroom.
        assert!((l.texture - 0.3).abs() < 1e-6 && (l.sharpen - 0.6).abs() < 1e-6);
        assert!((l.grain_rough - 0.7).abs() < 1e-6);
        assert!((l.vignette_mid - 0.06).abs() < 1e-6 && (l.vignette_feather - 0.87).abs() < 1e-6);
        assert!((l.vignette_round + 0.4).abs() < 1e-6);
    }

    #[test]
    fn finds_settings_inside_a_dng_file() {
        let mut bytes = vec![0x49, 0x49, 0x2a, 0x00, 1, 2, 3, 0, 255];
        bytes.extend_from_slice(XMP.as_bytes());
        bytes.extend_from_slice(&[0, 0, 7, 9]);
        let dir = std::env::temp_dir().join(format!("animagraph_preset_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("Mobile_Preset.dng");
        std::fs::write(&p, &bytes).unwrap();
        let preset = read(&p).unwrap();
        assert_eq!(preset.name, "Teal Leaves");
        assert!((preset.look.exposure - 0.5).abs() < 1e-6);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rgb_curves_and_parametric_curve_are_read() {
        let xmp = r#"<x:xmpmeta><rdf:Description crs:ParametricShadows="+40" crs:ParametricHighlights="-30">
 <crs:ToneCurvePV2012Blue><rdf:Seq><rdf:li>0, 30</rdf:li><rdf:li>255, 230</rdf:li></rdf:Seq></crs:ToneCurvePV2012Blue>
 </rdf:Description></x:xmpmeta>"#;
        let l = look_from_xmp(xmp);
        assert!(l.curves[1].is_identity() && l.curves[2].is_identity());
        assert!((l.curves[3].eval(0.0) - 30.0 / 255.0).abs() < 1e-4);
        // Parametric: shadows lifted, highlights lowered.
        assert!(l.curves[0].eval(0.12) > 0.12 + 0.02, "{}", l.curves[0].eval(0.12));
        assert!(l.curves[0].eval(0.87) < 0.87 - 0.02, "{}", l.curves[0].eval(0.87));
    }

    #[test]
    fn grayscale_presets_are_black_and_white() {
        let xmp = r#"<x:xmpmeta><rdf:Description crs:ConvertToGrayscale="True" crs:Contrast2012="+10"
          crs:GrayMixerBlue="-60" crs:HueAdjustmentBlue="+40"/></x:xmpmeta>"#;
        let l = look_from_xmp(xmp);
        assert_eq!(l.saturation, -1.0);
        // The gray mix replaces the color adjustments.
        assert_eq!(l.hsl[5], [0.0, 0.0, -0.6]);
    }

    #[test]
    fn profile_settings_are_not_taken_for_the_preset() {
        let xmp = r#"<x:xmpmeta><rdf:Description crs:Exposure2012="+0.30">
          <crs:Look><rdf:Description crs:Amount="1"><crs:Name>Profile Name</crs:Name>
            <crs:Parameters><rdf:Description crs:Contrast2012="+80"/></crs:Parameters></rdf:Description></crs:Look>
          <crs:Name><rdf:Alt><rdf:li xml:lang="x-default">Real Name</rdf:li></rdf:Alt></crs:Name>
        </rdf:Description></x:xmpmeta>"#;
        let clean = without_profile(xmp);
        assert_eq!(preset_name(&clean).as_deref(), Some("Real Name"));
        let l = look_from_xmp(xmp);
        assert!((l.exposure - 0.3).abs() < 1e-6 && l.contrast == 0.0);
    }

    #[test]
    fn white_balance_as_shot_is_kept() {
        let shot = r#"<x:xmpmeta><rdf:Description crs:WhiteBalance="As Shot" crs:Temperature="3200" crs:Tint="+12"/></x:xmpmeta>"#;
        let l = look_from_xmp(shot);
        assert_eq!((l.warmth, l.tint), (0.0, 0.0));
        let custom = r#"<x:xmpmeta><rdf:Description crs:WhiteBalance="Custom" crs:Temperature="7500"/></x:xmpmeta>"#;
        let warm = look_from_xmp(custom).warmth;
        assert!(warm > 0.2 && warm < 0.6, "{warm}");
    }
}
