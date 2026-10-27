use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use crate::ids::{CameraModel, DeviceCapabilities, UvcUnitIds};

/// UVC Class-Specific Request Codes (`MacUVCCameraControl::SetData` / `GetData` at `0x10019282c`)
/// - `SET_CUR`: `bmRequestType = 0x21`, `bRequest = 0x01` (`w10 = 0x0121`)
/// - `GET_*`: `bmRequestType = 0xa1`, `bRequest = 0x81..0x87`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum UvcRequest {
    SetCur = 0x01,
    GetCur = 0x81,
    GetMin = 0x82,
    GetMax = 0x83,
    GetRes = 0x84,
    GetLen = 0x85,
    GetInfo = 0x86,
    GetDef = 0x87,
}

/// Standard UVC Input Terminal (Camera Terminal) Control Selectors (`MacUVCCameraController`)
pub mod ct_selectors {
    /// `CT_AE_MODE_CONTROL` (`0x02`, 1 byte: 1=Manual, 2=Auto, 4=ShutterPriority, 8=AperturePriority)
    pub const AE_MODE: u8 = 0x02;
    /// `CT_EXPOSURE_TIME_ABSOLUTE_CONTROL` (`0x04`, 4 bytes)
    pub const EXPOSURE_TIME_ABSOLUTE: u8 = 0x04;
    /// `CT_FOCUS_ABSOLUTE_CONTROL` (`0x06`, 2 bytes)
    pub const FOCUS_ABSOLUTE: u8 = 0x06;
    /// `CT_FOCUS_AUTO_CONTROL` (`0x08`, 1 byte: 0 or 1)
    pub const FOCUS_AUTO: u8 = 0x08;
    /// `CT_ZOOM_ABSOLUTE_CONTROL` (`0x0b`, 2 bytes, typically 100..400)
    pub const ZOOM_ABSOLUTE: u8 = 0x0b;
    /// `CT_PANTILT_ABSOLUTE_CONTROL` (`0x0d`, 8 bytes: i32 pan_arcsec, i32 tilt_arcsec)
    pub const PANTILT_ABSOLUTE: u8 = 0x0d;
    /// `CT_PANTILT_RELATIVE_CONTROL` (`0x0e`, 4 bytes: i8 pan_dir, u8 pan_speed, i8 tilt_dir, u8 tilt_speed)
    pub const PANTILT_RELATIVE: u8 = 0x0e;
    /// `CT_ROLL_ABSOLUTE_CONTROL` (`0x0f`, 2 bytes: i16 roll)
    pub const ROLL_ABSOLUTE: u8 = 0x0f;
}

/// Standard UVC Processing Unit Control Selectors (`MacUVCCameraController`)
pub mod pu_selectors {
    /// `PU_BRIGHTNESS_CONTROL` (`0x02`, 2 bytes: i16)
    pub const BRIGHTNESS: u8 = 0x02;
    /// `PU_CONTRAST_CONTROL` (`0x03`, 2 bytes: u16)
    pub const CONTRAST: u8 = 0x03;
    /// `PU_POWER_LINE_FREQUENCY_CONTROL` (`0x05`, 1 byte: 0=Disabled, 1=50Hz, 2=60Hz, 3=Auto)
    pub const POWER_LINE_FREQUENCY: u8 = 0x05;
    /// `PU_SATURATION_CONTROL` (`0x07`, 2 bytes: u16)
    pub const SATURATION: u8 = 0x07;
    /// `PU_SHARPNESS_CONTROL` (`0x08`, 2 bytes: u16)
    pub const SHARPNESS: u8 = 0x08;
    /// `PU_WHITE_BALANCE_TEMPERATURE_CONTROL` (`0x0a`, 2 bytes: u16 Kelvin)
    pub const WHITE_BALANCE_TEMPERATURE: u8 = 0x0a;
    /// `PU_WHITE_BALANCE_TEMPERATURE_AUTO_CONTROL` (`0x0b`, 1 byte: 0 or 1)
    pub const WHITE_BALANCE_TEMPERATURE_AUTO: u8 = 0x0b;
}

/// Insta360 Extension Unit 1 (`FAF1672D-B71B-4793-8C91-7B1C9B7F95F8`) Control Selectors
/// Verified across `MacUVCCameraExtendController::*` (`0x100195a18`–`0x10019bd00`).
pub mod xu1_selectors {
    /// `0x01`: Preset / Gimbal / PTZ / Factory Reset / Composition Bias / Forbidden Area multiplexed control
    /// - `[0, index, 0, 0]` (4B): SetCurPresetPosition
    /// - `[1, index, 0, 0]` (4B): AddPresetPosition
    /// - `[2, index, 0, 0]` (4B): DeletePresetPosition
    /// - `[3, 0, 0, 0]` (4B): ResetGimbal (PTZ home)
    /// - `[4, 0, 0, 0]` (4B): LoadFactoryDefault
    /// - `[6, x_lo, x_hi, y_lo, y_hi]` (5B): SetCompositionBias (x: i16, y: i16; 0x7fff = reset)
    /// - `[7, 1, 0, 0]` (4B): EnterTrackForbiddenAreaSetting
    /// - `[8, 1, 0, 0]` (4B): ExitTrackForbiddenAreaSetting
    pub const SYSTEM_AND_PRESET_CMD: u8 = 0x01;

    /// `0x02`: Video Mode & Auxiliary Data (`SetVideoMode` / `GetVideoMode`)
    /// - Set: 8 bytes (Link/Link 2) or 36 bytes (Link 2C/PUC3 Host)
    /// - Get: 36 bytes (`[mode_u8, flags_u8, x_i32, y_i32, w_i32, h_i32, pitch_i16, zoom_u16, ...]`)
    pub const VIDEO_MODE: u8 = 0x02;

    /// `0x03`: AI Tracking & Gesture Control
    /// - `GetGestureStatus` / `EnableAllGesture` / `EnableGestureWork`: 6-byte status (`[all, ok, palm, l, v, rock]`)
    /// - `ForceSetSmartWhiteBoard(mode)` (`[3, mode_u8, ...]`, 16B)
    /// - `SetTrackSpeed` / `GetTrackSpeed`: (`[5, speed_u8, ...]`, 16B)
    /// - `SetCompositionStyle` / `GetCompositionStyle`: (`[6, style_u8, ...]`, 16B)
    pub const TRACKING_AND_GESTURE: u8 = 0x03;

    /// `0x04`: Audio Noise Reduction & Audio Capture Mode (`2` bytes)
    /// - `EnableAudioNoiseReduction` / `GetAudioNoiseStatus`: `buf[0] = 0/1`
    /// - `SetAudioCaptureMode` / `GetAudioCaptureMode`: `buf[0] = mode (1..=5)`
    pub const AUDIO_CONTROL: u8 = 0x04;

    /// `0x05`: Select Tracked Person / Object Bounding Box (`SetTrackObj`, 10 bytes: `i16 x, y, w, h, id`)
    pub const TRACK_OBJECT_SELECT: u8 = 0x05;

    /// `0x06`: Detected Trackable Objects List (Link 1: `GetTrackObjLists`, N * 10 bytes of `UVCRect`)
    pub const TRACK_OBJECT_LIST_V1: u8 = 0x06;

    /// `0x07`: Exposure Curve & Manual Exposure (`GetExpouseCurve` / `SetExpouseCurve` / `EnableAutoExposure` / `GetAutoExposureStatus` / `SetISOValue` / `GetISOValue`, 259 bytes)
    pub const EXPOSURE_CURVE_AND_ISO: u8 = 0x07;

    /// `0x08`: Exposure Compensation (`SetExposureCompensation` / `GetExposureCompensation`, 4 bytes: `f32` EV)
    pub const EXPOSURE_COMPENSATION: u8 = 0x08;

    /// `0x09`: White Balance Extended Parameters (`SetWhiteBalanceParam` / `GetWhiteBalanceParam`, 129 bytes)
    pub const WHITE_BALANCE_EXT: u8 = 0x09;

    /// `0x0a`: Extended Function Bitmask (`EnableExtendFuncWork` / `GetExtendFuncStatus` / `EnableVerticalScreen` / `EnableLowResolution`, 8 bytes: `u32 mask, u32 values`)
    pub const EXTEND_FUNC_BITMASK: u8 = 0x0a;

    /// `0x0b`: Gimbal Relative Pan/Tilt (`SetPanTiltRelative` / `GetPanTiltRelative`, 4 bytes: `[pan_speed, pan_dir, tilt_speed, tilt_dir]`)
    pub const PANTILT_RELATIVE_EXT: u8 = 0x0b;

    /// `0x0c`: Device Status Flags (`GetDeviceStatus`, 4 bytes: `u32` bitflags)
    pub const DEVICE_STATUS: u8 = 0x0c;

    /// `0x0d`: PTZ Version & Gimbal Absolute Position (`GetPTZVersionInfo` / `GetPanTiltAbsoluteValue`, 30 bytes)
    pub const PTZ_VERSION_AND_ABS: u8 = 0x0d;

    /// `0x0e`: Preset Position Bitflags (`GetCurPresetPositionFlag`, 8 bytes)
    pub const PRESET_POSITION_FLAGS: u8 = 0x0e;

    /// `0x0f`: Switch to Read-Only Mass Storage Class (`SwitchMscReadOnlyClass`, 4 bytes: `[2, 0, 0, 0]`)
    /// Also used by `DownloadFileFromCamera` (`[1, 0, 0, 0]`).
    pub const USB_MODE_SWITCH: u8 = 0x0f;
}

/// Insta360 Extension Unit 2 (`E307E649-4618-A3FF-82FC-2D8B5F216773`) Control Selectors
/// Verified across `MacUVCCameraExtendController::*` (`0x100195a18`–`0x10019c478`).
pub mod xu2_selectors {
    /// `0x01`: Gesture Binding Map (`GetGestureBindingMap` / `SetGestureBindingFunc`)
    pub const GESTURE_BINDING_MAP: u8 = 0x01;

    /// `0x02`: Camera System & Timestamp (`SetTimestamp`, `SetIQSwitch`, `RestoreFactorySet`)
    /// - `[1, ts_le_bytes[0..8], ...]`: SetTimestamp
    /// - `[2, iq_bitmask_u32_le, ...]`: SetIQSwitch (D0=bit 0, D1=bit 1)
    /// - `[4, 1, ...]`: RestoreFactorySet(true) (reboot/full)
    /// - `[5, 1, ...]`: RestoreFactorySet(false) (settings only)
    pub const SYSTEM_TIME_AND_IQ: u8 = 0x02;

    /// `0x03`: Device Info & Serial Number (`GetDeviceInfo` / `GetSerialNumber`, 150 bytes:
    /// `[0..50]` device_name, `[50..100]` fw_version, `[100..150]` serial_number)
    pub const DEVICE_INFO: u8 = 0x03;

    /// `0x04`: Image Template Management (`GetImageTemplateInfo`, `SetImageTemplate`, `DeleteTemplate`, 11 bytes)
    pub const IMAGE_TEMPLATE_INFO: u8 = 0x04;

    /// `0x05`: Image Template Name (`GetImageTemplateName` / `SetImageTemplateName`, 65 bytes: `[id_i8, name[64]]`)
    pub const IMAGE_TEMPLATE_NAME: u8 = 0x05;

    /// `0x06`: Image Filter Mode (`SetFilter` / `GetFilter`, 1 byte: `0..=8`)
    pub const FILTER_MODE: u8 = 0x06;

    /// `0x08`: Fill Light & Privacy Mode (`GetFillLightParam`, `SetPrivacyMode`, `GetPrivacyMode`, 4 bytes)
    /// - `SetPrivacyMode(enable)`: `[2, enable_u8, 0, 0]`
    /// - `GetPrivacyMode`: `status = buf[0]`, `enabled = buf[1] != 0`
    pub const PRIVACY_AND_FILL_LIGHT: u8 = 0x08;

    /// `0x09`: Host Digital PTZ Crop (`SetHostPTZ`, 8 bytes: `u16 x, u16 y, u16 w, u16 h`)
    pub const HOST_PTZ: u8 = 0x09;

    /// `0x0a`: Detected Trackable Objects List V2 (Link 2 / Link 2C: `GetNewTrackObjLists`)
    pub const TRACK_OBJECT_LIST_V2: u8 = 0x0a;

    /// `0x0b`: Tracking Forbidden Areas (`SetTrackForbiddenArea`, `GetTrackForbiddenArea`, `EnableTrackForbiddenArea`)
    pub const TRACK_FORBIDDEN_AREA: u8 = 0x0b;

    /// `0x0c`: Default Track Mode (`SetDefaultTrackMode` / `GetDefaultTrackMode`, 1 byte)
    pub const DEFAULT_TRACK_MODE: u8 = 0x0c;

    /// `0x0d`: Horizontal Correction (`EnableHorizontalCorrection` / `GetEnableHorizontalCorrection`, 1 byte)
    pub const HORIZONTAL_CORRECTION: u8 = 0x0d;

    /// `0x0e`: Real Zoom & UDisk Open Count (`GetRealZoomValue`, `GetUDiskOpenCount`, `SetUDiskOpenCount`)
    pub const REAL_ZOOM_AND_UDISK: u8 = 0x0e;

    /// `0x0f`: Diagnostic Report Data (`GetReportData`)
    pub const REPORT_DATA: u8 = 0x0f;
}

/// Discovered Insta360 camera metadata (`UVCCameraInfo` at `0x100191844`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredCamera {
    pub vendor_id: u16,
    pub product_id: u16,
    pub model: CameraModel,
    pub name: String,
    pub unique_id: String,
    pub bus_number: u8,
    pub device_address: u8,
    pub unit_ids: UvcUnitIds,
    pub capabilities: DeviceCapabilities,
}

/// Range and default metadata for a standard UVC control (`CameraControlInfo` at `0x100193694`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ControlRange {
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub default: i64,
    pub flags: i64,
}

/// Relative pan/tilt speed and direction (`CameraControlRelativeInfo` at `0x1001990f4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RelativeControl {
    pub min_speed: u8,
    pub max_speed: u8,
    pub step_speed: u8,
    pub default_speed: u8,
    pub current_speed: u8,
    /// Direction: `0` = stop, `1` = positive (right/up), `-1` (or `0xff`) = negative (left/down)
    pub direction: i8,
}

/// Camera Video Mode (`VideoMode` in `CameraInsta` at `0x1000601d4`–`0x100060234`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum VideoMode {
    #[default]
    Normal = 0,
    AutoTracking = 1,
    WhiteBoard = 4,
    OverheadShot = 5,
    DeskView = 6,
    AutoFraming = 7,
    SmartWhiteBoard = 8,
}

impl VideoMode {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Normal,
            1 => Self::AutoTracking,
            4 => Self::WhiteBoard,
            5 => Self::OverheadShot,
            6 => Self::DeskView,
            7 => Self::AutoFraming,
            8 | 10 => Self::SmartWhiteBoard,
            _ => Self::Normal,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::AutoTracking => "AI Tracking",
            Self::WhiteBoard => "Whiteboard",
            Self::OverheadShot => "Overhead",
            Self::DeskView => "DeskView",
            Self::AutoFraming => "Auto Framing",
            Self::SmartWhiteBoard => "Smart Whiteboard",
        }
    }
}

/// Auxiliary data sent/received with `VideoMode` (`VideoModeAuxiliaryData` at `0x100197088` & `0x1001978b8`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoModeAuxData {
    pub flags: u8,
    /// Optional 4-corner or bounding rectangle coordinates (`x, y, width, height` or whiteboard corners)
    pub rect: [i32; 4],
    /// DeskView / Host pitch angle (`i16`, e.g., -900..0 or -61926 sentinel)
    pub pitch: i16,
    /// Zoom factor (`u16`, default `100`)
    pub zoom: u16,
    /// Extra mode-specific bytes (up to 36 bytes total wire payload)
    pub extra: [u8; 14],
}

impl Default for VideoModeAuxData {
    fn default() -> Self {
        Self {
            flags: 0,
            rect: [0, 0, 0, 0],
            pitch: 0,
            zoom: 100,
            extra: [0u8; 14],
        }
    }
}

/// AI Tracking / Framing Composition Style (`CompositionStyle` at `0x100197900` & `PTZControl.qml` line 944).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum CompositionStyle {
    Custom = 0,
    #[default]
    Head = 1,
    HalfBody = 2,
    WholeBody = 3,
}

impl CompositionStyle {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Head,
            2 => Self::HalfBody,
            3 => Self::WholeBody,
            _ => Self::Head,
        }
    }
}

/// Gimbal AI Tracking Speed (`TrackSpeed` at `0x1001979e4` & `PTZControl.qml` line 1051).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum TrackSpeed {
    Fast = 0,
    #[default]
    Normal = 1,
    Slow = 2,
}

impl TrackSpeed {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Fast,
            1 => Self::Normal,
            _ => Self::Slow,
        }
    }
}

/// Extended Function bit indices in XU1 selector `0x0a` (`ExtendFuction` jump table at `0x102864a20`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ExtendFunction {
    /// Bit 0: `ai_zoom_checked_`
    AiZoom = 0,
    /// Bit 1: Reserved
    Reserved1 = 1,
    /// Bit 2: `enable_hdr_`
    Hdr = 2,
    /// Bit 3: `mirror_checked_` (Horizontal Mirror)
    HorizontalMirror = 3,
    /// Bit 4: `enable_all_gestures_`
    AllGestures = 4,
    /// Bit 5: `enable_vertical_mode_` (9:16 Vertical Resolution)
    VerticalMode = 5,
    /// Bit 6: `enable_startup_params_` (Save Startup Parameters to Camera)
    StartupParams = 6,
    /// Bit 7: `enable_single_tap_tracking_`
    SingleTapTracking = 7,
    /// Bit 8: `enable_auto_tracking_`
    AutoTracking = 8,
    /// Bit 9: `smart_adjustment_checked_`
    SmartAdjustment = 9,
    /// Bit 10: `forced_vertical_` (Uncropped 9:16 Portrait)
    ForcedVertical = 10,
    /// Bit 11: `enable_privacy_mode_` (Ultimate Privacy / 10s Sleep)
    UltimatePrivacyMode = 11,
    /// Bit 12: `enable_vertically_mirror_` (Vertical Flip)
    VerticalMirror = 12,
    /// Bit 13: `enable_low_resolution_`
    LowResolution = 13,
    /// Bit 14: `enable_track_forbidden_area_`
    TrackForbiddenArea = 14,
}

impl ExtendFunction {
    pub const ALL: [ExtendFunction; 14] = [
        Self::AiZoom,
        Self::Hdr,
        Self::HorizontalMirror,
        Self::AllGestures,
        Self::VerticalMode,
        Self::StartupParams,
        Self::SingleTapTracking,
        Self::AutoTracking,
        Self::SmartAdjustment,
        Self::ForcedVertical,
        Self::UltimatePrivacyMode,
        Self::VerticalMirror,
        Self::LowResolution,
        Self::TrackForbiddenArea,
    ];

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::AiZoom),
            1 => Some(Self::Reserved1),
            2 => Some(Self::Hdr),
            3 => Some(Self::HorizontalMirror),
            4 => Some(Self::AllGestures),
            5 => Some(Self::VerticalMode),
            6 => Some(Self::StartupParams),
            7 => Some(Self::SingleTapTracking),
            8 => Some(Self::AutoTracking),
            9 => Some(Self::SmartAdjustment),
            10 => Some(Self::ForcedVertical),
            11 => Some(Self::UltimatePrivacyMode),
            12 => Some(Self::VerticalMirror),
            13 => Some(Self::LowResolution),
            14 => Some(Self::TrackForbiddenArea),
            _ => None,
        }
    }
}

/// Hand Gesture types (`EnableGestureWork` at `0x100197bc8` and `setUseGesture*` at `0x100067f58`–`0x100068888`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Gesture {
    Ok = 0,
    Palm = 1,
    LShape = 2,
    VShape = 3,
    Rock = 4,
}

impl Gesture {
    pub const ALL: [Gesture; 5] = [
        Self::Ok,
        Self::Palm,
        Self::LShape,
        Self::VShape,
        Self::Rock,
    ];
}

/// Gesture-bound action (`GestureFunction` in `GetGestureBindingMap` at `0x100198384`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum GestureFunction {
    #[default]
    None = 0,
    AutoTracking = 1,
    Zoom = 2,
    WhiteBoard = 3,
    AutoFraming = 4,
}

impl GestureFunction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::AutoTracking,
            2 => Self::Zoom,
            3 => Self::WhiteBoard,
            4 => Self::AutoFraming,
            _ => Self::None,
        }
    }
}

/// Audio Capture / Microphone Mode (`AudioCaptureMode` at `0x10019bf54` & `0x1000702a8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum AudioCaptureMode {
    /// Voice Focus (Directional beamforming)
    #[default]
    VoiceFocus = 1,
    /// Voice Suppression / Noise Canceling (Wide)
    VoiceSuppression = 2,
    /// Original / Music Balance
    MusicBalance = 3,
    /// Original Sound
    Original = 4,
    /// Sound Wall / Audio Fence (PUC3)
    SoundWall = 5,
}

impl AudioCaptureMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::VoiceFocus,
            2 => Self::VoiceSuppression,
            3 => Self::MusicBalance,
            4 => Self::Original,
            5 => Self::SoundWall,
            _ => Self::VoiceFocus,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::VoiceFocus => "Voice Focus",
            Self::VoiceSuppression => "Noise Suppression",
            Self::MusicBalance => "Music Balance",
            Self::Original => "Original Sound",
            Self::SoundWall => "Sound Wall",
        }
    }
}

/// Power Line Anti-Flicker Frequency (`PowerLineFrequency` at `0x100194ecc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum PowerLineFrequency {
    #[default]
    Disabled = 0,
    Hz50 = 1,
    Hz60 = 2,
    Auto = 3,
}

impl PowerLineFrequency {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Hz50,
            2 => Self::Hz60,
            3 => Self::Auto,
            _ => Self::Disabled,
        }
    }
}

/// Hardware Filter Mode (`FilterMode` at `0x10019bdb0` & `0x100096f94`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum FilterMode {
    #[default]
    None = 0,
    Style1 = 1,
    Style2 = 2,
    Fresh = 3,
     Amber = 4,
    Film1 = 5,
    Film2 = 6,
    Cool = 7,
    Warm = 8,
}

impl FilterMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Style1,
            2 => Self::Style2,
            3 => Self::Fresh,
            4 => Self::Amber,
            5 => Self::Film1,
            6 => Self::Film2,
            7 => Self::Cool,
            8 => Self::Warm,
            _ => Self::None,
        }
    }
}

/// Exposure Curve & Manual ISO/Exposure state (`ExpouseCurve` at `0x10019aaa0`–`0x10019b7d0`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureCurve {
    /// `0` = Manual Exposure, `1` = Auto Exposure
    pub mode: u8,
    /// Manual ISO value (`u16`, e.g. 100..=3200)
    pub iso: u16,
    /// 128-point tone/exposure curve (`[u16; 128]`)
    pub points: Vec<u16>,
}

impl Default for ExposureCurve {
    fn default() -> Self {
        Self {
            mode: 1,
            iso: 100,
            points: vec![0; 128],
        }
    }
}

/// Extended White Balance Parameters (`WhiteBalanceParam` at `0x10019afe0`–`0x10019b264`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhiteBalanceParam {
    pub mode: u8,
    pub gains: Vec<u32>,
}

/// Camera Device Information (`DeviceInfo` at `0x100196e74`, 150 bytes across three 50-byte ASCII fields).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_name: String,
    pub firmware_version: String,
    pub serial_number: String,
}

/// PTZ Gimbal Firmware Version & Absolute Pan/Tilt Angles (`PTZVerInfo` at `0x1001993bc`, 30 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PtzVersionInfo {
    pub version: String,
    pub pan: i16,
    pub tilt: i16,
    pub zoom: u16,
    pub status: u8,
}

bitflags! {
    /// Device runtime status flags (`GetDeviceStatus` at `0x10019b8dc`, 4-byte `u32`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct DeviceStatus: u32 {
        const TRACKING_ACTIVE   = 1 << 0;
        const WHITEBOARD_ACTIVE = 1 << 1;
        const GESTURE_DETECTED  = 1 << 2;
        const PRIVACY_SLEEP     = 1 << 3;
        const OVERHEAD_MODE     = 1 << 4;
        const DESKVIEW_MODE     = 1 << 5;
        const UPGRADING         = 1 << 6;
    }
}

/// Bounding rectangle for tracked person/object (`UVCRect` at `0x100198a38`, 10 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UvcRect {
    pub x: i16,
    pub y: i16,
    pub width: i16,
    pub height: i16,
    pub track_id: i16,
}

/// Digital Host PTZ crop box (`HostPTZInfo` at `0x10019bec0`, 8 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPtzInfo {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Default for HostPtzInfo {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 5140,
            height: 5140,
        }
    }
}

/// Tracking Forbidden Area entry (`ForbiddenAreaInfo_s` at `0x10019bff4`, 10 bytes per entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ForbiddenAreaInfo {
    pub enabled: bool,
    pub index: u8,
    pub pan_min: i16,
    pub pan_max: i16,
    pub tilt_min: i16,
    pub tilt_max: i16,
}

/// Preset slot flags (`PresetFlag` at `0x1001997c0`, 8 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PresetFlag {
    pub slot_mask: u8,
    pub current_slot: u8,
    pub max_slots: u8,
    pub raw: [u8; 8],
}

/// Image Template Info (`ImageTemplateInfo` at `0x10019ba4c`, 11 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ImageTemplateInfo {
    pub current_id: i8,
    pub operation: u8,
    pub bitmask: u64,
    pub max_templates: u8,
}

/// Image Template Operation (`ImageOperation` at `0x10019bb08`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ImageOperation {
    None = 0,
    SaveOrUpdate = 1,
    Apply = 2,
}
