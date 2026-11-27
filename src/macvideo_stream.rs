//! Reads a whole video file frame by frame, changes each frame and writes a
//! new H.264 video with the sound, using the video support built into macOS
//! (AVFoundation). Only one frame is held at a time, so long and 4K videos
//! fit in memory.

use std::path::Path;
use std::sync::atomic::Ordering;

use image::RgbaImage;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderStatus, AVAssetReaderTrackOutput, AVAssetTrack, AVAssetWriter, AVAssetWriterInput,
    AVAssetWriterInputPixelBufferAdaptor, AVAssetWriterStatus, AVFileTypeMPEG4, AVFileTypeQuickTimeMovie,
    AVMediaTypeAudio, AVMediaTypeVideo, AVURLAsset,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::macvideo::{Turn, bgra_settings, frame_of};
use crate::render::Progress;

/// What happened to the sound.
#[derive(Debug, PartialEq)]
pub enum Sound {
    Kept,
    /// The video has no sound.
    None,
    /// The sound could not be read; the reason.
    Dropped(String),
}

/// Read `source`, pass every frame (upright, full size) through `edit`, and
/// write the frames with the original timing, and the sound, to `target`.
#[allow(deprecated)]
pub fn transcode(
    source: &Path,
    target: &Path,
    mov: bool,
    progress: &Progress,
    edit: &mut dyn FnMut(RgbaImage) -> RgbaImage,
) -> Result<Sound, String> {
    unsafe {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&source.to_string_lossy()));
        let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
        let media = AVMediaTypeVideo.ok_or("Video support is not available on this Mac.")?;
        let track = asset.tracksWithMediaType(media).firstObject().ok_or("This file has no video track.")?;
        let turn = Turn::of(&track);
        let duration = asset.duration().seconds().max(0.001);
        let fps = track.nominalFrameRate().max(1.0);

        let (video_reader, video_out) = reader(&asset, &track, Some(&bgra_settings()))?;
        let first = video_out.copyNextSampleBuffer().ok_or("The video has no frames.")?;
        let start = first.presentation_time_stamp();
        let img = edit(frame_of(&first, turn).ok_or("Could not read the first frame.")?);
        // H.264 needs even sizes.
        let (w, h) = ((img.width() & !1).max(2) as usize, (img.height() & !1).max(2) as usize);

        // The sound, read as plain samples and written as AAC.
        let mut sound = Sound::None;
        let audio_track = AVMediaTypeAudio.and_then(|m| asset.tracksWithMediaType(m).firstObject());
        let audio = match &audio_track {
            Some(t) => match reader(&asset, t, Some(&pcm_settings())) {
                Ok(r) => Some(r),
                Err(e) => {
                    sound = Sound::Dropped(e);
                    None
                }
            },
            None => None,
        };

        let _ = std::fs::remove_file(target); // the writer refuses to replace a file
        let url = NSURL::fileURLWithPath(&NSString::from_str(&target.to_string_lossy()));
        let file_type = if mov { AVFileTypeQuickTimeMovie } else { AVFileTypeMPEG4 }.ok_or("Video saving is not available.")?;
        let writer = AVAssetWriter::assetWriterWithURL_fileType_error(&url, file_type)
            .map_err(|e| format!("Could not create the video: {}", e.localizedDescription()))?;
        let h264 = crate::macexport::h264_settings(w, h, fps as f64)?;
        let video_in = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(media, Some(&h264));
        video_in.setExpectsMediaDataInRealTime(false);
        let adaptor =
            AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
                &video_in,
                Some(&bgra_settings()),
            );
        if !writer.canAddInput(&video_in) {
            return Err("Could not set up the video track.".into());
        }
        writer.addInput(&video_in);
        let audio_in = match (&audio, AVMediaTypeAudio) {
            (Some(_), Some(m)) => {
                let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(m, Some(&aac_settings()));
                input.setExpectsMediaDataInRealTime(false);
                if writer.canAddInput(&input) {
                    writer.addInput(&input);
                    sound = Sound::Kept;
                    Some(input)
                } else {
                    sound = Sound::Dropped("the sound could not be added to this file type".into());
                    None
                }
            }
            _ => None,
        };
        let audio = if audio_in.is_some() { audio } else { None };
        if !writer.startWriting() {
            return Err(failure(&writer));
        }
        writer.startSessionAtSourceTime(start);

        let fail = |why: String| {
            writer.cancelWriting();
            let _ = std::fs::remove_file(target);
            Err(why)
        };
        let mut pending = Some((img, start));
        let mut video_done = false;
        let mut audio_done = audio.is_none();
        let mut idle = 0u32;
        while !(video_done && audio_done) {
            if progress.cancel.load(Ordering::Relaxed) {
                return fail("Canceled".into());
            }
            let mut moved = false;
            if !video_done && video_in.isReadyForMoreMediaData() {
                moved = true;
                if pending.is_none() {
                    pending = match video_out.copyNextSampleBuffer() {
                        Some(s) => frame_of(&s, turn).map(|f| (edit(f), s.presentation_time_stamp())),
                        None => None,
                    };
                }
                match pending.take() {
                    Some((img, pts)) => {
                        let buffer = crate::macexport::make_buffer(&img, w.min(img.width() as usize), h.min(img.height() as usize))?;
                        if !adaptor.appendPixelBuffer_withPresentationTime(&buffer, pts) {
                            return fail(failure(&writer));
                        }
                        let done = ((pts.seconds() - start.seconds()) / duration * 1000.0).clamp(0.0, 999.0);
                        progress.permille.store(done as u32, Ordering::Relaxed);
                    }
                    None => {
                        if video_reader.status() == AVAssetReaderStatus::Failed {
                            return fail(read_failure(&video_reader));
                        }
                        video_in.markAsFinished();
                        video_done = true;
                    }
                }
            }
            if let (false, Some((_, out)), Some(input)) = (audio_done, &audio, &audio_in) {
                if input.isReadyForMoreMediaData() {
                    moved = true;
                    match out.copyNextSampleBuffer() {
                        Some(s) => {
                            if !input.appendSampleBuffer(&s) {
                                return fail(failure(&writer));
                            }
                        }
                        None => {
                            input.markAsFinished();
                            audio_done = true;
                        }
                    }
                }
            }
            if moved {
                idle = 0;
            } else {
                idle += 1;
                if idle > 15_000 {
                    return fail("The video encoder stopped responding.".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        if !writer.finishWriting() || writer.status() == AVAssetWriterStatus::Failed {
            return Err(failure(&writer));
        }
        progress.permille.store(1000, Ordering::Relaxed);
        Ok(sound)
    }
}

/// A reader for one track, started.
unsafe fn reader(
    asset: &AVURLAsset,
    track: &AVAssetTrack,
    settings: Option<&NSDictionary<NSString, AnyObject>>,
) -> Result<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>), String> {
    unsafe {
        let reader = AVAssetReader::assetReaderWithAsset_error(asset)
            .map_err(|e| format!("Could not open the video: {}", e.localizedDescription()))?;
        let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(track, settings);
        output.setAlwaysCopiesSampleData(false);
        if !reader.canAddOutput(&output) {
            return Err("This track cannot be read.".into());
        }
        reader.addOutput(&output);
        if !reader.startReading() {
            return Err(read_failure(&reader));
        }
        Ok((reader, output))
    }
}

/// A dictionary of AVFoundation settings, by key name.
fn settings(pairs: &[(&str, Retained<NSNumber>)]) -> Retained<NSDictionary<NSString, AnyObject>> {
    let keys: Vec<Retained<NSString>> = pairs.iter().map(|(k, _)| NSString::from_str(k)).collect();
    let keys: Vec<&NSString> = keys.iter().map(|k| &**k).collect();
    let values: Vec<&AnyObject> = pairs.iter().map(|(_, v)| v.as_ref()).collect();
    NSDictionary::from_slices(&keys, &values)
}

const LINEAR_PCM: u32 = u32::from_be_bytes(*b"lpcm");
const AAC: u32 = u32::from_be_bytes(*b"aac ");
const SAMPLE_RATE: f64 = 48_000.0;

/// Sound read as 16-bit stereo samples at 48 kHz.
fn pcm_settings() -> Retained<NSDictionary<NSString, AnyObject>> {
    settings(&[
        ("AVFormatIDKey", NSNumber::new_u32(LINEAR_PCM)),
        ("AVSampleRateKey", NSNumber::new_f64(SAMPLE_RATE)),
        ("AVNumberOfChannelsKey", NSNumber::new_u32(2)),
        ("AVLinearPCMBitDepthKey", NSNumber::new_u32(16)),
        ("AVLinearPCMIsFloatKey", NSNumber::new_bool(false)),
        ("AVLinearPCMIsBigEndianKey", NSNumber::new_bool(false)),
        ("AVLinearPCMIsNonInterleaved", NSNumber::new_bool(false)),
    ])
}

/// Sound written as stereo AAC, which MP4 and MOV both hold.
fn aac_settings() -> Retained<NSDictionary<NSString, AnyObject>> {
    settings(&[
        ("AVFormatIDKey", NSNumber::new_u32(AAC)),
        ("AVSampleRateKey", NSNumber::new_f64(SAMPLE_RATE)),
        ("AVNumberOfChannelsKey", NSNumber::new_u32(2)),
        ("AVEncoderBitRateKey", NSNumber::new_u32(192_000)),
    ])
}

unsafe fn failure(writer: &AVAssetWriter) -> String {
    unsafe {
        writer
            .error()
            .map(|e| format!("Saving the video failed: {}", e.localizedDescription()))
            .unwrap_or_else(|| "Saving the video failed.".into())
    }
}

unsafe fn read_failure(reader: &AVAssetReader) -> String {
    unsafe {
        let why = reader.error().map(|e| e.localizedDescription().to_string()).unwrap_or_else(|| "unknown reason".into());
        format!("Could not read the video: {why}")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Length in seconds and number of sound tracks of a saved video.
    pub(crate) fn length_and_sound(path: &Path) -> (f64, usize) {
        unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
            #[allow(deprecated)]
            let sound = AVMediaTypeAudio.map_or(0, |m| asset.tracksWithMediaType(m).count());
            (asset.duration().seconds(), sound)
        }
    }

    /// The iPhone clip in reference/ (4K ProRes, sound as plain samples),
    /// saved whole with edits. Slow: cargo test reference_video -- --ignored --nocapture
    #[test]
    #[ignore]
    fn reference_video() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/IMG_6970.MOV");
        if !source.exists() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("animagraph_ref_video_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, mov) in [("out.mp4", false), ("out.mov", true)] {
            let mut edits = crate::scene::Edits::default();
            edits.look.exposure = 0.4;
            edits.border.enabled = true;
            let job = crate::video_export::VideoJob {
                source: source.clone(),
                target: dir.join(name),
                mov,
                crop: crate::crop::CropRect { x: 0.0, y: 0.1, w: 1.0, h: 0.75 },
                edits,
                size: crate::export_size::ExportSize::INSTAGRAM,
                quarter_turns: 0,
                angle: 0.0,
                from_log: true,
            };
            let t = std::time::Instant::now();
            let (saved, note) = job.run(&Progress::new()).unwrap();
            let (secs, sound) = length_and_sound(&saved);
            let back = crate::macvideo::decode(&saved, 1.0, 30).unwrap();
            println!(
                "{name}: {:.1}s to save, {secs:.2}s long, {sound} sound track(s), note {note:?}, {} bytes, frame {:?}",
                t.elapsed().as_secs_f32(),
                std::fs::metadata(&saved).unwrap().len(),
                back.origin.as_ref().map(|o| o.size)
            );
            back.frames[0].save(dir.join(format!("{name}.png"))).unwrap();
            assert!((secs - 4.83).abs() < 0.1 && sound == 1);
            assert_eq!(back.origin.map(|o| o.size.0), Some(1080));
        }
        println!("frames in {}", dir.display());
    }
}
