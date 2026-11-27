//! Reads video files with the video support built into macOS
//! (AVFoundation), so MP4, MOV and M4V open without installing anything.

use std::path::Path;

use image::RgbaImage;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{AVAssetReader, AVAssetReaderStatus, AVAssetReaderTrackOutput, AVMediaTypeVideo, AVURLAsset};
use objc2_core_foundation::CFString;
use objc2_core_media::{CMTime, CMTimeRange};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress, kCVPixelBufferPixelFormatTypeKey,
    kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::video::{MAX_FRAMES, MAX_SIDE, VideoSource};

/// How the phone held the camera, from the track's transform.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Turn {
    None,
    Right,
    Half,
    Left,
}

/// Read the first `seconds` of a video, keeping about `fps` frames per second.
#[allow(deprecated)]
pub fn decode(path: &Path, seconds: f32, fps: u32) -> Result<VideoSource, String> {
    unsafe {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
        let media_type = AVMediaTypeVideo.ok_or("Video support is not available on this Mac.")?;
        let tracks = asset.tracksWithMediaType(media_type);
        let track = tracks.firstObject().ok_or("This file has no video track.")?;

        let turn = Turn::of(&track);
        let native_fps = track.nominalFrameRate();
        let fps = if native_fps > 1.0 { (fps as f32).min(native_fps) } else { fps as f32 };

        let reader = AVAssetReader::assetReaderWithAsset_error(&asset)
            .map_err(|e| format!("Could not open the video: {}", e.localizedDescription()))?;
        reader.setTimeRange(CMTimeRange {
            start: CMTime::with_seconds(0.0, 600),
            duration: CMTime::with_seconds(seconds as f64, 600),
        });

        let settings = bgra_settings();

        let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(&track, Some(&settings));
        output.setAlwaysCopiesSampleData(false);
        reader.addOutput(&output);
        if !reader.startReading() {
            let why = reader
                .error()
                .map(|e| e.localizedDescription().to_string())
                .unwrap_or_else(|| "unknown reason".into());
            return Err(format!("Could not read the video: {why}"));
        }

        let step = 1.0 / fps as f64;
        let mut next_time = 0.0f64;
        let mut first_pts: Option<f64> = None;
        let mut frames = Vec::new();
        let mut native = None;
        // Apple Log clips (iPhone "Log" video) are marked; whether they are
        // converted to normal colours is chosen afterwards (VideoSource::with_log_choice).
        let mut apple_log: Option<bool> = None;
        while let Some(sample) = output.copyNextSampleBuffer() {
            if apple_log.is_none() {
                apple_log = Some(sample.format_description().is_some_and(|d| is_apple_log(&d)));
            }
            let pts = sample.presentation_time_stamp().seconds();
            let start = *first_pts.get_or_insert(pts);
            let rel = pts - start;
            if rel + 1e-3 < next_time {
                continue;
            }
            next_time += step;
            while next_time <= rel {
                next_time += step;
            }
            if let Some(img) = frame_of(&sample, turn) {
                if native.is_none() {
                    native = Some(img.dimensions());
                }
                frames.push(crate::scene::downscale(&img, MAX_SIDE));
            }
            if frames.len() >= MAX_FRAMES {
                break;
            }
        }
        if reader.status() == AVAssetReaderStatus::Failed && frames.len() < 2 {
            let why = reader
                .error()
                .map(|e| e.localizedDescription().to_string())
                .unwrap_or_else(|| "unknown reason".into());
            return Err(format!("Could not read the video: {why}"));
        }
        let mut v = VideoSource::new(frames, fps)?;
        v.apple_log = apple_log == Some(true);
        v.origin = Some(crate::video::Origin { path: path.to_path_buf(), size: native.unwrap_or(v.frames[0].dimensions()) });
        Ok(v)
    }
}

impl Turn {
    /// From the track's transform.
    pub(crate) unsafe fn of(track: &objc2_av_foundation::AVAssetTrack) -> Self {
        let t = unsafe { track.preferredTransform() };
        if t.a.abs() < 0.5 && t.b > 0.5 {
            Turn::Right
        } else if t.a.abs() < 0.5 && t.b < -0.5 {
            Turn::Left
        } else if t.a < -0.5 && t.d < -0.5 {
            Turn::Half
        } else {
            Turn::None
        }
    }

    fn apply(self, img: RgbaImage) -> RgbaImage {
        match self {
            Turn::None => img,
            Turn::Right => image::imageops::rotate90(&img),
            Turn::Half => image::imageops::rotate180(&img),
            Turn::Left => image::imageops::rotate270(&img),
        }
    }
}

/// Settings asking for plain 8-bit BGRA pixels.
pub(crate) fn bgra_settings() -> objc2::rc::Retained<NSDictionary<NSString, AnyObject>> {
    unsafe {
        let key_cf: &CFString = kCVPixelBufferPixelFormatTypeKey;
        let key: &NSString = &*(key_cf as *const CFString as *const NSString);
        let value = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
        let value_obj: &AnyObject = value.as_ref();
        NSDictionary::<NSString, AnyObject>::from_slices(&[key], &[value_obj])
    }
}

/// The picture of one BGRA video sample, upright, at full size.
pub(crate) unsafe fn frame_of(sample: &objc2_core_media::CMSampleBuffer, turn: Turn) -> Option<RgbaImage> {
    unsafe {
        let buffer = sample.image_buffer()?;
        if CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags::ReadOnly) != 0 {
            return None;
        }
        let w = CVPixelBufferGetWidth(&buffer);
        let h = CVPixelBufferGetHeight(&buffer);
        let stride = CVPixelBufferGetBytesPerRow(&buffer);
        let base = CVPixelBufferGetBaseAddress(&buffer) as *const u8;
        let frame = if base.is_null() || w == 0 || h == 0 {
            None
        } else {
            let data = std::slice::from_raw_parts(base, stride * h);
            let mut img = RgbaImage::new(w as u32, h as u32);
            for (y, row) in img.as_mut().chunks_exact_mut(w * 4).enumerate() {
                let src = &data[y * stride..y * stride + w * 4];
                for (d, s) in row.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = 255;
                }
            }
            Some(img)
        };
        CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags::ReadOnly);
        frame.map(|f| turn.apply(f))
    }
}

/// Whether a video track is marked as Apple Log.
unsafe fn is_apple_log(desc: &objc2_core_media::CMFormatDescription) -> bool {
    use objc2_core_media::{kCMFormatDescriptionExtension_LogTransferFunction, kCMFormatDescriptionLogTransferFunction_AppleLog};
    unsafe {
        desc.extension(kCMFormatDescriptionExtension_LogTransferFunction)
            .and_then(|v| v.downcast_ref::<CFString>().map(|s| s.to_string()))
            .is_some_and(|s| s == kCMFormatDescriptionLogTransferFunction_AppleLog.to_string())
    }
}
