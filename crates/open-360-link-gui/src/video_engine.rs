use crate::state::{
    BackgroundEffectMode, CameraAppState, CompositionStyle, FilterMode, ISO_LIST,
    SHUTTER_DENOM_LIST,
};
use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use open_360_link_protocol::{TrackSpeed, UvcRect, VideoMode};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug)]
struct RgbFloat {
    r: f32,
    g: f32,
    b: f32,
}

impl RgbFloat {
    const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0_f32,
            g: g as f32 / 255.0_f32,
            b: b as f32 / 255.0_f32,
        }
    }

    fn to_color32(self) -> Color32 {
        Color32::from_rgb(
            (self.r.clamp(0.0_f32, 1.0_f32) * 255.0_f32).round() as u8,
            (self.g.clamp(0.0_f32, 1.0_f32) * 255.0_f32).round() as u8,
            (self.b.clamp(0.0_f32, 1.0_f32) * 255.0_f32).round() as u8,
        )
    }

    fn blend(self, other: Self, factor: f32) -> Self {
        let f = factor.clamp(0.0_f32, 1.0_f32);
        Self {
            r: self.r * (1.0_f32 - f) + other.r * f,
            g: self.g * (1.0_f32 - f) + other.g * f,
            b: self.b * (1.0_f32 - f) + other.b * f,
        }
    }
}

/// Real-time live camera capture engine supporting hardware webcams via `ffmpeg`
/// and a high-performance 60fps virtual studio simulator.
pub struct VideoPreviewEngine {
    available_devices: Vec<String>,
    active_device: Option<String>,
    capture_width: usize,
    capture_height: usize,
    capture_fps: u32,
    capture_stop: Arc<AtomicBool>,
    capture_child: Arc<Mutex<Option<Child>>>,
    capture_frame: Arc<Mutex<Option<(usize, usize, Vec<u8>)>>>,
    frame_seq: Arc<AtomicU64>,
    last_rendered_seq: u64,
    last_effects_key: (FilterMode, bool, u8),
    recording_stdin: Arc<Mutex<Option<(ChildStdin, usize, usize)>>>,
    recording_child: Option<Child>,
    recording_path: Option<PathBuf>,
    texture_handle: Option<TextureHandle>,
    latest_frame: Option<ColorImage>,
    start_time: Instant,
    last_frame_time: Instant,
    auto_pan: f32,
    auto_tilt: f32,
    auto_zoom: f32,
}

impl Default for VideoPreviewEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for VideoPreviewEngine {
    fn drop(&mut self) {
        let _ = self.stop_recording();
        self.stop_physical_capture();
    }
}

impl VideoPreviewEngine {
    /// Creates a new preview engine and discovers system capture devices.
    pub fn new() -> Self {
        let mut engine = Self {
            available_devices: Vec::new(),
            active_device: None,
            capture_width: 1920,
            capture_height: 1080,
            capture_fps: 30,
            capture_stop: Arc::new(AtomicBool::new(false)),
            capture_child: Arc::new(Mutex::new(None)),
            capture_frame: Arc::new(Mutex::new(None)),
            frame_seq: Arc::new(AtomicU64::new(0)),
            last_rendered_seq: 0,
            last_effects_key: (FilterMode::None, false, 0),
            recording_stdin: Arc::new(Mutex::new(None)),
            recording_child: None,
            recording_path: None,
            texture_handle: None,
            latest_frame: None,
            start_time: Instant::now(),
            last_frame_time: Instant::now(),
            auto_pan: 0.0_f32,
            auto_tilt: 0.0_f32,
            auto_zoom: 1.0_f32,
        };
        engine.refresh_capture_devices();
        engine
    }

    /// Updates the target capture resolution and frame rate, restarting the physical capture stream if active.
    pub fn set_capture_format(&mut self, width: usize, height: usize, fps: u32) {
        let w = width.max(320);
        let h = height.max(240);
        let f = fps.clamp(15, 60);
        if self.capture_width == w && self.capture_height == h && self.capture_fps == f {
            return;
        }
        self.capture_width = w;
        self.capture_height = h;
        self.capture_fps = f;
        if let Some(dev) = self.active_device.clone() {
            self.start_physical_capture(&dev);
        }
    }

    /// Returns the currently configured capture dimensions `(width, height)`.
    pub fn capture_dimensions(&self) -> (usize, usize) {
        (self.capture_width, self.capture_height)
    }

    /// Returns the currently configured capture frame rate (`fps`).
    pub fn capture_fps(&self) -> u32 {
        self.capture_fps
    }

    /// List of discovered hardware capture devices.
    pub fn available_capture_devices(&self) -> &[String] {
        &self.available_devices
    }

    /// Currently active hardware capture device name, or None if in virtual studio mode.
    pub fn active_capture_device(&self) -> Option<&str> {
        self.active_device.as_deref()
    }

    /// Checks if physical camera capture is currently active.
    pub fn is_using_physical_capture(&self) -> bool {
        self.active_device.is_some()
    }

    /// Returns true once the background physical capture thread has decoded at least one live frame.
    pub fn has_live_physical_frame(&self) -> bool {
        self.active_device.is_some()
            && self
                .capture_frame
                .lock()
                .ok()
                .map(|g| g.is_some())
                .unwrap_or(false)
    }

    /// Refreshes the list of available physical capture devices using platform tools.
    pub fn refresh_capture_devices(&mut self) {
        self.available_devices.clear();

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("ffmpeg")
                .args(["-hide_banner", "-f", "avfoundation", "-list_devices", "true", "-i", ""])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stderr);
                let mut in_video_section = false;
                for line in text.lines() {
                    if line.contains("AVFoundation video devices:") {
                        in_video_section = true;
                        continue;
                    }
                    if line.contains("AVFoundation audio devices:") {
                        break;
                    }
                    if in_video_section {
                        // Lines look like: [AVFoundation indev @ 0x...] [0] Insta360 Link
                        if let Some(pos) = line.find("] [") {
                            let sub = &line[pos + 2..];
                            if let Some(end) = sub.find(']') {
                                let idx_str = &sub[1..end];
                                let name = sub[end + 1..].trim();
                                if idx_str.chars().all(|c| c.is_ascii_digit()) {
                                    self.available_devices.push(format!("[{}] {}", idx_str, name));
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            for i in 0..16 {
                let dev_path = format!("/dev/video{}", i);
                if !Path::new(&dev_path).exists() {
                    continue;
                }
                let sys_dir = PathBuf::from(format!("/sys/class/video4linux/video{}", i));
                // Skip secondary metadata V4L2 nodes (`index != 0`)
                let idx = std::fs::read_to_string(sys_dir.join("index"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if !idx.is_empty() && idx != "0" {
                    continue;
                }
                let card_name = std::fs::read_to_string(sys_dir.join("name"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if card_name.is_empty() {
                    self.available_devices.push(dev_path);
                } else {
                    self.available_devices
                        .push(format!("[{}] {}", dev_path, card_name));
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Command::new("ffmpeg")
                .args(["-hide_banner", "-f", "dshow", "-list_devices", "true", "-i", "dummy"])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stderr);
                let mut in_video_section = false;
                for line in text.lines() {
                    if line.contains("DirectShow video devices") {
                        in_video_section = true;
                        continue;
                    }
                    if line.contains("DirectShow audio devices") {
                        break;
                    }
                    if in_video_section {
                        if let Some(start) = line.find('"') {
                            if let Some(end) = line[start + 1..].find('"') {
                                let name = &line[start + 1..start + 1 + end];
                                if !name.is_empty() {
                                    self.available_devices.push(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Starts streaming video from a physical webcam via background ffmpeg process.
    pub fn start_physical_capture(&mut self, device_spec: &str) {
        self.stop_physical_capture();

        self.active_device = Some(device_spec.to_string());
        self.capture_stop.store(false, Ordering::SeqCst);

        let device_arg = if device_spec.starts_with('[') {
            if let Some(end) = device_spec.find(']') {
                let bracketed = device_spec[1..end].trim();
                let name = device_spec[end + 1..].trim();
                if bracketed.starts_with("/dev/") {
                    bracketed.to_string()
                } else if !name.is_empty() {
                    name.to_string()
                } else {
                    bracketed.to_string()
                }
            } else {
                device_spec.to_string()
            }
        } else {
            device_spec.to_string()
        };

        let stop_flag = self.capture_stop.clone();
        let frame_buf = self.capture_frame.clone();
        let frame_seq = self.frame_seq.clone();
        let child_store = self.capture_child.clone();
        let rec_stdin = self.recording_stdin.clone();
        let out_w = self.capture_width;
        let out_h = self.capture_height;
        let fps = self.capture_fps;

        std::thread::spawn(move || {
            let scale_filter = format!("scale={out_w}:{out_h}");
            let frame_size = out_w * out_h * 3;
            let mut buffer = vec![0u8; frame_size];

            for attempt in 0..8 {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                if attempt > 0 {
                    std::thread::sleep( std::time::Duration::from_millis(400));
                    if stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                }
                let mut cmd = Command::new("ffmpeg");

                #[cfg(target_os = "macos")]
                {
                    // Insta360 Link AVFoundation natively supports 3840x2160, 1920x1080, 1280x720, 1080x1920
                    // in '420v' (nv12) and '2vuy' (uyvy422). Using nv12 matches both the official
                    // Insta360 Link Controller (`OpenDevice` at 0x10008770c) and Chrome/Google Meet.
                    let hw_fps = if attempt < 6 {
                        fps.to_string()
                    } else {
                        fps.min(30).to_string()
                    };
                    let hw_size = match (out_w, out_h) {
                        (3840, 2160) => "3840x2160",
                        (1280, 720) => "1280x720",
                        (1080, 1920) if attempt < 6 => "1080x1920",
                        _ => "1920x1080",
                    };
                    cmd.args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        "avfoundation",
                        "-pixel_format",
                        "nv12",
                    ]);
                    if attempt < 7 {
                        cmd.args(["-framerate", &hw_fps, "-video_size", hw_size]);
                    } else {
                        cmd.args(["-framerate", "30", "-video_size", "1280x720"]);
                    }
                    cmd.args([
                        "-i",
                        &device_arg,
                        "-fps_mode",
                        "passthrough",
                        "-vf",
                        &scale_filter,
                        "-pix_fmt",
                        "rgb24",
                        "-vcodec",
                        "rawvideo",
                        "-f",
                        "rawvideo",
                        "-",
                    ]);
                }

                #[cfg(target_os = "linux")]
                {
                    let hw_fps = if attempt < 6 {
                        fps.to_string()
                    } else {
                        fps.min(30).to_string()
                    };
                    let hw_size = match (out_w, out_h) {
                        (3840, 2160) => "3840x2160",
                        (1280, 720) => "1280x720",
                        (1080, 1920) if attempt < 6 => "1080x1920",
                        _ => "1920x1080",
                    };
                    cmd.args(["-hide_banner", "-loglevel", "error", "-f", "v4l2"]);
                    if attempt < 7 {
                        cmd.args([
                            "-input_format",
                            "mjpeg",
                            "-framerate",
                            &hw_fps,
                            "-video_size",
                            hw_size,
                        ]);
                    } else {
                        cmd.args(["-framerate", "30", "-video_size", "1280x720"]);
                    }
                    cmd.args([
                        "-i",
                        &device_arg,
                        "-fps_mode",
                        "passthrough",
                        "-vf",
                        &scale_filter,
                        "-pix_fmt",
                        "rgb24",
                        "-vcodec",
                        "rawvideo",
                        "-f",
                        "rawvideo",
                        "-",
                    ]);
                }

                #[cfg(target_os = "windows")]
                {
                    let dshow_input = format!("video={}", device_arg);
                    let fps_str = fps.to_string();
                    let out_size_arg = format!("{out_w}x{out_h}");
                    cmd.args(["-hide_banner", "-loglevel", "error", "-f", "dshow"]);
                    if attempt == 0 {
                        cmd.args(["-framerate", &fps_str, "-video_size", &out_size_arg]);
                    }
                    cmd.args([
                        "-i",
                        &dshow_input,
                        "-fps_mode",
                        "passthrough",
                        "-vf",
                        &scale_filter,
                        "-pix_fmt",
                        "rgb24",
                        "-vcodec",
                        "rawvideo",
                        "-f",
                        "rawvideo",
                        "-",
                    ]);
                }

                cmd.stdout(Stdio::piped()).stderr(Stdio::null());

                let mut child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let mut stdout = match child.stdout.take() {
                    Some(s) => s,
                    None => {
                        let _ = child.kill();
                        continue;
                    }
                };

                if let Ok(mut lock) = child_store.lock() {
                    *lock = Some(child);
                }

                let mut got_any_frame = false;
                while !stop_flag.load(Ordering::Relaxed) {
                    if stdout.read_exact(&mut buffer).is_ok() {
                        got_any_frame = true;
                        if let Ok(mut rec_lock) = rec_stdin.lock() {
                            if let Some((ref mut stdin, rw, rh)) = *rec_lock {
                                if rw == out_w && rh == out_h {
                                    let _ = stdin.write_all(&buffer);
                                }
                            }
                        }
                        if let Ok(mut lock) = frame_buf.lock() {
                            *lock = Some((out_w, out_h, buffer.clone()));
                            frame_seq.fetch_add(1, Ordering::Release);
                        }
                    } else {
                        break;
                    }
                }

                if got_any_frame {
                    break;
                }
            }
        });
    }

    /// Stops background ffmpeg capture and returns to the virtual studio renderer.
    pub fn stop_physical_capture(&mut self) {
        self.capture_stop.store(true, Ordering::SeqCst);
        if let Ok(mut lock) = self.capture_child.lock() {
            if let Some(mut child) = lock.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if let Ok(mut lock) = self.capture_frame.lock() {
            *lock = None;
        }
        self.active_device = None;
    }

    /// Renders a full frame (either physical or virtual studio) at the selected capture resolution,
    /// processes real-time filters/effects, uploads to egui GPU texture, and returns the texture handle.
    pub fn render_frame(&mut self, ctx: &Context, state: &mut CameraAppState) -> TextureHandle {
        let now = Instant::now();
        let dt = (now - self.last_frame_time).as_secs_f32().clamp(0.001_f32, 0.1_f32);
        self.last_frame_time = now;
        let elapsed = self.start_time.elapsed().as_secs_f32();

        let cur_seq = self.frame_seq.load(Ordering::Acquire);
        let effects_key = (state.filter_mode, state.makeup_enabled, state.makeup_intensity);

        // Fast-path: if physical capture is active and neither the frame nor host effects changed,
        // return the existing GPU texture handle without re-converting or re-uploading.
        if self.is_using_physical_capture()
            && cur_seq > 0
            && cur_seq == self.last_rendered_seq
            && effects_key == self.last_effects_key
        {
            if let Some(ref th) = self.texture_handle {
                return th.clone();
            }
        }

        let physical_opt = self.capture_frame.lock().ok().and_then(|guard| guard.clone());
        let is_physical_active = self.is_using_physical_capture() && physical_opt.is_some();
        let is_insta360_hw_stream = self
            .active_device
            .as_deref()
            .map(|d| d.to_lowercase().contains("insta360"))
            .unwrap_or(false);

        let color_image = if is_physical_active {
            let (src_w, src_h, raw_rgb) = physical_opt.unwrap();
            self.last_rendered_seq = cur_seq;
            self.last_effects_key = effects_key;

            if is_insta360_hw_stream && !state.makeup_enabled && state.filter_mode == FilterMode::None {
                // Zero-downscale native resolution fast path (360p, 720p, 1080p, 4K)
                ColorImage::from_rgb([src_w, src_h], &raw_rgb)
            } else {
                let (out_w, out_h) = if src_w > 1920 {
                    (1920, (src_h * 1920) / src_w.max(1))
                } else {
                    (src_w, src_h)
                };
                let mut frame_pixels = self.crop_physical_frame(
                    &raw_rgb,
                    src_w,
                    src_h,
                    out_w,
                    out_h,
                    state,
                    is_insta360_hw_stream,
                );
                if is_insta360_hw_stream {
                    self.apply_host_effects_only(&mut frame_pixels, state);
                } else {
                    self.apply_image_pipeline(&mut frame_pixels, out_w, out_h, state);
                }
                let mut pixels = Vec::with_capacity(out_w * out_h);
                for p in frame_pixels {
                    pixels.push(p.to_color32());
                }
                ColorImage {
                    size: [out_w, out_h],
                    pixels,
                }
            }
        } else {
            let is_vert = state.format_preset.is_vertical();
            let (out_w, out_h) = if state.low_resolution_mode {
                if is_vert {
                    (270, 480)
                } else {
                    (480, 270)
                }
            } else if is_vert {
                (360, 640)
            } else {
                (640, 360)
            };
            let mut frame_pixels = self.render_virtual_studio_frame(out_w, out_h, elapsed, dt, state);
            self.apply_image_pipeline(&mut frame_pixels, out_w, out_h, state);
            let mut pixels = Vec::with_capacity(out_w * out_h);
            for p in frame_pixels {
                pixels.push(p.to_color32());
            }
            if let Ok(mut rec_lock) = self.recording_stdin.lock() {
                if let Some((ref mut stdin, rw, rh)) = *rec_lock {
                    let mut rgb24 = Vec::with_capacity(rw * rh * 3);
                    for y in 0..rh {
                        let sy = (y * out_h) / rh.max(1);
                        for x in 0..rw {
                            let sx = (x * out_w) / rw.max(1);
                            let c = pixels[sy * out_w + sx];
                            rgb24.push(c.r());
                            rgb24.push(c.g());
                            rgb24.push(c.b());
                        }
                    }
                    let _ = stdin.write_all(&rgb24);
                }
            }
            ColorImage {
                size: [out_w, out_h],
                pixels,
            }
        };

        // Upload to TextureHandle
        let texture_options = TextureOptions::LINEAR;
        let handle = if let Some(ref mut th) = self.texture_handle {
            th.set(color_image.clone(), texture_options);
            th.clone()
        } else {
            let th = ctx.load_texture("camera_live_preview", color_image.clone(), texture_options);
            self.texture_handle = Some(th.clone());
            th
        };

        self.latest_frame = Some(color_image);
        handle
    }

    /// Resizes and crops a physical camera RGB24 buffer based on PTZ state.
    fn crop_physical_frame(
        &self,
        raw_rgb: &[u8],
        src_w: usize,
        src_h: usize,
        out_w: usize,
        out_h: usize,
        state: &CameraAppState,
        is_insta360_hw_stream: bool,
    ) -> Vec<RgbFloat> {
        let (zoom, center_x, center_y) = if is_insta360_hw_stream {
            // Physical Insta360 camera performs gimbal pan/tilt and zoom in hardware
            (1.0_f32, 0.5_f32, 0.5_f32)
        } else {
            (
                state.zoom_x.max(1.0_f32),
                state.host_ptz_x - (state.pan_deg / 120.0_f32),
                state.host_ptz_y - (state.tilt_deg / 90.0_f32),
            )
        };

        let mut out = Vec::with_capacity(out_w * out_h);
        for y in 0..out_h {
            let vy = (y as f32 + 0.5_f32) / out_h as f32;
            for x in 0..out_w {
                let ux = (x as f32 + 0.5_f32) / out_w as f32;

                let src_u = center_x + (ux - 0.5_f32) / zoom;
                let src_v = center_y + (vy - 0.5_f32) / zoom;

                let px = (src_u.clamp(0.0_f32, 1.0_f32) * (src_w - 1) as f32).round() as usize;
                let py = (src_v.clamp(0.0_f32, 1.0_f32) * (src_h - 1) as f32).round() as usize;
                let idx = (py * src_w + px) * 3;

                if idx + 2 < raw_rgb.len() {
                    out.push(RgbFloat::from_u8(
                        raw_rgb[idx],
                        raw_rgb[idx + 1],
                        raw_rgb[idx + 2],
                    ));
                } else {
                    out.push(RgbFloat::new(0.0_f32, 0.0_f32, 0.0_f32));
                }
            }
        }
        out
    }

    /// Applies host-side visual effects (Color Grading Filter LUTs and Makeup) when streaming
    /// from a physical Insta360 camera that already performs PTZ and UVC PU/CT adjustments in hardware.
    fn apply_host_effects_only(&self, pixels: &mut [RgbFloat], state: &CameraAppState) {
        let makeup_weight = if state.makeup_enabled {
            state.makeup_intensity as f32 / 100.0_f32
        } else {
            0.0_f32
        };
        if makeup_weight == 0.0_f32 && state.filter_mode == FilterMode::None {
            return;
        }
        for p in pixels.iter_mut() {
            if makeup_weight > 0.0_f32
                && p.r > p.g
                && p.g > p.b
                && (p.r - p.g) > 0.04_f32
                && (p.r - p.g) < 0.32_f32
                && p.r > 0.32_f32
            {
                p.r += 0.05_f32 * makeup_weight;
                p.g += 0.035_f32 * makeup_weight;
                p.b += 0.035_f32 * makeup_weight;
            }
            apply_filter_lut(p, state.filter_mode);
            p.r = p.r.clamp(0.0_f32, 1.0_f32);
            p.g = p.g.clamp(0.0_f32, 1.0_f32);
            p.b = p.b.clamp(0.0_f32, 1.0_f32);
        }
    }

    /// Renders the complete virtual studio scene in world coordinates.
    fn render_virtual_studio_frame(
        &mut self,
        out_w: usize,
        out_h: usize,
        elapsed: f32,
        dt: f32,
        state: &mut CameraAppState,
    ) -> Vec<RgbFloat> {
        // Presenter breathing & swaying motion
        let sway_x = (elapsed * 1.3_f32).sin() * 0.007_f32 + (elapsed * 0.7_f32).cos() * 0.004_f32;
        let breath_y = (elapsed * 2.2_f32).sin() * 0.005_f32;
        let pres_x = 0.48_f32 + sway_x;
        let pres_y = 0.52_f32 + breath_y;

        // Determine framing target based on video mode
        let mut target_pan = state.pan_deg;
        let mut target_tilt = state.tilt_deg;
        let mut target_zoom = state.zoom_x;

        match state.video_mode {
            VideoMode::WhiteBoard | VideoMode::SmartWhiteBoard => {
                // Focus smoothly onto the whiteboard
                target_pan = 24.0_f32;
                target_tilt = 4.0_f32;
                target_zoom = 2.2_f32;
            }
            VideoMode::DeskView | VideoMode::OverheadShot => {
                // Tilt down onto desk
                target_tilt = -38.0_f32;
                target_zoom = 1.35_f32;
            }
            VideoMode::AutoTracking | VideoMode::AutoFraming => {
                let (track_y_offset, track_zoom) = match state.composition_style {
                    CompositionStyle::Head => (-0.14_f32, 2.8_f32),
                    CompositionStyle::HalfBody => (-0.04_f32, 1.8_f32),
                    CompositionStyle::WholeBody => (0.06_f32, 1.15_f32),
                    CompositionStyle::Custom => (0.0_f32, 1.5_f32),
                };

                target_pan = (pres_x - 0.5_f32) * 80.0_f32;
                target_tilt = -(pres_y + track_y_offset - 0.5_f32) * 60.0_f32;
                target_zoom = track_zoom;
            }
            VideoMode::Normal => {}
        }

        // Smooth camera movement towards target
        let alpha = match state.track_speed {
            TrackSpeed::Slow => (dt * 3.0_f32).clamp(0.02_f32, 0.2_f32),
            TrackSpeed::Normal => (dt * 7.0_f32).clamp(0.05_f32, 0.4_f32),
            TrackSpeed::Fast => (dt * 15.0_f32).clamp(0.1_f32, 0.8_f32),
        };

        self.auto_pan += (target_pan - self.auto_pan) * alpha;
        self.auto_tilt += (target_tilt - self.auto_tilt) * alpha;
        self.auto_zoom += (target_zoom - self.auto_zoom) * alpha;

        let active_pan = if state.video_mode == VideoMode::Normal {
            state.pan_deg
        } else {
            self.auto_pan
        };
        let active_tilt = if state.video_mode == VideoMode::Normal {
            state.tilt_deg
        } else {
            self.auto_tilt
        };
        let active_zoom = if state.video_mode == VideoMode::Normal {
            state.zoom_x
        } else {
            self.auto_zoom
        };

        // Center calculation
        let center_u = 0.5_f32 - (active_pan / 120.0_f32)
            + (state.bias_x as f32 / 1000.0_f32)
            + (state.host_ptz_x - 0.5_f32);
        let center_v = 0.5_f32 - (active_tilt / 90.0_f32)
            + (state.bias_y as f32 / 1000.0_f32)
            + (state.host_ptz_y - 0.5_f32);

        // Update live presenter tracked bounding box in 0..1000 coordinates
        let pres_min_x = pres_x - 0.13_f32;
        let pres_max_x = pres_x + 0.13_f32;
        let pres_min_y = pres_y - 0.22_f32;
        let pres_max_y = pres_y + 0.26_f32;

        let scr_min_u = (pres_min_x - center_u) * active_zoom + 0.5_f32;
        let scr_max_u = (pres_max_x - center_u) * active_zoom + 0.5_f32;
        let scr_min_v = (pres_min_y - center_v) * active_zoom + 0.5_f32;
        let scr_max_v = (pres_max_y - center_v) * active_zoom + 0.5_f32;

        let uvc_x = ((scr_min_u.min(scr_max_u) * 1000.0_f32).clamp(0.0_f32, 1000.0_f32)) as i16;
        let uvc_y = ((scr_min_v.min(scr_max_v) * 1000.0_f32).clamp(0.0_f32, 1000.0_f32)) as i16;
        let uvc_w = (((scr_max_u - scr_min_u).abs() * 1000.0_f32).clamp(20.0_f32, 1000.0_f32)) as i16;
        let uvc_h = (((scr_max_v - scr_min_v).abs() * 1000.0_f32).clamp(20.0_f32, 1000.0_f32)) as i16;

        let live_target = UvcRect {
            x: uvc_x,
            y: uvc_y,
            width: uvc_w,
            height: uvc_h,
            track_id: 0,
        };
        if state.tracked_targets.is_empty() {
            state.tracked_targets.push(live_target);
        } else {
            state.tracked_targets[0] = live_target;
        }

        // Render base pixels
        let mut pixels = Vec::with_capacity(out_w * out_h);
        let mut presenter_mask = vec![false; out_w * out_h];

        let deskview_active = state.video_mode == VideoMode::DeskView
            || state.video_mode == VideoMode::OverheadShot;
        let pitch_rad = (state.deskview_pitch as f32 / 10.0_f32).abs().to_radians();

        for y in 0..out_h {
            let v_screen = (y as f32 + 0.5_f32) / out_h as f32;
            for x in 0..out_w {
                let u_screen = (x as f32 + 0.5_f32) / out_w as f32;

                // DeskView perspective correction
                let (u_corr, v_corr) = if deskview_active {
                    let desk_y = (v_screen - 0.55_f32).max(0.0_f32) / 0.45_f32;
                    let keystone = 1.0_f32 + (1.0_f32 - desk_y) * pitch_rad.sin() * 0.9_f32;
                    (
                        (u_screen - 0.5_f32) * keystone + 0.5_f32,
                        v_screen + (1.0_f32 - desk_y) * 0.12_f32,
                    )
                } else {
                    (u_screen, v_screen)
                };

                // World coordinate mapping
                let wu = center_u + (u_corr - 0.5_f32) / active_zoom;
                let wv = center_v + (v_corr - 0.5_f32) / active_zoom;

                // Test presenter hit
                if let Some(pres_color) = sample_presenter(wu, wv, pres_x, pres_y, elapsed) {
                    pixels.push(pres_color);
                    presenter_mask[y * out_w + x] = true;
                } else {
                    let bg_color = sample_studio_background(wu, wv);
                    pixels.push(bg_color);
                }
            }
        }

        // Apply background effects outside the presenter silhouette
        apply_background_effects(
            &mut pixels,
            &presenter_mask,
            out_w,
            out_h,
            state.background_mode,
            state.blur_intensity,
        );

        pixels
    }

    /// Full image and color grading pipeline executed on every frame.
    fn apply_image_pipeline(
        &self,
        pixels: &mut [RgbFloat],
        width: usize,
        height: usize,
        state: &CameraAppState,
    ) {
        // 1. Spatial transforms: Roll, Flips, Forced Vertical Lock
        apply_spatial_transforms(pixels, width, height, state);

        // 2. Exposure & tone curve
        let exposure_gain = if state.auto_exposure {
            2.0_f32.powf(state.ev_compensation * 0.5_f32)
        } else {
            let iso = ISO_LIST.get(state.iso_index).copied().unwrap_or(200) as f32;
            let shutter = SHUTTER_DENOM_LIST.get(state.shutter_index).copied().unwrap_or(100) as f32;
            (iso / 200.0_f32) * (100.0_f32 / shutter)
        };

        // Precompute 256-entry spline lookup table for manual exposure tone curve
        let mut spline_lut = [0.0_f32; 256];
        let points = if state.spline_points.len() >= 2 {
            &state.spline_points
        } else {
            &[(0.0_f32, 0.0_f32), (1.0_f32, 1.0_f32)][..]
        };
        for (i, entry) in spline_lut.iter_mut().enumerate() {
            let x = i as f32 / 255.0_f32;
            let mut y = x;
            for w in points.windows(2) {
                let (x0, y0) = w[0];
                let (x1, y1) = w[1];
                if x >= x0 && x <= x1 {
                    let span = (x1 - x0).max(1e-5_f32);
                    let t = ((x - x0) / span).clamp(0.0_f32, 1.0_f32);
                    y = y0 + t * (y1 - y0);
                    break;
                }
            }
            *entry = y.clamp(0.0_f32, 1.0_f32);
        }

        // 3. White balance shifts
        let (wb_r, wb_g, wb_b) = if !state.auto_white_balance {
            let shift = (state.wb_kelvin as f32 - 5000.0_f32) / 5000.0_f32;
            (1.0_f32 + shift * 0.35_f32, 1.0_f32, 1.0_f32 - shift * 0.35_f32)
        } else {
            (1.0_f32, 1.0_f32, 1.0_f32)
        };

        // 4. Brightness, Contrast, Saturation constants
        let bright_offset = (state.brightness as f32 - 50.0_f32) / 50.0_f32 * 0.20_f32;
        let contrast_mult = (state.contrast as f32 / 50.0_f32).powf(1.15_f32);
        let sat_mult = state.saturation as f32 / 50.0_f32;
        let makeup_weight = if state.makeup_enabled {
            state.makeup_intensity as f32 / 100.0_f32
        } else {
            0.0_f32
        };

        for p in pixels.iter_mut() {
            // Apply exposure
            p.r *= exposure_gain;
            p.g *= exposure_gain;
            p.b *= exposure_gain;

            // Apply manual spline tone curve if not in auto-exposure
            if !state.auto_exposure {
                let r_idx = (p.r.clamp(0.0_f32, 1.0_f32) * 255.0_f32).round() as usize;
                let g_idx = (p.g.clamp(0.0_f32, 1.0_f32) * 255.0_f32).round() as usize;
                let b_idx = (p.b.clamp(0.0_f32, 1.0_f32) * 255.0_f32).round() as usize;
                p.r = spline_lut[r_idx];
                p.g = spline_lut[g_idx];
                p.b = spline_lut[b_idx];
            }

            // HDR shadow lift and highlight compression
            if state.hdr_enabled {
                let luma = 0.299_f32 * p.r + 0.587_f32 * p.g + 0.114_f32 * p.b;
                if luma < 0.5_f32 {
                    let lift = (0.5_f32 - luma) * 0.35_f32;
                    p.r += lift;
                    p.g += lift;
                    p.b += lift;
                } else {
                    let comp = (luma - 0.5_f32) * 0.25_f32;
                    p.r -= comp;
                    p.g -= comp;
                    p.b -= comp;
                }
            }

            // White balance
            p.r *= wb_r;
            p.g *= wb_g;
            p.b *= wb_b;

            // Brightness & Contrast
            p.r = (p.r - 0.5_f32) * contrast_mult + 0.5_f32 + bright_offset;
            p.g = (p.g - 0.5_f32) * contrast_mult + 0.5_f32 + bright_offset;
            p.b = (p.b - 0.5_f32) * contrast_mult + 0.5_f32 + bright_offset;

            // Saturation
            let gray = 0.299_f32 * p.r + 0.587_f32 * p.g + 0.114_f32 * p.b;
            p.r = gray + (p.r - gray) * sat_mult;
            p.g = gray + (p.g - gray) * sat_mult;
            p.b = gray + (p.b - gray) * sat_mult;

            // Makeup skin brightening & smoothing
            if makeup_weight > 0.0_f32 {
                if p.r > p.g
                    && p.g > p.b
                    && (p.r - p.g) > 0.04_f32
                    && (p.r - p.g) < 0.32_f32
                    && p.r > 0.32_f32
                {
                    p.r += 0.05_f32 * makeup_weight;
                    p.g += 0.035_f32 * makeup_weight;
                    p.b += 0.035_f32 * makeup_weight;
                }
            }

            // Filter Color Grading LUTs
            apply_filter_lut(p, state.filter_mode);

            // Clamp channels
            p.r = p.r.clamp(0.0_f32, 1.0_f32);
            p.g = p.g.clamp(0.0_f32, 1.0_f32);
            p.b = p.b.clamp(0.0_f32, 1.0_f32);
        }

        // 5. Sharpness enhancement (50 is neutral)
        if state.sharpness != 50 {
            apply_sharpness(pixels, width, height, state.sharpness);
        }

        // 6. Manual focus defocus blur
        if !state.auto_focus && (state.manual_focus as i32 - 50).abs() > 8 {
            let dist = (state.manual_focus as i32 - 50).abs() - 8;
            let blur_r = ((dist as f32 / 42.0_f32) * 5.0_f32).round() as usize + 1;
            apply_defocus_blur(pixels, width, height, blur_r);
        }
    }

    /// Downscales the latest rendered frame to 96x54 thumbnail for UI preset tiles.
    pub fn capture_thumbnail(&self) -> ColorImage {
        let tw = 96;
        let th = 54;
        if let Some(ref src) = self.latest_frame {
            let sw = src.size[0];
            let sh = src.size[1];
            let mut pixels = Vec::with_capacity(tw * th);
            for ty in 0..th {
                let sy = (ty * sh) / th;
                for tx in 0..tw {
                    let sx = (tx * sw) / tw;
                    pixels.push(src.pixels[sy * sw + sx]);
                }
            }
            ColorImage {
                size: [tw, th],
                pixels,
            }
        } else {
            ColorImage::new([tw, th], Color32::from_rgb(22, 24, 30))
        }
    }

    /// Saves the current frame to disk as a full-resolution PNG screenshot at the selected resolution.
    pub fn save_screenshot(&self, save_dir: &Path, state: &CameraAppState) -> Result<PathBuf, String> {
        std::fs::create_dir_all(save_dir)
            .map_err(|e| format!("Failed to create screenshot directory: {e}"))?;

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = save_dir.join(format!("insta360_screenshot_{ts}.png"));

        let physical_opt = self.capture_frame.lock().ok().and_then(|g| g.clone());
        if self.is_using_physical_capture() {
            if let Some((src_w, src_h, raw_rgb)) = physical_opt {
                let is_insta360_hw_stream = self
                    .active_device
                    .as_deref()
                    .map(|d| d.to_lowercase().contains("insta360"))
                    .unwrap_or(false);
                let mut pixels = self.crop_physical_frame(
                    &raw_rgb,
                    src_w,
                    src_h,
                    src_w,
                    src_h,
                    state,
                    is_insta360_hw_stream,
                );
                if is_insta360_hw_stream {
                    self.apply_host_effects_only(&mut pixels, state);
                } else {
                    self.apply_image_pipeline(&mut pixels, src_w, src_h, state);
                }
                let mut rgba = Vec::with_capacity(src_w * src_h * 4);
                for p in pixels {
                    let c = p.to_color32();
                    rgba.push(c.r());
                    rgba.push(c.g());
                    rgba.push(c.b());
                    rgba.push(255);
                }
                image::save_buffer(
                    &path,
                    &rgba,
                    src_w as u32,
                    src_h as u32,
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|e| format!("Failed to encode screenshot PNG: {e}"))?;
                return Ok(path);
            }
        }

        let img = self
            .latest_frame
            .as_ref()
            .ok_or_else(|| "No video frame available to save".to_string())?;

        let out_w = self.capture_width.max(1);
        let out_h = self.capture_height.max(1);
        let sw = img.size[0].max(1);
        let sh = img.size[1].max(1);
        let mut rgba = Vec::with_capacity(out_w * out_h * 4);
        for y in 0..out_h {
            let sy = (y * sh) / out_h;
            for x in 0..out_w {
                let sx = (x * sw) / out_w;
                let p = img.pixels[sy * sw + sx];
                rgba.push(p.r());
                rgba.push(p.g());
                rgba.push(p.b());
                rgba.push(p.a());
            }
        }

        image::save_buffer(
            &path,
            &rgba,
            out_w as u32,
            out_h as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to encode screenshot PNG: {e}"))?;

        Ok(path)
    }

    /// Starts recording the live video feed at the selected capture resolution to an `.mp4` file.
    pub fn start_recording(&mut self, save_dir: &Path) -> Result<PathBuf, String> {
        let _ = self.stop_recording();
        std::fs::create_dir_all(save_dir)
            .map_err(|e| format!("Failed to create video directory: {e}"))?;

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = save_dir.join(format!("insta360_video_{ts}.mp4"));

        let w = self.capture_width;
        let h = self.capture_height;
        let fps = self.capture_fps.clamp(15, 60);
        let size_arg = format!("{w}x{h}");
        let fps_arg = fps.to_string();

        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            &size_arg,
            "-r",
            &fps_arg,
            "-i",
            "-",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg recorder: {e}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to open ffmpeg recorder stdin".to_string())?;

        if let Ok(mut lock) = self.recording_stdin.lock() {
            *lock = Some((stdin, w, h));
        }
        self.recording_child = Some(child);
        self.recording_path = Some(path.clone());
        Ok(path)
    }

    /// Stops an active video recording and finalizes the `.mp4` file.
    pub fn stop_recording(&mut self) -> Result<PathBuf, String> {
        if let Ok(mut lock) = self.recording_stdin.lock() {
            if let Some((mut stdin, _, _)) = lock.take() {
                let _ = stdin.flush();
            }
        }
        if let Some(mut child) = self.recording_child.take() {
            let _ = child.wait();
        }
        self.recording_path
            .take()
            .ok_or_else(|| "No active recording to stop".to_string())
    }
}

// ---------------------------------------------------------------------------
// Procedural Studio Scene & Object Samplers
// ---------------------------------------------------------------------------

fn sample_studio_background(wu: f32, wv: f32) -> RgbFloat {
    // 1. Desk surface at bottom
    if wv > 0.68_f32 {
        let desk_grain = ((wu * 180.0_f32).sin() * 0.03_f32).max(-0.02_f32);
        let desk_base = RgbFloat::new(0.24_f32 + desk_grain, 0.17_f32 + desk_grain, 0.13_f32);

        // Desk edge highlight
        if wv < 0.688_f32 {
            return RgbFloat::new(0.38_f32, 0.28_f32, 0.22_f32);
        }

        // Sleek desk mat
        if (0.22_f32..0.62_f32).contains(&wu) && (0.69_f32..0.88_f32).contains(&wv) {
            // Desk mat border stitch
            if wu < 0.225_f32 || wu > 0.615_f32 || wv < 0.695_f32 || wv > 0.875_f32 {
                return RgbFloat::new(0.20_f32, 0.22_f32, 0.26_f32);
            }
            return RgbFloat::new(0.14_f32, 0.15_f32, 0.18_f32);
        }

        // Open Laptop on desk
        if (0.28_f32..0.44_f32).contains(&wu) && (0.62_f32..0.76_f32).contains(&wv) {
            // Screen display
            if (0.29_f32..0.43_f32).contains(&wu) && (0.63_f32..0.72_f32).contains(&wv) {
                // Code editor lines
                let line_y = ((wv - 0.63_f32) * 120.0_f32).floor() as i32;
                if line_y % 2 == 0 {
                    let code_hash = ((wu * 80.0_f32).floor() as i32 + line_y * 7) % 5;
                    return match code_hash {
                        0 => RgbFloat::new(0.35_f32, 0.65_f32, 0.95_f32), // Keyword cyan/blue
                        1 => RgbFloat::new(0.92_f32, 0.65_f32, 0.25_f32), // Function amber
                        2 => RgbFloat::new(0.40_f32, 0.82_f32, 0.48_f32), // String green
                        3 => RgbFloat::new(0.85_f32, 0.45_f32, 0.75_f32), // Macro pink
                        _ => RgbFloat::new(0.70_f32, 0.75_f32, 0.82_f32), // Text light gray
                    };
                }
                return RgbFloat::new(0.10_f32, 0.12_f32, 0.16_f32);
            }
            // Laptop bezel and aluminum base
            return RgbFloat::new(0.28_f32, 0.30_f32, 0.34_f32);
        }

        // Ceramic coffee mug
        if (0.17_f32..0.21_f32).contains(&wu) && (0.70_f32..0.76_f32).contains(&wv) {
            let mug_cx = 0.19_f32;
            let mug_dx = (wu - mug_cx) / 0.02_f32;
            let mug_dy = (wv - 0.705_f32) / 0.015_f32;
            if mug_dx * mug_dx + mug_dy * mug_dy < 1.0_f32 {
                // Coffee surface
                return RgbFloat::new(0.16_f32, 0.09_f32, 0.06_f32);
            }
            return RgbFloat::new(0.78_f32, 0.34_f32, 0.22_f32); // Warm terracotta
        }

        // Spiral notebook & pen on the right
        if (0.70_f32..0.82_f32).contains(&wu) && (0.70_f32..0.78_f32).contains(&wv) {
            // Metallic pen resting beside notebook
            if (0.805_f32..0.815_f32).contains(&wu) && (0.71_f32..0.77_f32).contains(&wv) {
                return RgbFloat::new(0.75_f32, 0.78_f32, 0.84_f32);
            }
            // Notebook paper
            if ((wv - 0.70_f32) * 100.0_f32).fract() < 0.15_f32 {
                return RgbFloat::new(0.80_f32, 0.85_f32, 0.92_f32); // Light blue grid
            }
            return RgbFloat::new(0.95_f32, 0.94_f32, 0.90_f32); // Cream paper
        }

        return desk_base;
    }

    // 2. Wall-mounted physical Whiteboard on the right
    if (0.52_f32..0.88_f32).contains(&wu) && (0.18_f32..0.62_f32).contains(&wv) {
        // Outer brushed aluminum frame
        if wu < 0.528_f32 || wu > 0.872_f32 || wv < 0.188_f32 || wv > 0.612_f32 {
            return RgbFloat::new(0.75_f32, 0.78_f32, 0.82_f32);
        }

        // 4 Green corner markers for Smart Whiteboard
        let corner_size = 0.024_f32;
        let is_top_left = wu < 0.53_f32 + corner_size && wv < 0.19_f32 + corner_size;
        let is_top_right = wu > 0.87_f32 - corner_size && wv < 0.19_f32 + corner_size;
        let is_bot_left = wu < 0.53_f32 + corner_size && wv > 0.61_f32 - corner_size;
        let is_bot_right = wu > 0.87_f32 - corner_size && wv > 0.61_f32 - corner_size;

        if is_top_left || is_top_right || is_bot_left || is_bot_right {
            return RgbFloat::new(0.15_f32, 0.82_f32, 0.35_f32); // Vibrant tracking green
        }

        // Drawn architecture diagram on the whiteboard
        // Box 1: Client/Edge (Blue marker)
        if (0.57_f32..0.65_f32).contains(&wu) && (0.23_f32..0.29_f32).contains(&wv) {
            if wu < 0.574_f32 || wu > 0.646_f32 || wv < 0.234_f32 || wv > 0.286_f32 {
                return RgbFloat::new(0.12_f32, 0.45_f32, 0.85_f32);
            }
            return RgbFloat::new(0.92_f32, 0.95_f32, 0.99_f32);
        }

        // Connecting arrow 1 (Blue marker line)
        if (0.608_f32..0.612_f32).contains(&wu) && (0.29_f32..0.35_f32).contains(&wv) {
            return RgbFloat::new(0.12_f32, 0.45_f32, 0.85_f32);
        }

        // Box 2: AI Core / Processing (Purple marker)
        if (0.56_f32..0.66_f32).contains(&wu) && (0.35_f32..0.43_f32).contains(&wv) {
            if wu < 0.564_f32 || wu > 0.656_f32 || wv < 0.354_f32 || wv > 0.426_f32 {
                return RgbFloat::new(0.58_f32, 0.22_f32, 0.82_f32);
            }
            return RgbFloat::new(0.96_f32, 0.92_f32, 0.99_f32);
        }

        // Connecting arrow 2 (Purple marker line)
        if (0.608_f32..0.612_f32).contains(&wu) && (0.43_f32..0.49_f32).contains(&wv) {
            return RgbFloat::new(0.58_f32, 0.22_f32, 0.82_f32);
        }

        // Box 3: Cloud Database (Teal marker)
        if (0.57_f32..0.65_f32).contains(&wu) && (0.49_f32..0.55_f32).contains(&wv) {
            if wu < 0.574_f32 || wu > 0.646_f32 || wv < 0.494_f32 || wv > 0.546_f32 {
                return RgbFloat::new(0.08_f32, 0.58_f32, 0.62_f32);
            }
            return RgbFloat::new(0.91_f32, 0.98_f32, 0.98_f32);
        }

        // Chart bars on the right side of the whiteboard
        if (0.72_f32..0.82_f32).contains(&wu) && (0.42_f32..0.55_f32).contains(&wv) {
            // Chart baseline
            if wv > 0.545_f32 {
                return RgbFloat::new(0.22_f32, 0.24_f32, 0.28_f32);
            }
            // Bar 1: Blue
            if (0.73_f32..0.75_f32).contains(&wu) && wv > 0.48_f32 {
                return RgbFloat::new(0.18_f32, 0.52_f32, 0.92_f32);
            }
            // Bar 2: Orange
            if (0.76_f32..0.78_f32).contains(&wu) && wv > 0.45_f32 {
                return RgbFloat::new(0.94_f32, 0.50_f32, 0.16_f32);
            }
            // Bar 3: Green
            if (0.79_f32..0.81_f32).contains(&wu) && wv > 0.42_f32 {
                return RgbFloat::new(0.16_f32, 0.75_f32, 0.38_f32);
            }
        }

        // Clean white whiteboard surface
        return RgbFloat::new(0.96_f32, 0.97_f32, 0.98_f32);
    }

    // 3. Bookshelf on the left
    if (0.04_f32..0.22_f32).contains(&wu) && (0.18_f32..0.66_f32).contains(&wv) {
        // Shelf horizontal wooden boards
        let is_shelf = (0.33_f32..0.35_f32).contains(&wv)
            || (0.49_f32..0.51_f32).contains(&wv)
            || (0.64_f32..0.66_f32).contains(&wv);
        if is_shelf {
            return RgbFloat::new(0.32_f32, 0.20_f32, 0.12_f32);
        }

        // Vertical book spines
        let book_col = ((wu - 0.045_f32) / 0.024_f32).floor() as i32;
        let book_phase = ((wu - 0.045_f32) / 0.024_f32).fract();
        if book_phase < 0.88_f32 && book_col >= 0 && book_col < 7 {
            let shelf_idx = if wv < 0.34_f32 {
                0
            } else if wv < 0.50_f32 {
                1
            } else {
                2
            };
            let color_idx = (book_col * 3 + shelf_idx * 5) % 6;
            return match color_idx {
                0 => RgbFloat::new(0.12_f32, 0.22_f32, 0.42_f32), // Deep Navy
                1 => RgbFloat::new(0.62_f32, 0.16_f32, 0.20_f32), // Ruby
                2 => RgbFloat::new(0.18_f32, 0.42_f32, 0.25_f32), // Forest Green
                3 => RgbFloat::new(0.82_f32, 0.62_f32, 0.18_f32), // Gold
                4 => RgbFloat::new(0.72_f32, 0.35_f32, 0.18_f32), // Terracotta
                _ => RgbFloat::new(0.15_f32, 0.48_f32, 0.52_f32), // Teal
            };
        }
        return RgbFloat::new(0.16_f32, 0.11_f32, 0.08_f32); // Shelf shadow backboard
    }

    // 4. Acoustic wood-slat accent wall on the left
    if wu < 0.45_f32 {
        let slat_freq = wu * 24.0_f32;
        let slat_idx = slat_freq.floor() as i32;
        let slat_fract = slat_freq.fract();

        // Dark acoustic felt groove between slats
        if slat_fract < 0.22_f32 {
            return RgbFloat::new(0.10_f32, 0.08_f32, 0.07_f32);
        }

        // Wood slat tone
        let tone = if slat_idx % 2 == 0 {
            RgbFloat::new(0.62_f32, 0.40_f32, 0.24_f32) // Warm teak
        } else {
            RgbFloat::new(0.72_f32, 0.48_f32, 0.30_f32) // Honey oak
        };

        // Key light radial glow from upper left
        let light_dist = ((wu - 0.16_f32).powi(2) + (wv - 0.22_f32).powi(2)).sqrt();
        let glow = (1.0_f32 - (light_dist / 0.65_f32).min(1.0_f32)).powi(2) * 0.30_f32;
        return RgbFloat::new(
            (tone.r + glow * 1.1_f32).min(1.0_f32),
            (tone.g + glow * 0.9_f32).min(1.0_f32),
            (tone.b + glow * 0.6_f32).min(1.0_f32),
        );
    }

    // 5. Studio neutral accent wall
    let wall_light = 0.88_f32 - (wv * 0.12_f32);
    RgbFloat::new(wall_light * 0.95_f32, wall_light * 0.96_f32, wall_light * 0.98_f32)
}

/// Samples the animated presenter in the center. Returns Some(color) if hit.
fn sample_presenter(wu: f32, wv: f32, px: f32, py: f32, elapsed: f32) -> Option<RgbFloat> {
    // 1. Head & Face
    let head_cx = px;
    let head_cy = py - 0.14_f32;
    let hdx = (wu - head_cx) / 0.062_f32;
    let hdy = (wv - head_cy) / 0.078_f32;
    let head_dist = hdx * hdx + hdy * hdy;

    // Hair volume on top and sides
    let hair_cy = head_cy - 0.025_f32;
    let hair_dx = (wu - head_cx) / 0.068_f32;
    let hair_dy = (wv - hair_cy) / 0.072_f32;
    if hair_dx * hair_dx + hair_dy * hair_dy < 1.0_f32 && wv < head_cy - 0.015_f32 {
        return Some(RgbFloat::new(0.18_f32, 0.13_f32, 0.10_f32)); // Dark brown hair
    }

    if head_dist < 1.0_f32 {
        // Eyes
        let eye_y = head_cy - 0.012_f32;
        let left_eye_x = head_cx - 0.024_f32;
        let right_eye_x = head_cx + 0.024_f32;

        let is_left_eye = ((wu - left_eye_x) / 0.009_f32).powi(2)
            + ((wv - eye_y) / 0.007_f32).powi(2)
            < 1.0_f32;
        let is_right_eye = ((wu - right_eye_x) / 0.009_f32).powi(2)
            + ((wv - eye_y) / 0.007_f32).powi(2)
            < 1.0_f32;

        // Periodic natural blinking
        let is_blinking = (elapsed % 4.2_f32) < 0.14_f32;

        if is_left_eye || is_right_eye {
            if is_blinking {
                return Some(RgbFloat::new(0.35_f32, 0.22_f32, 0.18_f32));
            }
            let eye_cx = if is_left_eye { left_eye_x } else { right_eye_x };
            // Dark pupil & iris with specular reflection
            if (wu - eye_cx).abs() < 0.004_f32 && (wv - eye_y).abs() < 0.004_f32 {
                if wu > eye_cx && wv < eye_y {
                    return Some(RgbFloat::new(1.0_f32, 1.0_f32, 1.0_f32)); // Specular glint
                }
                return Some(RgbFloat::new(0.14_f32, 0.10_f32, 0.08_f32)); // Dark pupil
            }
            return Some(RgbFloat::new(0.96_f32, 0.96_f32, 0.98_f32)); // Sclera white
        }

        // Eyebrows
        let brow_y = head_cy - 0.025_f32;
        let left_brow = (wu - (head_cx - 0.024_f32)).abs() < 0.014_f32 && (wv - brow_y).abs() < 0.0025_f32;
        let right_brow = (wu - (head_cx + 0.024_f32)).abs() < 0.014_f32 && (wv - brow_y).abs() < 0.0025_f32;
        if left_brow || right_brow {
            return Some(RgbFloat::new(0.24_f32, 0.16_f32, 0.12_f32));
        }

        // Warm smile
        let mouth_y = head_cy + 0.040_f32;
        let mdx = (wu - head_cx) / 0.018_f32;
        let mdy = (wv - mouth_y) / 0.007_f32;
        if mdx * mdx + mdy * mdy < 1.0_f32 {
            return Some(RgbFloat::new(0.82_f32, 0.44_f32, 0.40_f32)); // Soft lip tone
        }

        // Face skin
        let skin_shadow = (hdy * 0.05_f32).max(0.0_f32);
        return Some(RgbFloat::new(
            0.94_f32 - skin_shadow,
            0.75_f32 - skin_shadow,
            0.64_f32 - skin_shadow,
        ));
    }

    // 2. Neck & Shirt Collar
    if (py - 0.07_f32..py).contains(&wv) && (wu - px).abs() < 0.034_f32 {
        // V-Neck / Shirt collar
        if wv > py - 0.04_f32 && (wu - px).abs() < (wv - (py - 0.04_f32)) * 0.8_f32 {
            return Some(RgbFloat::new(0.96_f32, 0.96_f32, 0.98_f32)); // Crisp white collar
        }
        return Some(RgbFloat::new(0.88_f32, 0.70_f32, 0.58_f32)); // Neck skin
    }

    // 3. Shoulders & Torso (Tailored Navy Blazer)
    if wv >= py - 0.02_f32 && wv < 0.78_f32 {
        let torso_prog = (wv - (py - 0.02_f32)) / 0.28_f32;
        let shoulder_w = 0.08_f32 + torso_prog * 0.14_f32;
        if (wu - px).abs() < shoulder_w {
            // White shirt in center
            if (wu - px).abs() < 0.018_f32 && wv < py + 0.12_f32 {
                return Some(RgbFloat::new(0.95_f32, 0.95_f32, 0.98_f32));
            }
            // Navy tailored blazer
            let blazer_highlight = (1.0_f32 - (wu - px).abs() / shoulder_w) * 0.06_f32;
            return Some(RgbFloat::new(
                0.14_f32 + blazer_highlight,
                0.18_f32 + blazer_highlight,
                0.28_f32 + blazer_highlight,
            ));
        }
    }

    None
}

/// Applies background blurring, bokeh, or custom replacement backdrops.
fn apply_background_effects(
    pixels: &mut [RgbFloat],
    presenter_mask: &[bool],
    width: usize,
    height: usize,
    mode: BackgroundEffectMode,
    blur_intensity: u8,
) {
    match mode {
        BackgroundEffectMode::None => {}
        BackgroundEffectMode::StandardBlur | BackgroundEffectMode::NaturalBokeh => {
            let radius = (1 + (blur_intensity as usize * 8) / 100).min(10);
            let original = pixels.to_vec();

            // Horizontal blur pass on background pixels
            let mut temp = original.clone();
            for y in 0..height {
                let row_offset = y * width;
                for x in 0..width {
                    if presenter_mask[row_offset + x] {
                        continue;
                    }
                    let mut sum_r = 0.0_f32;
                    let mut sum_g = 0.0_f32;
                    let mut sum_b = 0.0_f32;
                    let mut count = 0.0_f32;

                    let x_start = x.saturating_sub(radius);
                    let x_end = (x + radius).min(width - 1);
                    for kx in x_start..=x_end {
                        let sample = original[row_offset + kx];
                        sum_r += sample.r;
                        sum_g += sample.g;
                        sum_b += sample.b;
                        count += 1.0_f32;
                    }
                    temp[row_offset + x] = RgbFloat::new(sum_r / count, sum_g / count, sum_b / count);
                }
            }

            // Vertical blur pass on background pixels
            for y in 0..height {
                let row_offset = y * width;
                for x in 0..width {
                    if presenter_mask[row_offset + x] {
                        continue;
                    }
                    let mut sum_r = 0.0_f32;
                    let mut sum_g = 0.0_f32;
                    let mut sum_b = 0.0_f32;
                    let mut count = 0.0_f32;

                    let y_start = y.saturating_sub(radius);
                    let y_end = (y + radius).min(height - 1);
                    for ky in y_start..=y_end {
                        let sample = temp[ky * width + x];
                        sum_r += sample.r;
                        sum_g += sample.g;
                        sum_b += sample.b;
                        count += 1.0_f32;
                    }
                    pixels[row_offset + x] = RgbFloat::new(sum_r / count, sum_g / count, sum_b / count);
                }
            }

            // Add warm circular bokeh light circles if in NaturalBokeh mode
            if mode == BackgroundEffectMode::NaturalBokeh {
                let bokeh_spots = [
                    (0.14_f32, 0.22_f32, 0.042_f32, 0.55_f32),
                    (0.24_f32, 0.15_f32, 0.032_f32, 0.45_f32),
                    (0.36_f32, 0.28_f32, 0.050_f32, 0.60_f32),
                    (0.72_f32, 0.22_f32, 0.040_f32, 0.50_f32),
                    (0.84_f32, 0.32_f32, 0.052_f32, 0.65_f32),
                ];

                for (bx, by, brad, bint) in bokeh_spots {
                    let cx = (bx * width as f32) as isize;
                    let cy = (by * height as f32) as isize;
                    let r_pix = (brad * width as f32) as isize;

                    let y_min = (cy - r_pix).max(0) as usize;
                    let y_max = (cy + r_pix).min((height - 1) as isize) as usize;
                    let x_min = (cx - r_pix).max(0) as usize;
                    let x_max = (cx + r_pix).min((width - 1) as isize) as usize;

                    for y in y_min..=y_max {
                        let dy = (y as f32 - cy as f32) / r_pix as f32;
                        let row_offset = y * width;
                        for x in x_min..=x_max {
                            if presenter_mask[row_offset + x] {
                                continue;
                            }
                            let dx = (x as f32 - cx as f32) / r_pix as f32;
                            let dist2 = dx * dx + dy * dy;
                            if dist2 < 1.0_f32 {
                                // Bokeh ring edge boost
                                let ring = (1.0_f32 - (dist2.sqrt() - 0.75_f32).abs() * 3.0_f32).max(0.4_f32);
                                let bokeh_color = RgbFloat::new(1.0_f32, 0.86_f32, 0.60_f32);
                                pixels[row_offset + x] = pixels[row_offset + x]
                                    .blend(bokeh_color, bint * ring * 0.5_f32);
                            }
                        }
                    }
                }
            }
        }
        BackgroundEffectMode::CustomImage(idx) => {
            // Replace background pixels with distinct stylized backdrop
            for y in 0..height {
                let v = y as f32 / height as f32;
                let row_offset = y * width;
                for x in 0..width {
                    if !presenter_mask[row_offset + x] {
                        let u = x as f32 / width as f32;
                        pixels[row_offset + x] = sample_custom_backdrop(idx, u, v);
                    }
                }
            }
        }
    }
}

/// Samples distinct stylized custom virtual backdrops.
fn sample_custom_backdrop(idx: usize, u: f32, v: f32) -> RgbFloat {
    match idx % 3 {
        0 => {
            // Modern Office: floor-to-ceiling glass window overlooking city skyline
            if (0.32_f32..0.335_f32).contains(&u) || (0.66_f32..0.675_f32).contains(&u) {
                return RgbFloat::new(0.24_f32, 0.28_f32, 0.35_f32); // Window mullions
            }
            if v > 0.65_f32 {
                // Distant cityscape buildings
                let bldg_idx = (u * 18.0_f32).floor() as i32;
                let bldg_h = 0.50_f32 + ((bldg_idx * 17) % 5) as f32 * 0.04_f32;
                if v > bldg_h {
                    return RgbFloat::new(0.42_f32, 0.50_f32, 0.62_f32);
                }
            }
            // Daylight sky gradient
            let sky = 0.70_f32 + (1.0_f32 - v) * 0.25_f32;
            RgbFloat::new(sky * 0.82_f32, sky * 0.90_f32, sky)
        }
        1 => {
            // Minimalist Studio: slate gray acoustic panels with cyan/magenta LED ambient backlighting
            let panel_x = (u * 14.0_f32).fract();
            let base_panel = if panel_x < 0.12_f32 {
                RgbFloat::new(0.08_f32, 0.09_f32, 0.11_f32)
            } else {
                RgbFloat::new(0.18_f32, 0.20_f32, 0.24_f32)
            };
            // Dual-tone LED strip gradient
            let led_cyan = (1.0_f32 - u).powi(2) * 0.35_f32;
            let led_magenta = u.powi(2) * 0.35_f32;
            RgbFloat::new(
                (base_panel.r + led_magenta).min(1.0_f32),
                (base_panel.g + led_cyan * 0.8_f32).min(1.0_f32),
                (base_panel.b + led_cyan + led_magenta * 0.7_f32).min(1.0_f32),
            )
        }
        _ => {
            // Warm Bookshelf: rich library wall with mahogany shelves and warm lighting
            let shelf_y = (v * 5.0_f32).fract();
            if shelf_y < 0.12_f32 {
                return RgbFloat::new(0.28_f32, 0.16_f32, 0.10_f32); // Mahogany shelf
            }
            let book_x = (u * 22.0_f32).fract();
            if book_x < 0.85_f32 {
                let book_id = (u * 22.0_f32).floor() as i32 + (v * 5.0_f32).floor() as i32 * 7;
                return match book_id % 4 {
                    0 => RgbFloat::new(0.48_f32, 0.14_f32, 0.12_f32),
                    1 => RgbFloat::new(0.14_f32, 0.30_f32, 0.20_f32),
                    2 => RgbFloat::new(0.65_f32, 0.45_f32, 0.15_f32),
                    _ => RgbFloat::new(0.12_f32, 0.20_f32, 0.35_f32),
                };
            }
            RgbFloat::new(0.14_f32, 0.09_f32, 0.06_f32)
        }
    }
}

// ---------------------------------------------------------------------------
// Real-Time Image Pipeline Operations
// ---------------------------------------------------------------------------

fn apply_spatial_transforms(
    pixels: &mut [RgbFloat],
    width: usize,
    height: usize,
    state: &CameraAppState,
) {
    if !state.horizontal_mirror
        && !state.vertical_mirror
        && !state.forced_vertical_lock
        && state.roll_deg.abs() < 0.01_f32
    {
        return;
    }

    let src = pixels.to_vec();
    let roll_rad = state.roll_deg.to_radians();
    let cos_r = roll_rad.cos();
    let sin_r = roll_rad.sin();

    for y in 0..height {
        let v0 = (y as f32 + 0.5_f32) / height as f32;
        let row_offset = y * width;
        for x in 0..width {
            let u0 = (x as f32 + 0.5_f32) / width as f32;

            // Forced vertical lock 90-degree rotate
            let (mut u, mut v) = if state.forced_vertical_lock {
                (v0, 1.0_f32 - u0)
            } else {
                (u0, v0)
            };

            // Horizontal & vertical mirrors
            if state.horizontal_mirror {
                u = 1.0_f32 - u;
            }
            if state.vertical_mirror {
                v = 1.0_f32 - v;
            }

            // Roll rotation
            if state.roll_deg.abs() >= 0.01_f32 {
                let du = u - 0.5_f32;
                let dv = v - 0.5_f32;
                u = cos_r * du - sin_r * dv + 0.5_f32;
                v = sin_r * du + cos_r * dv + 0.5_f32;
            }

            let sx = (u.clamp(0.0_f32, 1.0_f32) * (width - 1) as f32).round() as usize;
            let sy = (v.clamp(0.0_f32, 1.0_f32) * (height - 1) as f32).round() as usize;

            pixels[row_offset + x] = src[sy * width + sx];
        }
    }
}

fn apply_filter_lut(p: &mut RgbFloat, mode: FilterMode) {
    match mode {
        FilterMode::None => {}
        FilterMode::Style1 => {
            // Neon cyan / magenta
            p.r = (p.r * 1.15_f32 + 0.05_f32).clamp(0.0_f32, 1.0_f32);
            p.g = (p.g * 0.85_f32).clamp(0.0_f32, 1.0_f32);
            p.b = (p.b * 1.25_f32 + 0.08_f32).clamp(0.0_f32, 1.0_f32);
        }
        FilterMode::Style2 => {
            // Vintage1 Sepia
            let r_s = 0.393_f32 * p.r + 0.769_f32 * p.g + 0.189_f32 * p.b;
            let g_s = 0.349_f32 * p.r + 0.686_f32 * p.g + 0.168_f32 * p.b;
            let b_s = 0.272_f32 * p.r + 0.534_f32 * p.g + 0.131_f32 * p.b;
            p.r = (p.r * 0.25_f32 + r_s * 0.75_f32).clamp(0.0_f32, 1.0_f32);
            p.g = (p.g * 0.25_f32 + g_s * 0.75_f32).clamp(0.0_f32, 1.0_f32);
            p.b = (p.b * 0.25_f32 + b_s * 0.75_f32).clamp(0.0_f32, 1.0_f32);
        }
        FilterMode::Fresh => {
            // Crisp clean clarity
            p.r = (p.r * 0.96_f32).clamp(0.0_f32, 1.0_f32);
            p.g = (p.g * 1.04_f32 + 0.01_f32).clamp(0.0_f32, 1.0_f32);
            p.b = (p.b * 1.08_f32 + 0.03_f32).clamp(0.0_f32, 1.0_f32);
        }
        FilterMode::Amber => {
            // Vintage2 golden amber
            p.r = (p.r * 1.22_f32 + 0.06_f32).clamp(0.0_f32, 1.0_f32);
            p.g = (p.g * 1.05_f32 + 0.02_f32).clamp(0.0_f32, 1.0_f32);
            p.b = (p.b * 0.78_f32).clamp(0.0_f32, 1.0_f32);
        }
        FilterMode::Film1 => {
            // Daylight film stock
            p.r = (p.r * 1.08_f32 + 0.03_f32).clamp(0.0_f32, 1.0_f32);
            p.g = (p.g * 1.02_f32 + 0.02_f32).clamp(0.0_f32, 1.0_f32);
            p.b = (p.b * 0.94_f32 + 0.04_f32).clamp(0.0_f32, 1.0_f32);
        }
        FilterMode::Film2 => {
            // Portrait warm skin tone
            p.r = (p.r * 1.12_f32 + 0.04_f32).clamp(0.0_f32, 1.0_f32);
            p.g = (p.g * 1.03_f32 + 0.01_f32).clamp(0.0_f32, 1.0_f32);
            p.b = (p.b * 0.96_f32).clamp(0.0_f32, 1.0_f32);
        }
        FilterMode::Cool => {
            // Chilled blue cool tone
            p.r = (p.r * 0.88_f32).clamp(0.0_f32, 1.0_f32);
            p.g = (p.g * 0.98_f32).clamp(0.0_f32, 1.0_f32);
            p.b = (p.b * 1.20_f32 + 0.06_f32).clamp(0.0_f32, 1.0_f32);
        }
        FilterMode::Warm => {
            // Cozy sunset warm tone
            p.r = (p.r * 1.18_f32 + 0.05_f32).clamp(0.0_f32, 1.0_f32);
            p.g = (p.g * 1.06_f32 + 0.02_f32).clamp(0.0_f32, 1.0_f32);
            p.b = (p.b * 0.88_f32).clamp(0.0_f32, 1.0_f32);
        }
    }
}

fn apply_sharpness(pixels: &mut [RgbFloat], width: usize, height: usize, sharpness: u16) {
    let diff = (sharpness as f32 - 50.0_f32) / 50.0_f32;
    let strength = diff * 0.55_f32;
    let original = pixels.to_vec();

    for y in 1..height - 1 {
        let row_offset = y * width;
        for x in 1..width - 1 {
            let center = original[row_offset + x];
            let top = original[(y - 1) * width + x];
            let bot = original[(y + 1) * width + x];
            let left = original[row_offset + (x - 1)];
            let right = original[row_offset + (x + 1)];

            let avg_r = (top.r + bot.r + left.r + right.r) * 0.25_f32;
            let avg_g = (top.g + bot.g + left.g + right.g) * 0.25_f32;
            let avg_b = (top.b + bot.b + left.b + right.b) * 0.25_f32;

            pixels[row_offset + x] = RgbFloat::new(
                (center.r + (center.r - avg_r) * strength).clamp(0.0_f32, 1.0_f32),
                (center.g + (center.g - avg_g) * strength).clamp(0.0_f32, 1.0_f32),
                (center.b + (center.b - avg_b) * strength).clamp(0.0_f32, 1.0_f32),
            );
        }
    }
}

fn apply_defocus_blur(pixels: &mut [RgbFloat], width: usize, height: usize, radius: usize) {
    let original = pixels.to_vec();
    let mut temp = original.clone();

    // Horizontal box blur
    for y in 0..height {
        let row = y * width;
        for x in 0..width {
            let mut sum_r = 0.0_f32;
            let mut sum_g = 0.0_f32;
            let mut sum_b = 0.0_f32;
            let mut count = 0.0_f32;

            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius).min(width - 1);
            for kx in x_start..=x_end {
                let p = original[row + kx];
                sum_r += p.r;
                sum_g += p.g;
                sum_b += p.b;
                count += 1.0_f32;
            }
            temp[row + x] = RgbFloat::new(sum_r / count, sum_g / count, sum_b / count);
        }
    }

    // Vertical box blur
    for y in 0..height {
        let row = y * width;
        for x in 0..width {
            let mut sum_r = 0.0_f32;
            let mut sum_g = 0.0_f32;
            let mut sum_b = 0.0_f32;
            let mut count = 0.0_f32;

            let y_start = y.saturating_sub(radius);
            let y_end = (y + radius).min(height - 1);
            for ky in y_start..=y_end {
                let p = temp[ky * width + x];
                sum_r += p.r;
                sum_g += p.g;
                sum_b += p.b;
                count += 1.0_f32;
            }
            pixels[row + x] = RgbFloat::new(sum_r / count, sum_g / count, sum_b / count);
        }
    }
}
