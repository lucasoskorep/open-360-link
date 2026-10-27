#![allow(dead_code)]

use egui::{ColorImage, Pos2, TextureHandle};
pub use open_360_link_protocol::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// UI mode matching `Webcam.Controller.curUIMode` (`MainWindow` vs `Toolbar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    MainWindow,
    Toolbar,
}

/// Right sidebar tab matching `Settings.qml` (`View`, `Effects`, `More`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightSidebarTab {
    View,
    Effects,
    More,
}

/// Active popup in Compact Toolbar Mode (`Toolbar.qml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarPopup {
    None,
    PtzPanel,
    TrackMode,
    VirtualBackground,
    PictureSetting,
    GestureCtrl,
    RemoteCtrl,
    MoreSetting,
}

/// Active tab in the Preferences modal (`preference/MoreSettings.qml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceTab {
    General,
    Hotkeys,
    About,
    GetLog,
}

/// Virtual background mode (`ImageBackground.qml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundEffectMode {
    None,
    StandardBlur,
    NaturalBokeh,
    CustomImage(usize),
}

/// Resolution & frame rate preset (`DefFormatCombo.qml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoFormatPreset {
    Uhd4k30,
    Fhd1080p60,
    Fhd1080p30,
    Hd720p60,
    Hd720p30,
    Sd360p30,
    Vertical1080x1920p30,
}

impl VideoFormatPreset {
    pub const ALL: [VideoFormatPreset; 7] = [
        Self::Uhd4k30,
        Self::Fhd1080p60,
        Self::Fhd1080p30,
        Self::Hd720p60,
        Self::Hd720p30,
        Self::Sd360p30,
        Self::Vertical1080x1920p30,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Uhd4k30 => "4K 30fps",
            Self::Fhd1080p60 => "1080p 60fps",
            Self::Fhd1080p30 => "1080p 30fps",
            Self::Hd720p60 => "720p 60fps",
            Self::Hd720p30 => "720p 30fps",
            Self::Sd360p30 => "360p 30fps",
            Self::Vertical1080x1920p30 => "1080x1920 30fps",
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Uhd4k30 => (3840, 2160),
            Self::Fhd1080p60 | Self::Fhd1080p30 => (1920, 1080),
            Self::Hd720p60 | Self::Hd720p30 => (1280, 720),
            Self::Sd360p30 => (640, 360),
            Self::Vertical1080x1920p30 => (1088, 1920),
        }
    }

    pub fn fps(&self) -> u32 {
        match self {
            Self::Fhd1080p60 | Self::Hd720p60 => 60,
            _ => 30,
        }
    }

    pub fn is_4k(&self) -> bool {
        matches!(self, Self::Uhd4k30)
    }

    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Vertical1080x1920p30)
    }
}

/// Saved PTZ preset item (`PTZControl.qml` repeater item).
#[derive(Clone)]
pub struct PtzPresetSlot {
    pub id: u8,
    pub name: String,
    pub saved: bool,
    pub pan_deg: f32,
    pub tilt_deg: f32,
    pub zoom_x: f32,
    pub host_x: f32,
    pub host_y: f32,
    pub mode: VideoMode,
    pub thumbnail: Option<TextureHandle>,
    pub raw_thumb: Option<ColorImage>,
    pub renaming: bool,
}

/// Saved Color Preset item (`ImageScreen.qml` template repeater item).
#[derive(Clone)]
pub struct ColorPresetSlot {
    pub id: i8,
    pub name: String,
    pub saved: bool,
    pub brightness: i16,
    pub contrast: u16,
    pub saturation: u16,
    pub sharpness: u16,
    pub wb_auto: bool,
    pub wb_kelvin: u16,
    pub auto_exposure: bool,
    pub ev_comp: f32,
    pub iso: u16,
    pub shutter: u32,
    pub hdr: bool,
    pub thumbnail: Option<TextureHandle>,
    pub raw_thumb: Option<ColorImage>,
    pub renaming: bool,
}

/// ISO discrete step values matching `Webcam.Controller.camera.isoList` (`ImageScreen.qml`).
pub const ISO_LIST: [u16; 16] = [
    100, 125, 160, 200, 250, 320, 400, 500, 640, 800, 1000, 1250, 1600, 2000, 2500, 3200,
];

/// Shutter speed denominator list (`1/N` seconds) matching `Webcam.Controller.camera.shutterList`.
pub const SHUTTER_DENOM_LIST: [u32; 20] = [
    30, 40, 50, 60, 80, 100, 120, 160, 200, 240, 320, 400, 500, 640, 800, 1000, 1600, 2000, 4000,
    8000,
];

/// Shared UI & camera state used across `main.rs`, `right_panel.rs`, and `video_engine.rs`.
pub struct CameraAppState {
    // Theme & window mode
    pub is_light_mode: bool,
    pub ui_mode: UiMode,
    pub right_tab: RightSidebarTab,
    pub toolbar_popup: ToolbarPopup,
    pub show_preferences: bool,
    pub preference_tab: PreferenceTab,
    pub show_qr_dialog: bool,
    pub show_virtual_cam_pop: bool,
    pub show_camera_log_pop: bool,
    pub show_factory_reset_pop: bool,
    pub show_pause_track_overlay: bool,
    pub show_deskview_pitch_pop: bool,

    // Preview & recording state
    pub preview_open: bool,
    pub virtual_camera_enabled: bool,
    pub format_preset: VideoFormatPreset,
    pub is_recording: bool,
    pub recording_started: Option<Instant>,
    pub screenshot_dir: PathBuf,
    pub recording_dir: PathBuf,
    pub toast_message: Option<(String, Instant, bool)>,

    // Device Info & Status
    pub device_info: DeviceInfo,
    pub ptz_info: PtzVersionInfo,
    pub device_status: DeviceStatus,

    // Gimbal / View / PTZ state
    pub pan_deg: f32,
    pub tilt_deg: f32,
    pub zoom_x: f32,        // 1.0 .. 4.0
    pub roll_deg: f32,      // -1.0 .. +1.0 (tenths of degree in hardware)
    pub deskview_pitch: i16, // -900 .. 0
    pub host_ptz_x: f32,    // 0.0 .. 1.0 normalized center X (Link 2C)
    pub host_ptz_y: f32,    // 0.0 .. 1.0 normalized center Y (Link 2C)
    pub bias_x: i16,
    pub bias_y: i16,
    pub ptz_presets: Vec<PtzPresetSlot>,
    pub active_preset_id: Option<u8>,
    pub open_more_positions: bool,

    // Video Mode & Tracking state
    pub video_mode: VideoMode,
    pub default_track_mode: u8, // 0 = Single Tracking, 1 = Group Tracking (Link 2)
    pub ai_zoom_enabled: bool,  // Smart Composition switch
    pub composition_style: CompositionStyle,
    pub track_speed: TrackSpeed,
    pub enable_auto_tracking: bool,
    pub enable_single_tap_tracking: bool,
    pub pause_track_enabled: bool,
    pub forbidden_areas: [ForbiddenAreaInfo; 4],
    pub tracked_targets: Vec<UvcRect>,
    pub selected_target_id: i16,
    pub whiteboard_corners: [Pos2; 4], // normalized [0..1] corners for Smart Whiteboard

    // Color & Exposure state (`ImageScreen.qml`)
    pub auto_exposure: bool,
    pub ev_compensation: f32, // -3.0 .. +3.0
    pub iso_index: usize,     // index into ISO_LIST
    pub shutter_index: usize, // index into SHUTTER_DENOM_LIST
    pub spline_points: Vec<(f32, f32)>, // normalized (0..1, 0..1) control points for DefSplineCanvas
    pub auto_focus: bool,
    pub manual_focus: u16, // 0..100
    pub auto_white_balance: bool,
    pub wb_kelvin: u16,       // 2000..10000
    pub open_more_color: bool, // Show/Collapse in Color section
    pub brightness: i16,      // 0..100
    pub contrast: u16,        // 0..100
    pub saturation: u16,      // 0..100
    pub sharpness: u16,       // 0..100
    pub hdr_enabled: bool,
    pub anti_flicker: PowerLineFrequency,
    pub startup_params_enabled: bool,
    pub color_presets: Vec<ColorPresetSlot>,
    pub active_color_preset_id: Option<i8>,
    pub open_more_color_presets: bool,

    // Background, Makeup & Filter state (`ImageBackground.qml`, `ImageBeauty.qml`, `ImageFilter.qml`)
    pub background_mode: BackgroundEffectMode,
    pub blur_intensity: u8, // 0..100
    pub custom_backgrounds: Vec<String>,
    pub makeup_enabled: bool,
    pub makeup_intensity: u8, // 0..100
    pub filter_dropdown_open: bool,
    pub filter_mode: FilterMode,

    // More Tab state (`MoreSettings.qml`)
    pub enable_all_gestures: bool,
    pub use_gesture_palm: bool,
    pub use_gesture_v: bool,
    pub use_gesture_l: bool,
    pub use_gesture_rock: bool,
    pub use_gesture_ok: bool,
    pub audio_capture_mode: AudioCaptureMode,
    pub audio_denoise: bool,
    pub horizontal_mirror: bool,
    pub vertical_mirror: bool,
    pub smart_adjustment: bool,
    pub horizontal_correction: bool,
    pub ultimate_privacy: bool,
    pub privacy_sleep: bool,
    pub vertical_resolution_mode: bool,
    pub forced_vertical_lock: bool,
    pub low_resolution_mode: bool,

    // Real-time hardware sync, continuous PTZ velocity, and AI target polling
    pub stream_initialized: bool,
    pub joystick_dragging: bool,
    pub last_ptz_cmd: (u8, i8, u8, i8),
    pub last_ptz_send_time: Option<Instant>,
    pub nudge_stop_at: Option<Instant>,
    pub pending_ptz_restore: Option<(Instant, i32, i32)>,
    pub ready_auto_track: bool,
    pub last_track_poll: Option<Instant>,
    pub last_hw_sync: Option<Instant>,
    pub last_saved_settings: Option<PersistedAppSettings>,
}

/// User settings persisted across sessions in `~/.config/open-360-link/settings.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedAppSettings {
    pub format_preset: VideoFormatPreset,
    pub auto_exposure: bool,
    pub ev_compensation: f32,
    pub iso_index: usize,
    pub shutter_index: usize,
    pub auto_focus: bool,
    pub manual_focus: u16,
    pub auto_white_balance: bool,
    pub wb_kelvin: u16,
    pub brightness: i16,
    pub contrast: u16,
    pub saturation: u16,
    pub sharpness: u16,
    pub hdr_enabled: bool,
    pub anti_flicker: PowerLineFrequency,
    pub startup_params_enabled: bool,
    pub filter_mode: FilterMode,
    pub makeup_enabled: bool,
    pub makeup_intensity: u8,
    pub track_speed: TrackSpeed,
    pub enable_single_tap_tracking: bool,
    pub enable_all_gestures: bool,
    pub use_gesture_palm: bool,
    pub use_gesture_v: bool,
    pub use_gesture_l: bool,
    pub audio_denoise: bool,
    pub horizontal_mirror: bool,
    pub vertical_mirror: bool,
    pub smart_adjustment: bool,
    pub horizontal_correction: bool,
    pub roll_deg: f32,
    pub vertical_resolution_mode: bool,
    pub forced_vertical_lock: bool,
    pub low_resolution_mode: bool,
}

impl PersistedAppSettings {
    fn base_config_dir() -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.trim().is_empty() {
                return PathBuf::from(xdg);
            }
        }
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(".config")
    }

    pub fn config_path() -> PathBuf {
        Self::base_config_dir()
            .join("open-360-link")
            .join("settings.json")
    }

    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str(&data) {
                return Some(parsed);
            }
        }
        // Automatic migration from legacy `~/.config/insta360-link-controller/settings.json`
        let legacy = Self::base_config_dir()
            .join("insta360-link-controller")
            .join("settings.json");
        let data = std::fs::read_to_string(legacy).ok()?;
        let parsed: Self = serde_json::from_str(&data).ok()?;
        parsed.save();
        Some(parsed)
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

impl Default for CameraAppState {
    fn default() -> Self {
        let mut ptz_presets = Vec::with_capacity(6);
        for i in 0..6u8 {
            ptz_presets.push(PtzPresetSlot {
                id: i,
                name: format!("Preset {}", i + 1),
                saved: i < 2,
                pan_deg: if i == 0 { 0.0 } else { 22.0 },
                tilt_deg: if i == 0 { 0.0 } else { -8.0 },
                zoom_x: if i == 0 { 1.0 } else { 1.5 },
                host_x: 0.5,
                host_y: 0.5,
                mode: VideoMode::Normal,
                thumbnail: None,
                raw_thumb: None,
                renaming: false,
            });
        }

        let color_presets = vec![ColorPresetSlot {
            id: 1,
            name: "Studio Warm".to_string(),
            saved: true,
            brightness: 54,
            contrast: 52,
            saturation: 56,
            sharpness: 50,
            wb_auto: false,
            wb_kelvin: 5600,
            auto_exposure: true,
            ev_comp: 0.3,
            iso: 100,
            shutter: 120,
            hdr: false,
            thumbnail: None,
            raw_thumb: None,
            renaming: false,
        }];

        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));

        let mut s = Self {
            is_light_mode: false,
            ui_mode: UiMode::MainWindow,
            right_tab: RightSidebarTab::View,
            toolbar_popup: ToolbarPopup::None,
            show_preferences: false,
            preference_tab: PreferenceTab::General,
            show_qr_dialog: false,
            show_virtual_cam_pop: false,
            show_camera_log_pop: false,
            show_factory_reset_pop: false,
            show_pause_track_overlay: false,
            show_deskview_pitch_pop: false,

            preview_open: true,
            virtual_camera_enabled: false,
            format_preset: VideoFormatPreset::Fhd1080p30,
            is_recording: false,
            recording_started: None,
            screenshot_dir: home.join("Pictures"),
            recording_dir: home.join("Movies"),
            toast_message: None,

            device_info: DeviceInfo {
                device_name: "Insta360 Link 2".to_string(),
                firmware_version: "v2.5.8.8_build1".to_string(),
                serial_number: "INSTA360SIM0001".to_string(),
            },
            ptz_info: PtzVersionInfo {
                version: "v1.3.1.0_build0".to_string(),
                pan: 0,
                tilt: 0,
                zoom: 100,
                status: 0,
            },
            device_status: DeviceStatus::empty(),

            pan_deg: 0.0,
            tilt_deg: 0.0,
            zoom_x: 1.0,
            roll_deg: 0.0,
            deskview_pitch: 0,
            host_ptz_x: 0.5,
            host_ptz_y: 0.5,
            bias_x: 0,
            bias_y: 0,
            ptz_presets,
            active_preset_id: Some(0),
            open_more_positions: false,

            video_mode: VideoMode::Normal,
            default_track_mode: 0,
            ai_zoom_enabled: true,
            composition_style: CompositionStyle::HalfBody,
            track_speed: TrackSpeed::Normal,
            enable_auto_tracking: true,
            enable_single_tap_tracking: true,
            pause_track_enabled: false,
            forbidden_areas: [
                ForbiddenAreaInfo {
                    enabled: false,
                    index: 0,
                    pan_min: -350,
                    pan_max: -150,
                    tilt_min: -200,
                    tilt_max: 200,
                },
                ForbiddenAreaInfo {
                    enabled: false,
                    index: 1,
                    pan_min: 150,
                    pan_max: 350,
                    tilt_min: -200,
                    tilt_max: 200,
                },
                ForbiddenAreaInfo {
                    enabled: false,
                    index: 2,
                    pan_min: -200,
                    pan_max: 200,
                    tilt_min: 100,
                    tilt_max: 300,
                },
                ForbiddenAreaInfo {
                    enabled: false,
                    index: 3,
                    pan_min: -200,
                    pan_max: 200,
                    tilt_min: -300,
                    tilt_max: -100,
                },
            ],
            tracked_targets: Vec::new(),
            selected_target_id: 0,
            whiteboard_corners: [
                Pos2::new(0.18, 0.16),
                Pos2::new(0.82, 0.16),
                Pos2::new(0.82, 0.78),
                Pos2::new(0.18, 0.78),
            ],

            auto_exposure: true,
            ev_compensation: 0.0,
            iso_index: 0,
            shutter_index: 5, // 1/100s
            spline_points: vec![(0.0, 0.0), (0.33, 0.33), (0.67, 0.67), (1.0, 1.0)],
            auto_focus: true,
            manual_focus: 50,
            auto_white_balance: true,
            wb_kelvin: 5000,
            open_more_color: false,
            brightness: 50,
            contrast: 50,
            saturation: 50,
            sharpness: 50,
            hdr_enabled: false,
            anti_flicker: PowerLineFrequency::Auto,
            startup_params_enabled: false,
            color_presets,
            active_color_preset_id: None,
            open_more_color_presets: false,

            background_mode: BackgroundEffectMode::None,
            blur_intensity: 60,
            custom_backgrounds: vec![
                "Modern Office".to_string(),
                "Minimalist Studio".to_string(),
                "Warm Bookshelf".to_string(),
            ],
            makeup_enabled: false,
            makeup_intensity: 45,
            filter_dropdown_open: true,
            filter_mode: FilterMode::None,

            enable_all_gestures: true,
            use_gesture_palm: true,
            use_gesture_v: true,
            use_gesture_l: true,
            use_gesture_rock: false,
            use_gesture_ok: false,
            audio_capture_mode: AudioCaptureMode::VoiceFocus,
            audio_denoise: true,
            horizontal_mirror: false,
            vertical_mirror: false,
            smart_adjustment: false,
            horizontal_correction: false,
            ultimate_privacy: false,
            privacy_sleep: false,
            vertical_resolution_mode: false,
            forced_vertical_lock: false,
            low_resolution_mode: false,

            stream_initialized: false,
            joystick_dragging: false,
            last_ptz_cmd: (0, 0, 0, 0),
            last_ptz_send_time: None,
            nudge_stop_at: None,
            pending_ptz_restore: None,
            ready_auto_track: false,
            last_track_poll: None,
            last_hw_sync: None,
            last_saved_settings: None,
        };
        if let Some(saved) = PersistedAppSettings::load() {
            s.apply_persisted(&saved);
            s.last_saved_settings = Some(saved);
        }
        s
    }
}

impl CameraAppState {
    pub fn show_toast(&mut self, msg: impl Into<String>, is_error: bool) {
        self.toast_message = Some((msg.into(), Instant::now(), is_error));
    }

    pub fn to_persisted(&self) -> PersistedAppSettings {
        PersistedAppSettings {
            format_preset: self.format_preset,
            auto_exposure: self.auto_exposure,
            ev_compensation: (self.ev_compensation * 10.0).round() / 10.0,
            iso_index: self.iso_index,
            shutter_index: self.shutter_index,
            auto_focus: self.auto_focus,
            manual_focus: self.manual_focus,
            auto_white_balance: self.auto_white_balance,
            wb_kelvin: self.wb_kelvin,
            brightness: self.brightness,
            contrast: self.contrast,
            saturation: self.saturation,
            sharpness: self.sharpness,
            hdr_enabled: self.hdr_enabled,
            anti_flicker: self.anti_flicker,
            startup_params_enabled: self.startup_params_enabled,
            filter_mode: self.filter_mode,
            makeup_enabled: self.makeup_enabled,
            makeup_intensity: self.makeup_intensity,
            track_speed: self.track_speed,
            enable_single_tap_tracking: self.enable_single_tap_tracking,
            enable_all_gestures: self.enable_all_gestures,
            use_gesture_palm: self.use_gesture_palm,
            use_gesture_v: self.use_gesture_v,
            use_gesture_l: self.use_gesture_l,
            audio_denoise: self.audio_denoise,
            horizontal_mirror: self.horizontal_mirror,
            vertical_mirror: self.vertical_mirror,
            smart_adjustment: self.smart_adjustment,
            horizontal_correction: self.horizontal_correction,
            roll_deg: (self.roll_deg * 10.0).round() / 10.0,
            vertical_resolution_mode: self.vertical_resolution_mode,
            forced_vertical_lock: self.forced_vertical_lock,
            low_resolution_mode: self.low_resolution_mode,
        }
    }

    pub fn apply_persisted(&mut self, p: &PersistedAppSettings) {
        self.format_preset = p.format_preset;
        self.auto_exposure = p.auto_exposure;
        self.ev_compensation = p.ev_compensation;
        self.iso_index = p.iso_index.min(ISO_LIST.len() - 1);
        self.shutter_index = p.shutter_index.min(SHUTTER_DENOM_LIST.len() - 1);
        self.auto_focus = p.auto_focus;
        self.manual_focus = p.manual_focus.min(100);
        self.auto_white_balance = p.auto_white_balance;
        self.wb_kelvin = p.wb_kelvin.clamp(2000, 10000);
        self.brightness = p.brightness.clamp(0, 100);
        self.contrast = p.contrast.min(100);
        self.saturation = p.saturation.min(100);
        self.sharpness = p.sharpness.min(100);
        self.hdr_enabled = p.hdr_enabled;
        self.anti_flicker = p.anti_flicker;
        self.startup_params_enabled = p.startup_params_enabled;
        self.filter_mode = p.filter_mode;
        self.makeup_enabled = p.makeup_enabled;
        self.makeup_intensity = p.makeup_intensity.min(100);
        self.track_speed = p.track_speed;
        self.enable_single_tap_tracking = p.enable_single_tap_tracking;
        self.enable_all_gestures = p.enable_all_gestures;
        self.use_gesture_palm = p.use_gesture_palm;
        self.use_gesture_v = p.use_gesture_v;
        self.use_gesture_l = p.use_gesture_l;
        self.audio_denoise = p.audio_denoise;
        self.horizontal_mirror = p.horizontal_mirror;
        self.vertical_mirror = p.vertical_mirror;
        self.smart_adjustment = p.smart_adjustment;
        self.horizontal_correction = p.horizontal_correction;
        self.roll_deg = p.roll_deg.clamp(-1.0, 1.0);
        self.vertical_resolution_mode =
            p.vertical_resolution_mode || p.format_preset.fps() > 30 || p.format_preset.is_vertical();
        self.forced_vertical_lock = p.forced_vertical_lock;
        self.low_resolution_mode = p.low_resolution_mode;
    }
}
