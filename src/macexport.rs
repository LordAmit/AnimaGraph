//! Writes H.264 video (MP4 or MOV) with the video support built into macOS
//! (AVFoundation), so no extra install is needed.

use std::path::Path;
use std::ptr::NonNull;

use image::RgbaImage;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVAssetWriterStatus, AVFileTypeMPEG4,
    AVFileTypeQuickTimeMovie, AVMediaTypeVideo, AVVideoAverageBitRateKey, AVVideoCodecKey, AVVideoCodecTypeH264,
    AVVideoCompressionPropertiesKey, AVVideoHeightKey, AVVideoWidthKey,
};
use objc2_core_foundation::{CFRetained, CFString};
use objc2_core_media::CMTime;
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferCreate, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

/// Write `frames` as a video, playing the loop `repeats` times.
#[allow(deprecated)]
pub fn write_video(frames: &[RgbaImage], fps: u32, target: &Path, mov: bool, repeats: u32) -> Result<(), String> {
    let first = frames.first().ok_or("Nothing to save.")?;
    // H.264 needs even sizes.
    let w = (first.width() & !1).max(2) as usize;
    let h = (first.height() & !1).max(2) as usize;
    let _ = std::fs::remove_file(target); // the writer refuses to replace a file
    unsafe {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&target.to_string_lossy()));
        let file_type = if mov { AVFileTypeQuickTimeMovie } else { AVFileTypeMPEG4 }.ok_or("Video saving is not available.")?;
        let writer = AVAssetWriter::assetWriterWithURL_fileType_error(&url, file_type)
            .map_err(|e| format!("Could not create the video: {}", e.localizedDescription()))?;

        let num = |v: f64| NSNumber::new_f64(v);
        let bitrate = ((w * h) as f64 * fps as f64 * 0.2).max(2_000_000.0);
        let bitrate_n = num(bitrate);
        let compression = NSDictionary::<NSString, AnyObject>::from_slices(
            &[AVVideoAverageBitRateKey.ok_or("missing key")?],
            &[bitrate_n.as_ref()],
        );
        let (wn, hn) = (num(w as f64), num(h as f64));
        let codec: &AnyObject = AVVideoCodecTypeH264.ok_or("H.264 is not available.")?.as_ref();
        let settings = NSDictionary::<NSString, AnyObject>::from_slices(
            &[
                AVVideoCodecKey.ok_or("missing key")?,
                AVVideoWidthKey.ok_or("missing key")?,
                AVVideoHeightKey.ok_or("missing key")?,
                AVVideoCompressionPropertiesKey.ok_or("missing key")?,
            ],
            &[codec, wn.as_ref(), hn.as_ref(), compression.as_ref()],
        );
        let media = AVMediaTypeVideo.ok_or("Video support is not available.")?;
        let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(media, Some(&settings));
        input.setExpectsMediaDataInRealTime(false);

        let key_cf: &CFString = kCVPixelBufferPixelFormatTypeKey;
        let key: &NSString = &*(key_cf as *const CFString as *const NSString);
        let fmt = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
        let attrs = NSDictionary::<NSString, AnyObject>::from_slices(&[key], &[fmt.as_ref()]);
        let adaptor =
            AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
                &input,
                Some(&attrs),
            );
        if !writer.canAddInput(&input) {
            return Err("Could not set up the video track.".into());
        }
        writer.addInput(&input);
        if !writer.startWriting() {
            return Err(failure(&writer));
        }
        writer.startSessionAtSourceTime(CMTime::new(0, fps as i32));

        let total = frames.len() * repeats.max(1) as usize;
        for i in 0..total {
            let frame = &frames[i % frames.len()];
            let buffer = make_buffer(frame, w, h)?;
            let mut waited = 0;
            while !input.isReadyForMoreMediaData() {
                std::thread::sleep(std::time::Duration::from_millis(2));
                waited += 1;
                if waited > 5000 {
                    return Err("The video encoder stopped responding.".into());
                }
            }
            if !adaptor.appendPixelBuffer_withPresentationTime(&buffer, CMTime::new(i as i64, fps as i32)) {
                return Err(failure(&writer));
            }
        }
        input.markAsFinished();
        if !writer.finishWriting() || writer.status() == AVAssetWriterStatus::Failed {
            return Err(failure(&writer));
        }
    }
    Ok(())
}

unsafe fn failure(writer: &AVAssetWriter) -> String {
    unsafe {
        writer
            .error()
            .map(|e| format!("Saving the video failed: {}", e.localizedDescription()))
            .unwrap_or_else(|| "Saving the video failed.".into())
    }
}

/// A BGRA pixel buffer holding the top-left `w` x `h` of `img`.
unsafe fn make_buffer(img: &RgbaImage, w: usize, h: usize) -> Result<CFRetained<CVPixelBuffer>, String> {
    unsafe {
        let mut raw: *mut CVPixelBuffer = std::ptr::null_mut();
        let rc = CVPixelBufferCreate(None, w, h, kCVPixelFormatType_32BGRA, None, NonNull::from(&mut raw));
        let ptr = NonNull::new(raw).filter(|_| rc == 0).ok_or("Could not make a video frame.")?;
        let buffer = CFRetained::from_raw(ptr);
        CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags::empty());
        let base = CVPixelBufferGetBaseAddress(&buffer) as *mut u8;
        let stride = CVPixelBufferGetBytesPerRow(&buffer);
        if !base.is_null() {
            let src = img.as_raw();
            let iw = img.width() as usize;
            for y in 0..h {
                let row = std::slice::from_raw_parts_mut(base.add(y * stride), w * 4);
                for x in 0..w {
                    let s = &src[(y * iw + x) * 4..(y * iw + x) * 4 + 4];
                    row[x * 4] = s[2];
                    row[x * 4 + 1] = s[1];
                    row[x * 4 + 2] = s[0];
                    row[x * 4 + 3] = 255;
                }
            }
        }
        CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags::empty());
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn writes_a_playable_mp4_that_reads_back() {
        let frames: Vec<image::RgbaImage> = (0..12u8)
            .map(|i| image::RgbaImage::from_pixel(161, 91, image::Rgba([i * 20, 120, 200, 255])))
            .collect();
        let path = std::env::temp_dir().join(format!("cg_export_{}.mp4", std::process::id()));
        super::write_video(&frames, 12, &path, false, 2).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 500);
        let back = crate::macvideo::decode(&path, 10.0, 12).unwrap();
        assert!(back.frames.len() >= 20, "{} frames", back.frames.len());
        assert_eq!(back.frames[0].dimensions(), (160, 90));
        let _ = std::fs::remove_file(path);
    }
}
