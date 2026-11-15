//! Saving a finished loop: animated GIF, numbered PNG frames, or video
//! (MP4 / MOV). Video uses the system's encoder on macOS and ffmpeg elsewhere.

use std::path::{Path, PathBuf};
use std::process::Command;

use image::RgbaImage;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Gif,
    PngFrames,
    Mp4,
    Mov,
    /// A single still picture.
    Png,
    Jpeg,
    Heif,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Gif => "Animated GIF",
            Format::PngFrames => "PNG frames",
            Format::Mp4 => "MP4 video",
            Format::Mov => "MOV video",
            Format::Png => "PNG image (still)",
            Format::Jpeg => "JPEG image (still)",
            Format::Heif => "HEIC / HEIF image (still)",
        }
    }
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Format::Gif => Some("gif"),
            Format::PngFrames => None,
            Format::Mp4 => Some("mp4"),
            Format::Mov => Some("mov"),
            Format::Png => Some("png"),
            Format::Jpeg => Some("jpg"),
            Format::Heif => Some("heic"),
        }
    }

    pub fn is_still(self) -> bool {
        matches!(self, Format::Png | Format::Jpeg | Format::Heif)
    }

    pub fn is_video(self) -> bool {
        matches!(self, Format::Mp4 | Format::Mov)
    }
}

/// Whether video files can be written on this computer.
pub fn video_available() -> bool {
    cfg!(target_os = "macos") || ffmpeg_available()
}

pub fn ffmpeg_available() -> bool {
    crate::video::ffmpeg_path().is_some()
}

/// Save `frames`. Videos play the loop `repeats` times (players that
/// don't loop still show it more than once); GIFs loop forever.
pub fn save(format: Format, frames: &[RgbaImage], fps: u32, target: &Path, repeats: u32) -> Result<PathBuf, String> {
    match format {
        Format::Gif => save_gif(frames, fps, target),
        Format::PngFrames => save_pngs(frames, target),
        Format::Png | Format::Jpeg | Format::Heif => match frames.first() {
            Some(f) => save_still(format, f, target),
            None => Err("Nothing to save.".into()),
        },
        Format::Mp4 | Format::Mov => {
            #[cfg(target_os = "macos")]
            {
                crate::macexport::write_video(frames, fps, target, format == Format::Mov, repeats)?;
                Ok(target.to_path_buf())
            }
            #[cfg(not(target_os = "macos"))]
            save_with_ffmpeg(frames, fps, target, repeats)
        }
    }
}

/// Save one picture as PNG or JPEG.
pub fn save_still(format: Format, img: &RgbaImage, target: &Path) -> Result<PathBuf, String> {
    match format {
        Format::Jpeg => {
            let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
            let file = std::fs::File::create(target).map_err(|e| e.to_string())?;
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 92);
            enc.encode_image(&rgb).map_err(|e| e.to_string())?;
        }
        Format::Heif => save_heif(img, target)?,
        _ => img.save_with_format(target, image::ImageFormat::Png).map_err(|e| e.to_string())?,
    }
    Ok(target.to_path_buf())
}

/// HEIF (.heic) through the encoder built into macOS.
fn save_heif(img: &RgbaImage, target: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let tmp = std::env::temp_dir().join(format!("animagraph_heif_{}.png", std::process::id()));
        img.save_with_format(&tmp, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        let out = Command::new("/usr/bin/sips")
            .args(["-s", "format", "heic", "-s", "formatOptions", "90"])
            .arg(&tmp)
            .arg("--out")
            .arg(target)
            .output()
            .map_err(|e| format!("Could not start the HEIF encoder: {e}"))?;
        let _ = std::fs::remove_file(&tmp);
        if out.status.success() && target.exists() {
            Ok(())
        } else {
            Err("macOS could not save the HEIF file.".into())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (img, target);
        Err("HEIF export works on macOS only. Choose PNG or JPEG.".into())
    }
}

fn save_gif(frames: &[RgbaImage], fps: u32, target: &Path) -> Result<PathBuf, String> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let file = std::fs::File::create(target).map_err(|e| e.to_string())?;
    let mut enc = GifEncoder::new_with_speed(std::io::BufWriter::new(file), 10);
    enc.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;
    let delay = image::Delay::from_numer_denom_ms(1000, fps.max(1));
    for f in frames {
        enc.encode_frame(image::Frame::from_parts(f.clone(), 0, 0, delay))
            .map_err(|e| e.to_string())?;
    }
    Ok(target.to_path_buf())
}

fn save_pngs(frames: &[RgbaImage], folder: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    for (i, f) in frames.iter().enumerate() {
        f.save(folder.join(format!("frame_{i:04}.png"))).map_err(|e| e.to_string())?;
    }
    Ok(folder.to_path_buf())
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn save_with_ffmpeg(frames: &[RgbaImage], fps: u32, target: &Path, repeats: u32) -> Result<PathBuf, String> {
    let tmp = std::env::temp_dir().join(format!("animagraph_frames_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    save_pngs(frames, &tmp)?;
    let ffmpeg = crate::video::ffmpeg_path().ok_or("Saving video needs ffmpeg on this computer.")?;
    let out = Command::new(ffmpeg)
        .args(["-y", "-loglevel", "error", "-stream_loop"])
        .arg(repeats.saturating_sub(1).to_string())
        .arg("-framerate")
        .arg(fps.to_string())
        .arg("-i")
        .arg(tmp.join("frame_%04d.png"))
        .args([
            "-vf",
            "scale=trunc(iw/2)*2:trunc(ih/2)*2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "18",
        ])
        .arg(target)
        .output()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;
    let _ = std::fs::remove_dir_all(&tmp);
    if out.status.success() {
        Ok(target.to_path_buf())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Show a file or folder in the system file browser.
pub fn reveal(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg("/select,").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
}

#[cfg(test)]
mod tests {
    #[test]
    fn still_images_are_written_as_png_and_jpeg() {
        let img = image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 100, 50, 255]));
        let dir = std::env::temp_dir().join(format!("cg_still_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (f, name) in [(super::Format::Png, "a.png"), (super::Format::Jpeg, "a.jpg")] {
            let p = super::save_still(f, &img, &dir.join(name)).unwrap();
            let back = image::open(&p).unwrap();
            assert_eq!((back.width(), back.height()), (40, 30));
        }
        // HEIF: written by macOS, read back through the same system converter.
        #[cfg(target_os = "macos")]
        {
            let p = super::save_still(super::Format::Heif, &img, &dir.join("a.heic")).unwrap();
            let back = crate::raw::decode(&p).unwrap();
            assert_eq!(back.dimensions(), (40, 30));
            let px = back.get_pixel(5, 5);
            assert!((px[0] as i32 - 200).abs() < 15, "{px:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn gif_and_png_frames_are_written() {
        let frames: Vec<_> = (0..4).map(|i| image::RgbaImage::from_pixel(32, 18, image::Rgba([i * 60, 100, 200, 255]))).collect();
        let dir = std::env::temp_dir().join(format!("cg_export_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gif = super::save(super::Format::Gif, &frames, 24, &dir.join("a.gif"), 1).unwrap();
        assert!(std::fs::metadata(gif).unwrap().len() > 100);
        let pngs = super::save(super::Format::PngFrames, &frames, 24, &dir.join("frames"), 1).unwrap();
        assert_eq!(std::fs::read_dir(pngs).unwrap().count(), 4);
        let _ = std::fs::remove_dir_all(dir);
    }
}
