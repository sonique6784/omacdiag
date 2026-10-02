use super::*;
use image::{ColorType, codecs::jpeg::JpegEncoder};
use serde_json::json;
use std::{
    collections::VecDeque,
    io::Read,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const RATE: usize = 48000;
// Stream-local gain avoids changing mixer settings, including on cancellation.
const TONE_LEVEL: f64 = 0.30;
const REPLAY_PEAK: f64 = 0.35;
const MAX_REPLAY_GAIN: f64 = 12.0;
pub fn execute(req: &WorkerRequest, d: &Device) -> TestResult {
    match req.test.probe.as_str() {
        "camera" | "camera_quality" => camera(req, d),
        "microphone" | "microphone_speech" => microphone(req, d),
        "speaker" | "speaker_quality" => speaker(req, d),
        "acoustic" => acoustic(req, d),
        "audio_jack" => audio_jack(req, d),
        _ => problem(req, "Unknown media probe"),
    }
}
pub fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for b in data.chunks(3) {
        let n = ((b[0] as u32) << 16)
            | ((b.get(1).copied().unwrap_or(0) as u32) << 8)
            | (b.get(2).copied().unwrap_or(0) as u32);
        s.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        s.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        s.push(if b.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        s.push(if b.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}
fn emit(test: &TestSpec, payload: serde_json::Value) {
    crate::protocol::emit_raw(json!({"type":"media","payload":{"test_id":test.id,"data":payload}}));
}
const PREVIEW_WIDTH: u32 = 320;
const PREVIEW_HEIGHT: u32 = 240;
const PREVIEW_BYTES: usize = (PREVIEW_WIDTH * PREVIEW_HEIGHT * 3) as usize;
const PREVIEW_FILTER: &str =
    "scale=320:240:force_original_aspect_ratio=decrease,pad=320:240:(ow-iw)/2:(oh-ih)/2";

fn camera_command(
    node: &str,
    mode: Option<&crate::camera::Mode>,
    duration: bool,
) -> std::process::Command {
    let mut command = process::command("ffmpeg", &[]);
    command.args([
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "info",
        "-nostats",
        "-f",
        "v4l2",
    ]);
    if let Some(mode) = mode {
        if let Some(format) = mode.ffmpeg_format() {
            command.args(["-input_format", format]);
        }
        command.args(["-video_size", &mode.resolution()]);
        if let Some(rate) = &mode.frame_rate {
            command.args(["-framerate", rate]);
        }
    }
    command.args([
        "-i",
        node,
        "-an",
        "-vf",
        PREVIEW_FILTER,
        "-fps_mode",
        "passthrough",
    ]);
    if duration {
        command.args(["-t", "4", "-frames:v", "120"]);
    }
    command.args(["-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

struct CameraCapture {
    frames: usize,
    fps: f64,
    variance: f64,
    change: f64,
    error: String,
    source_resolution: Option<(u32, u32)>,
}

fn ffmpeg_input_resolution(log: &str) -> Option<(u32, u32)> {
    let mut in_input = false;
    for line in log.lines() {
        if line.starts_with("Input #") {
            in_input = true;
        } else if line.starts_with("Output #") {
            break;
        }
        if !in_input || !line.contains("Stream #0:0: Video:") {
            continue;
        }
        for token in line.split(|c: char| c.is_whitespace() || c == ',') {
            if let Some((width, height)) = token.split_once('x')
                && let (Ok(width), Ok(height)) = (width.parse::<u32>(), height.parse::<u32>())
                && width > 0
                && height > 0
            {
                return Some((width, height));
            }
        }
    }
    None
}

fn fps_below_advertised(delivered: f64, advertised: Option<f64>) -> bool {
    advertised.is_some_and(|fps| fps > 0.0 && delivered < fps * 0.9)
}

#[cfg(test)]
mod camera_tests {
    use super::{ffmpeg_input_resolution, fps_below_advertised};

    #[test]
    fn records_input_resolution_instead_of_scaled_output() {
        let log = "Input #0, video4linux2,v4l2, from '/dev/video0':\n  Stream #0:0: Video: mjpeg, yuvj422p, 1280x720, 29.97 fps\nOutput #0, rawvideo, to 'pipe:1':\n  Stream #0:0: Video: rawvideo, rgb24, 320x240\n";
        assert_eq!(ffmpeg_input_resolution(log), Some((1280, 720)));
        assert_eq!(
            ffmpeg_input_resolution(
                "Output #0, rawvideo\n  Stream #0:0: Video: rawvideo, rgb24, 320x240"
            ),
            None
        );
    }

    #[test]
    fn camera_frame_rate_needs_ninety_percent_of_advertised() {
        assert!(fps_below_advertised(15.0, Some(29.97)));
        assert!(!fps_below_advertised(27.0, Some(29.97)));
        assert!(!fps_below_advertised(15.0, None));
    }
}

fn capture_camera(
    req: &WorkerRequest,
    node: &str,
    mode: Option<&crate::camera::Mode>,
) -> Result<CameraCapture, String> {
    let mut command = camera_command(node, mode, true);
    process::bind_parent(&mut command);
    let mut child = command.spawn().map_err(|e| format!("spawn: {e}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let errors = thread::spawn(move || {
        let mut text = String::new();
        let _ = Read::by_ref(&mut stderr)
            .take(16384)
            .read_to_string(&mut text);
        text
    });
    let mut first_frame_at = None::<Instant>;
    let mut last_frame_at = None::<Instant>;
    let mut previous = Vec::<u8>::new();
    let mut frames = 0;
    let mut differences = Vec::new();
    let mut variance = 0.0;
    for _ in 0..120 {
        let mut frame = vec![0; PREVIEW_BYTES];
        if stdout.read_exact(&mut frame).is_err() {
            break;
        }
        let now = Instant::now();
        first_frame_at.get_or_insert(now);
        last_frame_at = Some(now);
        let mean = frame.iter().map(|b| *b as f64).sum::<f64>() / frame.len() as f64;
        variance = frame
            .iter()
            .map(|b| (*b as f64 - mean).powi(2))
            .sum::<f64>()
            / frame.len() as f64;
        if !previous.is_empty() {
            differences.push(
                frame
                    .iter()
                    .zip(&previous)
                    .map(|(a, b)| (*a as f64 - *b as f64).abs())
                    .sum::<f64>()
                    / frame.len() as f64,
            );
        }
        let mut jpeg = Vec::new();
        if JpegEncoder::new_with_quality(&mut jpeg, 65)
            .encode(
                &frame,
                PREVIEW_WIDTH,
                PREVIEW_HEIGHT,
                ColorType::Rgb8.into(),
            )
            .is_ok()
        {
            emit(
                &req.test,
                json!({"preview":"data:image/jpeg;base64,".to_string()+&base64(&jpeg),"frame":frames}),
            );
        }
        previous = frame;
        frames += 1;
    }
    let _ = child.kill();
    let _ = child.wait();
    let error = errors.join().unwrap_or_default();
    let source_resolution = ffmpeg_input_resolution(&error);
    let fps = match (first_frame_at, last_frame_at) {
        (Some(first), Some(last)) if frames > 1 && last > first => {
            (frames - 1) as f64 / last.duration_since(first).as_secs_f64()
        }
        _ => 0.0,
    };
    Ok(CameraCapture {
        frames,
        fps,
        variance,
        change: differences.iter().copied().fold(0.0, f64::max),
        error,
        source_resolution,
    })
}

fn add_camera_modes(result: &mut TestResult, device: &Device, verified: Option<(u32, u32)>) {
    if let (Some(width), Some(height)) = (
        device.attributes.get("max_capture_width"),
        device.attributes.get("max_capture_height"),
    ) {
        result.measure(
            "maximum_advertised_resolution",
            format!("{width}x{height}"),
            "pixels",
        );
        if let Ok(width) = width.parse::<u32>() {
            result.measure("maximum_advertised_width", width, "px");
        }
        if let Ok(height) = height.parse::<u32>() {
            result.measure("maximum_advertised_height", height, "px");
        }
        if let Some(format) = device.attributes.get("max_capture_format") {
            result.measure("maximum_advertised_format", format, "fourcc");
        }
        if let Some(fps) = device
            .attributes
            .get("max_capture_fps")
            .and_then(|s| s.parse::<f64>().ok())
        {
            result.measure("maximum_advertised_fps", fps, "frames/s");
        }
    } else {
        result
            .evidence
            .push("Maximum capture resolution could not be enumerated".into());
    }
    if let (Some(width), Some(height)) = (
        device.attributes.get("selected_capture_width"),
        device.attributes.get("selected_capture_height"),
    ) {
        result.measure(
            "requested_capture_resolution",
            format!("{width}x{height}"),
            "pixels",
        );
        if let Some(format) = device.attributes.get("selected_capture_format") {
            result.measure("requested_capture_format", format, "fourcc");
        }
        if let Some(fps) = device
            .attributes
            .get("selected_capture_fps")
            .and_then(|s| s.parse::<f64>().ok())
        {
            result.measure("requested_capture_fps", fps, "frames/s");
        }
    }
    if let Some((width, height)) = verified {
        result.measure(
            "verified_capture_resolution",
            format!("{width}x{height}"),
            "pixels",
        );
        result.measure("verified_capture_width", width, "px");
        result.measure("verified_capture_height", height, "px");
    } else {
        result
            .evidence
            .push("Native capture resolution remains unverified".into());
    }
}

fn camera(req: &WorkerRequest, d: &Device) -> TestResult {
    let Some(node) = &d.node else {
        let mut result = TestResult::new(
            &req.test,
            Outcome::Blocked,
            "capture_endpoint_missing",
            "Camera exists on a bus but no V4L2 endpoint is available; check driver/firmware evidence",
        );
        add_camera_modes(&mut result, d, None);
        return result;
    };
    let selected = crate::camera::selected_from_device(d);
    let first = capture_camera(req, node, selected.as_ref());
    let (capture, used_selected) = match first {
        Ok(capture) if capture.frames > 0 => (capture, selected.is_some()),
        other if selected.is_some() => {
            let fallback = capture_camera(req, node, None);
            match fallback {
                Ok(capture) if capture.frames > 0 => (capture, false),
                _ => {
                    let error = match other {
                        Ok(c) => c.error,
                        Err(e) => e,
                    };
                    let mut result = problem(
                        req,
                        if error.is_empty() {
                            "No frames delivered".into()
                        } else {
                            error
                        },
                    );
                    add_camera_modes(&mut result, d, None);
                    return result;
                }
            }
        }
        Ok(capture) => {
            let mut result = problem(
                req,
                if capture.error.is_empty() {
                    "No frames delivered".into()
                } else {
                    capture.error
                },
            );
            add_camera_modes(&mut result, d, None);
            return result;
        }
        Err(error) => {
            let mut result = problem(req, error);
            add_camera_modes(&mut result, d, None);
            return result;
        }
    };
    let good = capture.frames >= 2 && capture.variance > 4.0 && capture.change > 0.1;
    let verified = capture.source_resolution;
    let native_verified = used_selected
        && selected
            .as_ref()
            .is_some_and(|mode| verified == Some((mode.width, mode.height)));
    let slow = native_verified
        && fps_below_advertised(capture.fps, selected.as_ref().and_then(|mode| mode.fps));
    let unverified_native = selected.is_some() && !native_verified;
    let mut r = TestResult::new(
        &req.test,
        if good && !slow && !unverified_native {
            Outcome::Passed
        } else {
            Outcome::Inconclusive
        },
        if slow {
            "capture_fps_below_advertised"
        } else if unverified_native {
            "native_capture_unverified"
        } else if good {
            "frames_captured"
        } else {
            "image_content_unverified"
        },
        if slow {
            "Captured below 90% of the advertised frame rate; lighting, driver, and processing limits remain possible"
        } else {
            "Frames captured; a static/dark scene cannot establish whether the camera is faulty"
        },
    );
    r.measure("frames", capture.frames, "count");
    r.measure("delivered_fps", capture.fps, "frames/s");
    r.measure("last_frame_variance", capture.variance, "pixel_value²");
    r.measure("maximum_frame_change", capture.change, "pixel_value");
    add_camera_modes(&mut r, d, verified);
    if !used_selected && selected.is_some() {
        r.evidence.push(
            "Requested native mode failed; fallback capture used the device's default mode".into(),
        );
    } else if let (Some(mode), Some((width, height))) = (selected.as_ref(), verified)
        && (width, height) != (mode.width, mode.height)
    {
        r.evidence.push(format!(
            "Requested {} capture, but FFmpeg received {width}x{height}",
            mode.resolution()
        ));
    }
    r
}

pub struct CameraPreview {
    child: Arc<Mutex<std::process::Child>>,
    stop_requested: Arc<AtomicBool>,
    reader: Option<thread::JoinHandle<()>>,
    sender: Option<thread::JoinHandle<()>>,
    errors: Option<thread::JoinHandle<()>>,
    emit: crate::engine::EventSink,
    test_id: String,
}

impl CameraPreview {
    pub fn start(
        device: &Device,
        test: &TestSpec,
        emit: crate::engine::EventSink,
    ) -> Result<Self, String> {
        let node = device
            .node
            .as_deref()
            .ok_or("Camera capture endpoint is unavailable")?;
        let mode = crate::camera::preview_from_device(device)
            .or_else(|| crate::camera::selected_from_device(device));
        let mut command = camera_command(node, mode.as_ref(), false);
        process::isolated(&mut command);
        let mut child = command
            .spawn()
            .map_err(|e| format!("Could not start camera preview: {e}"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or("Camera preview has no video stream")?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or("Camera preview has no error stream")?;
        let child = Arc::new(Mutex::new(child));
        let stop_requested = Arc::new(AtomicBool::new(false));
        emit(
            "media",
            json!({"test_id":test.id,"data":{
                "preview_state":"starting",
                "preview_source_resolution":mode.as_ref().map(crate::camera::Mode::resolution)
            }}),
        );
        let errors = thread::spawn(move || {
            let mut message = String::new();
            let _ = Read::by_ref(&mut stderr)
                .take(16384)
                .read_to_string(&mut message);
            if message.contains("Error") || message.contains("error") {
                eprintln!("camera preview: {}", message.trim());
            }
        });
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
        let reader = thread::spawn(move || {
            loop {
                let mut frame = vec![0; PREVIEW_BYTES];
                if stdout.read_exact(&mut frame).is_err() {
                    break;
                }
                // Never queue old video frames behind the technician's live view.
                if let Err(mpsc::TrySendError::Disconnected(_)) = tx.try_send(frame) {
                    break;
                }
            }
        });
        let test_id = test.id.clone();
        let sender_id = test_id.clone();
        let sender_emit = emit.clone();
        let sender_stop = stop_requested.clone();
        let sender = thread::spawn(move || {
            let mut sent_at = VecDeque::<Instant>::new();
            let mut last_sent = None::<Instant>;
            loop {
                let frame = match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(frame) => frame,
                    Err(mpsc::RecvTimeoutError::Timeout)
                        if !sender_stop.load(Ordering::Relaxed) =>
                    {
                        continue;
                    }
                    Err(_) => break,
                };
                if sender_stop.load(Ordering::Relaxed) {
                    break;
                }
                let now = Instant::now();
                if last_sent
                    .is_some_and(|last| now.duration_since(last) < Duration::from_millis(33))
                {
                    continue;
                }
                let mut jpeg = Vec::new();
                if JpegEncoder::new_with_quality(&mut jpeg, 65)
                    .encode(
                        &frame,
                        PREVIEW_WIDTH,
                        PREVIEW_HEIGHT,
                        ColorType::Rgb8.into(),
                    )
                    .is_err()
                {
                    continue;
                }
                last_sent = Some(now);
                sent_at.push_back(now);
                while sent_at
                    .front()
                    .is_some_and(|first| now.duration_since(*first) > Duration::from_secs(1))
                {
                    sent_at.pop_front();
                }
                sender_emit(
                    "media",
                    json!({"test_id":sender_id,"data":{
                        "preview":"data:image/jpeg;base64,".to_string()+&base64(&jpeg),
                        "preview_state":"live", "preview_fps":sent_at.len()
                    }}),
                );
            }
            if !sender_stop.load(Ordering::Relaxed) {
                sender_emit(
                    "media",
                    json!({"test_id":sender_id,"data":{"preview_state":"unavailable"}}),
                );
            }
        });
        Ok(Self {
            child,
            stop_requested,
            reader: Some(reader),
            sender: Some(sender),
            errors: Some(errors),
            emit,
            test_id,
        })
    }

    pub fn stop(&mut self) {
        if self.reader.is_none() {
            return;
        }
        self.stop_requested.store(true, Ordering::Relaxed);
        if let Ok(mut child) = self.child.lock()
            && child.try_wait().ok().flatten().is_none()
        {
            process::terminate(&mut child);
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(sender) = self.sender.take() {
            let _ = sender.join();
        }
        if let Some(errors) = self.errors.take() {
            let _ = errors.join();
        }
        (self.emit)(
            "media",
            json!({"test_id":self.test_id,"data":{"preview_state":"stopped"}}),
        );
    }
}

impl Drop for CameraPreview {
    fn drop(&mut self) {
        self.stop();
    }
}

fn pcm_args(d: &Device, seconds: &str) -> Option<Vec<String>> {
    let endpoint = d.attributes.get("alsa_device")?;
    Some(vec![
        "-q".into(),
        "-D".into(),
        endpoint.clone(),
        "-t".into(),
        "raw".into(),
        "-f".into(),
        "S16_LE".into(),
        "-c".into(),
        "1".into(),
        "-r".into(),
        RATE.to_string(),
        "-d".into(),
        seconds.into(),
    ])
}
pub fn pcm(data: &[u8]) -> Vec<f64> {
    data.as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f64 / 32768.0)
        .collect()
}
pub fn levels(samples: &[f64]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (-120.0, 0.0, 0.0);
    }
    let rms = (samples.iter().map(|s| s * s).sum::<f64>() / samples.len() as f64).sqrt();
    let peak = samples.iter().map(|s| s.abs()).fold(0.0, f64::max);
    let clipping =
        samples.iter().filter(|s| s.abs() >= 0.999).count() as f64 / samples.len() as f64;
    (20.0 * rms.max(1e-6).log10(), peak, clipping)
}
fn microphone(req: &WorkerRequest, d: &Device) -> TestResult {
    if d.attributes
        .get("physical_input")
        .is_some_and(|v| v == "false")
        || d.driver.as_deref() == Some("snd_aloop")
        || d.attributes
            .get("alsa_device")
            .is_some_and(|s| s.contains("Loopback"))
    {
        return TestResult::new(
            &req.test,
            Outcome::Blocked,
            "physical_input_required",
            "Software loopback cannot verify a microphone",
        );
    }
    let Some(args) = pcm_args(d, "3") else {
        return problem(req, "No ALSA capture endpoint");
    };
    let refs: Vec<_> = args.iter().map(String::as_str).collect();
    let mut command = process::command("arecord", &refs);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    process::bind_parent(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return problem(req, format!("spawn: {e}")),
    };
    let mut stream = child.stdout.take().unwrap();
    let mut errors = child.stderr.take().unwrap();
    let error_reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = Read::by_ref(&mut errors)
            .take(16384)
            .read_to_string(&mut text);
        text
    });
    let mut samples = Vec::with_capacity(RATE * 3);
    for _ in 0..15 {
        let mut chunk = vec![0_u8; RATE / 5 * 2];
        if stream.read_exact(&mut chunk).is_err() {
            break;
        }
        let values = pcm(&chunk);
        let (db, peak, clip) = levels(&values);
        emit(
            &req.test,
            json!({"rms_dbfs":db,"peak":peak,"clipping_fraction":clip}),
        );
        samples.extend(values);
    }
    let status = child.wait();
    let errors = error_reader.join().unwrap_or_default();
    if !status.is_ok_and(|s| s.success()) || samples.len() < RATE {
        return problem(
            req,
            if errors.is_empty() {
                "Capture did not deliver enough samples".into()
            } else {
                errors
            },
        );
    }
    let (db, peak, clip) = levels(&samples);
    emit(
        &req.test,
        json!({"rms_dbfs":db,"peak":peak,"clipping_fraction":clip}),
    );
    let good = samples.len() >= RATE && db > -65.0 && clip < 0.01;
    let mut r = TestResult::new(
        &req.test,
        if good {
            Outcome::Passed
        } else {
            Outcome::Inconclusive
        },
        if good {
            "physical_signal_captured"
        } else {
            "microphone_signal_unverified"
        },
        "Physical PCM capture measured; silence, room noise and gain may affect interpretation",
    );
    r.measure("rms", db, "dBFS");
    r.measure("peak", peak, "normalized");
    r.measure("clipping", clip * 100.0, "%");
    r.measure("captured_samples", samples.len(), "samples");
    if req.test.probe == "microphone_speech" {
        match replay_capture(req, &samples) {
            Ok((endpoint, gain)) => {
                r.measure("replay_endpoint", endpoint, "identifier");
                r.measure("replay_gain", gain, "ratio");
                r.evidence.push("The captured sample was played once through the analog output; no recording or mixer change was retained".into());
            }
            Err(error) => {
                r.outcome = Outcome::Inconclusive;
                r.reason_code = "captured_audio_replay_unavailable".into();
                r.summary = "Microphone samples were captured, but replay for technician listening was unavailable".into();
                r.evidence.push(error.chars().take(2000).collect());
            }
        }
    }
    r
}
fn replay_pcm(samples: &[f64]) -> (Vec<u8>, f64) {
    let peak = samples.iter().map(|s| s.abs()).fold(0.0, f64::max);
    let gain = if peak > 0.0 {
        (REPLAY_PEAK / peak).min(MAX_REPLAY_GAIN)
    } else {
        1.0
    };
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let value = (sample * gain).clamp(-0.9, 0.9);
        bytes.extend_from_slice(&((value * 32767.0) as i16).to_le_bytes());
    }
    (bytes, gain)
}
fn replay_capture(req: &WorkerRequest, samples: &[f64]) -> Result<(String, f64), String> {
    let outputs: Vec<_> = req
        .inventory
        .devices
        .iter()
        .filter(|device| {
            device.kind == Kind::Speaker
                && device.recognition == Recognition::Detected
                && device
                    .attributes
                    .get("playback_route")
                    .is_some_and(|route| route == "analog")
                && device.attributes.contains_key("alsa_device")
        })
        .collect();
    let [output] = outputs.as_slice() else {
        return Err(
            "An unambiguous analog playback endpoint is required to replay the microphone sample"
                .into(),
        );
    };
    if output.sysfs_path.is_some() && !selected_device_is_current(output) {
        return Err(
            "Analog playback endpoint changed since inventory; refresh before replay".into(),
        );
    }
    let endpoint = output.attributes.get("alsa_device").unwrap().clone();
    let (bytes, gain) = replay_pcm(samples);
    let result = run(
        "aplay",
        &[
            "-q", "-D", &endpoint, "-t", "raw", "-f", "S16_LE", "-c", "1", "-r", "48000",
        ],
        Some(bytes),
        5,
        16384,
    )?;
    if result.code != Some(0) {
        return Err(format!("Microphone replay failed: {}", result.stderr));
    }
    Ok((endpoint, gain))
}
fn tone_sequence(seed: u64) -> (Vec<i16>, Vec<f64>) {
    let frames = RATE * 3;
    let mut stereo = Vec::with_capacity(frames * 2);
    let mut mono = Vec::with_capacity(frames);
    for i in 0..frames {
        let segment = i / (RATE / 4);
        let hz = [317.0, 503.0, 719.0, 887.0][((segment as u64 + seed) % 4) as usize]
            + (seed % 37) as f64;
        let local = (i % (RATE / 4)) as f64 / RATE as f64;
        let envelope = (local / 0.01).min(1.0) * ((0.25 - local) / 0.01).min(1.0);
        let value =
            TONE_LEVEL * envelope * (std::f64::consts::TAU * hz * i as f64 / RATE as f64).sin();
        let sample = (value * 32767.0) as i16;
        if i < frames / 2 {
            stereo.extend([sample, 0]);
        } else {
            stereo.extend([0, sample]);
        }
        mono.push(value);
    }
    (stereo, mono)
}
fn wav(samples: &[i16]) -> Vec<u8> {
    let size = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(size as usize + 44);
    b.extend(b"RIFF");
    b.extend((size + 36).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16_u32.to_le_bytes());
    b.extend(1_u16.to_le_bytes());
    b.extend(2_u16.to_le_bytes());
    b.extend((RATE as u32).to_le_bytes());
    b.extend((RATE as u32 * 4).to_le_bytes());
    b.extend(4_u16.to_le_bytes());
    b.extend(16_u16.to_le_bytes());
    b.extend(b"data");
    b.extend(size.to_le_bytes());
    for s in samples {
        b.extend(s.to_le_bytes());
    }
    b
}
fn playback(d: &Device, seed: u64) -> Result<Vec<f64>, String> {
    let output = d
        .attributes
        .get("alsa_device")
        .ok_or("No explicit ALSA output")?;
    let (samples, reference) = tone_sequence(seed);
    let o = run(
        "aplay",
        &["-q", "-D", output],
        Some(wav(&samples)),
        5,
        16384,
    )?;
    if o.code != Some(0) {
        return Err(o.stderr);
    }
    Ok(reference)
}
fn speaker(req: &WorkerRequest, d: &Device) -> TestResult {
    match playback(d, 17) {
        Ok(_) => {
            let mut r = TestResult::new(
                &req.test,
                if req.test.probe == "speaker_quality" {
                    Outcome::Passed
                } else {
                    Outcome::Inconclusive
                },
                "playback_stream_delivered",
                "Left/right tones delivered to the explicit PCM endpoint. Acoustic or operator evidence is required to establish audible output",
            );
            r.evidence.push(format!(
                "ALSA endpoint: {}",
                d.attributes.get("alsa_device").unwrap_or(&String::new())
            ));
            r.measure("playback_signal_peak", TONE_LEVEL, "normalized");
            r.evidence.push(
                "Test signal gain is stream-local; system mixer volume is not changed".into(),
            );
            r
        }
        Err(e) => problem(req, e),
    }
}
pub fn correlation(reference: &[f64], recording: &[f64]) -> (f64, usize) {
    if reference.is_empty() || recording.len() < reference.len() {
        return (0.0, 0);
    }
    let energy = reference.iter().map(|v| v * v).sum::<f64>();
    let mut best = (0.0, 0);
    for lag in (0..=recording.len() - reference.len()).step_by(3) {
        let window = &recording[lag..lag + reference.len()];
        let dot = reference
            .iter()
            .zip(window)
            .map(|(a, b)| a * b)
            .sum::<f64>();
        let measured = window.iter().map(|v| v * v).sum::<f64>();
        let score = dot.abs() / (energy * measured).sqrt().max(1e-12);
        if score > best.0 {
            best = (score, lag);
        }
    }
    best
}
fn correlation_near(
    reference: &[f64],
    recording: &[f64],
    center: usize,
    radius: usize,
) -> (f64, usize) {
    if reference.is_empty() || recording.len() < reference.len() {
        return (0.0, center);
    }
    let energy = reference.iter().map(|value| value * value).sum::<f64>();
    let mut best = (0.0, center);
    let last = recording.len() - reference.len();
    for lag in center.saturating_sub(radius)..=center.saturating_add(radius).min(last) {
        let window = &recording[lag..lag + reference.len()];
        let dot = reference
            .iter()
            .zip(window)
            .map(|(a, b)| a * b)
            .sum::<f64>();
        let measured = window.iter().map(|value| value * value).sum::<f64>();
        let score = dot.abs() / (energy * measured).sqrt().max(1e-12);
        if score > best.0 {
            best = (score, lag);
        }
    }
    best
}
fn acoustic(req: &WorkerRequest, d: &Device) -> TestResult {
    let candidates: Vec<_> = req
        .inventory
        .devices
        .iter()
        .filter(|d| {
            d.kind == Kind::Microphone
                && d.recognition == Recognition::Detected
                && d.attributes
                    .get("physical_input")
                    .is_none_or(|v| v == "true")
                && d.driver.as_deref() != Some("snd_aloop")
                && !d
                    .attributes
                    .get("alsa_device")
                    .is_some_and(|s| s.contains("Loopback"))
        })
        .collect();
    let mic = if let Some(id) = &req.microphone_id {
        candidates.iter().find(|m| m.id == *id).copied()
    } else if candidates.len() == 1 {
        Some(candidates[0])
    } else {
        None
    };
    let Some(mic) = mic else {
        return TestResult::new(
            &req.test,
            Outcome::Blocked,
            "physical_microphone_selection_required",
            "Select a physical microphone explicitly when multiple inputs exist; unavailable capture cannot diagnose a speaker",
        );
    };
    if mic.sysfs_path.is_some() && !selected_device_is_current(mic) {
        return TestResult::new(
            &req.test,
            Outcome::Blocked,
            "device_identity_changed",
            "Selected physical microphone changed since inventory; refresh before acoustic testing",
        );
    }
    let Some(args) = pcm_args(mic, "6") else {
        return problem(req, "No microphone endpoint");
    };
    let capture = thread::spawn(move || {
        let refs: Vec<_> = args.iter().map(String::as_str).collect();
        run("arecord", &refs, None, 8, RATE * 2 * 7)
    });
    thread::sleep(Duration::from_millis(600));
    let seed = crate::report::timestamp().parse::<u64>().unwrap_or(0);
    let play = playback(d, seed);
    let recorded = match capture.join() {
        Ok(Ok(o)) if o.code == Some(0) => pcm(&o.stdout),
        Ok(Ok(o)) => return problem(req, o.stderr),
        Ok(Err(e)) => return problem(req, e),
        Err(_) => return problem(req, "Capture worker failed"),
    };
    let reference = match play {
        Ok(r) => r,
        Err(e) => return problem(req, e),
    };
    let (db, _, clip) = levels(&recorded);
    if recorded.len() < RATE * 4 {
        return problem(
            req,
            "Insufficient captured samples for acoustic verification",
        );
    }
    let baseline = levels(&recorded[..RATE / 3]).0;
    let sampled: Vec<_> = recorded.iter().step_by(16).copied().collect();
    let expected: Vec<_> = reference.iter().step_by(16).copied().collect();
    let (score, lag) = correlation(&expected, &sampled);
    let half = expected.len() / 2;
    // Each half is allowed a small independent timing correction for capture/playback
    // clock drift and room reflections, but must remain near the global match.
    let radius = RATE / 16 / 40; // 25 ms at the downsampled 3 kHz rate.
    let (left, left_lag) = correlation_near(&expected[..half], &sampled, lag, radius);
    let (right, right_lag) = correlation_near(&expected[half..], &sampled, lag + half, radius);
    let snr = db - baseline;
    let verified = score > 0.35 && left > 0.25 && right > 0.25 && snr > 10.0 && clip < 0.01;
    let mut r = TestResult::new(
        &req.test,
        if verified {
            Outcome::Passed
        } else {
            Outcome::Inconclusive
        },
        if verified {
            "acoustic_path_verified"
        } else {
            "acoustic_path_unverified"
        },
        "Physical microphone correlation checks the joint acoustic path; absence alone cannot isolate speaker, microphone, routing or room conditions",
    );
    r.measure("correlation", score, "normalized");
    r.measure("left_correlation", left, "normalized");
    r.measure("right_correlation", right, "normalized");
    r.measure(
        "left_timing_adjustment",
        (left_lag as i64 - lag as i64) as f64 * 16.0 / RATE as f64 * 1000.0,
        "ms",
    );
    r.measure(
        "right_timing_adjustment",
        (right_lag as i64 - (lag + half) as i64) as f64 * 16.0 / RATE as f64 * 1000.0,
        "ms",
    );
    r.measure("signal_above_baseline", snr, "dB");
    r.measure("clipping", clip * 100.0, "%");
    r.measure("microphone_id", &mic.id, "identifier");
    r.measure("playback_signal_peak", TONE_LEVEL, "normalized");
    r.evidence.push(
        "Default thresholds are provisional until calibrated on the physical Mac validation matrix"
            .into(),
    );
    r
}
fn audio_jack(req: &WorkerRequest, d: &Device) -> TestResult {
    let card = d.attributes.get("card").map(String::as_str).unwrap_or("");
    let o = match run("amixer", &["-c", card, "contents"], None, 3, 128 * 1024) {
        Ok(o) => o,
        Err(e) => return problem(req, e),
    };
    let current = String::from_utf8_lossy(&o.stdout).into_owned();
    let before = req
        .baseline
        .devices
        .iter()
        .find(|b| b.id == d.id)
        .and_then(|b| b.attributes.get("jack_controls"));
    let jack_sections: Vec<_> = current
        .split("numid=")
        .filter(|s| s.to_lowercase().contains("jack") && s.contains("values=on"))
        .collect();
    let changed = before.is_some_and(|b| b != &current) && !jack_sections.is_empty();
    let output = req.inventory.devices.iter().find(|s| {
        s.kind == Kind::Speaker && s.attributes.get("card").map(String::as_str) == Some(card)
    });
    if let Some(output) = output
        && let Err(e) = playback(output, 17)
    {
        return problem(req, e);
    }
    let mut r = TestResult::new(
        &req.test,
        if changed && output.is_some() {
            Outcome::Passed
        } else {
            Outcome::Inconclusive
        },
        if changed {
            "jack_insertion_observed"
        } else {
            "jack_detection_unavailable"
        },
        "Headset playback attempted; operator must confirm physical routing and supported input functions",
    );
    r.evidence
        .extend(jack_sections.iter().map(|s| s.chars().take(1000).collect()));
    r
}

#[cfg(test)]
mod audio_level_tests {
    use super::*;

    #[test]
    fn test_tones_are_louder_but_bounded_without_mixer_changes() {
        let (stereo, reference) = tone_sequence(17);
        let peak = reference
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0, f64::max);
        assert!(peak > 0.29 && peak <= TONE_LEVEL);
        assert!(stereo.iter().all(|sample| sample.unsigned_abs() <= 32767));
    }

    #[test]
    fn transient_microphone_replay_gain_is_bounded() {
        let (quiet, gain) = replay_pcm(&[0.02, -0.02, 0.0]);
        assert_eq!(gain, MAX_REPLAY_GAIN);
        assert_eq!(i16::from_le_bytes([quiet[0], quiet[1]]), 7864);
        let (loud, gain) = replay_pcm(&[1.0]);
        assert_eq!(gain, REPLAY_PEAK);
        assert!(i16::from_le_bytes([loud[0], loud[1]]) < 16384);
    }

    #[test]
    fn local_channel_match_tolerates_small_timing_shift() {
        let reference: Vec<f64> = (0..256)
            .map(|i| ((i * 73 + i * i * 19) % 101) as f64 / 50.0 - 1.0)
            .collect();
        let mut recording = vec![0.0; 400];
        recording[57..57 + reference.len()].copy_from_slice(&reference);
        let (score, lag) = correlation_near(&reference, &recording, 50, 10);
        assert!(score > 0.999);
        assert_eq!(lag, 57);
        let (score, _) = correlation_near(&reference, &recording, 50, 3);
        assert!(score < 0.5);
    }
}
