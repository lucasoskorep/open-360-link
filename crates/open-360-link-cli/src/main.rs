use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use open_360_link_protocol::{
    list_connected_cameras, AudioCaptureMode, CameraModel, CompositionStyle, DeviceInfo,
    DeviceStatus, DiscoveredCamera, ExtendFunction, FilterMode, ForbiddenAreaInfo, Gesture,
    GestureFunction, HostPtzInfo, ImageOperation, Insta360CameraController, MockCameraTransport,
    NusbTransport, PowerLineFrequency, PtzVersionInfo, TrackSpeed, TransportError,
    UvcControlTransport, UvcRect, UvcRequest, VideoMode, VideoModeAuxData,
};
use serde::Serialize;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "open-360-link-ctl",
    version,
    about = "Open 360 Link: Cross-platform CLI controller for Insta360 Link, Link 2, and Link 2C cameras"
)]
pub struct Cli {
    /// Run against an in-memory simulated camera model instead of physical USB
    #[arg(long, global = true, value_enum, value_name = "MODEL")]
    pub mock: Option<MockModelArg>,

    /// Camera index (0, 1, ...) or unique_id when multiple physical cameras are connected
    #[arg(long, global = true, value_name = "INDEX_OR_ID")]
    pub device: Option<String>,

    /// Output structured JSON for any query or command
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MockModelArg {
    #[value(name = "link", alias = "link1")]
    Link,
    #[value(name = "link2")]
    Link2,
    #[value(name = "link2c")]
    Link2c,
}

impl From<MockModelArg> for CameraModel {
    fn from(val: MockModelArg) -> Self {
        match val {
            MockModelArg::Link => CameraModel::Link,
            MockModelArg::Link2 => CameraModel::Link2,
            MockModelArg::Link2c => CameraModel::Link2C,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List connected Insta360 Link family cameras (or simulated camera with --mock)
    List,

    /// Query and display device info, gimbal PTZ version, status flags, video mode, and extended features
    Info,

    /// Pan, Tilt, Zoom, Roll, Composition Bias, and Host Digital Crop controls
    #[command(subcommand)]
    Ptz(PtzCommand),

    /// Manage camera preset positions (0..5)
    #[command(subcommand)]
    Preset(PresetCommand),

    /// Get or set camera video mode (Normal, AI Tracking, Whiteboard, Overhead, DeskView, Auto Framing, Smart Whiteboard)
    #[command(subcommand)]
    Mode(ModeCommand),

    /// Configure AI tracking speed, framing style, hand gestures, and tracked target selection
    #[command(subcommand)]
    Tracking(TrackingCommand),

    /// Query and configure image settings, exposure, white balance, filters, and onboard templates
    #[command(subcommand)]
    Image(ImageCommand),

    /// Query and configure microphone capture mode and AI noise reduction
    #[command(subcommand)]
    Audio(AudioCommand),

    /// Configure extended hardware features, privacy sleep, anti-flicker, and system maintenance
    #[command(subcommand)]
    System(SystemCommand),
}

#[derive(Subcommand, Debug)]
pub enum PtzCommand {
    /// Show current pan/tilt, zoom, roll, and PTZ version info
    Get,

    /// Set absolute gimbal pan and tilt angles in arcseconds (1 degree = 3600 arcsec)
    Set {
        #[arg(long, allow_hyphen_values = true, value_name = "ARCSEC")]
        pan: i32,
        #[arg(long, allow_hyphen_values = true, value_name = "ARCSEC")]
        tilt: i32,
    },

    /// Move gimbal with relative speed (0..10) and direction (-1, 0, 1)
    Move {
        #[arg(long, default_value_t = 0, value_name = "0..10")]
        pan_speed: u8,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true, value_name = "-1|0|1")]
        pan_dir: i8,
        #[arg(long, default_value_t = 0, value_name = "0..10")]
        tilt_speed: u8,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true, value_name = "-1|0|1")]
        tilt_dir: i8,
    },

    /// Stop ongoing relative pan/tilt movement
    Stop,

    /// Set digital zoom multiplier (100 = 1.0x .. 400 = 4.0x)
    Zoom {
        #[arg(value_name = "100..400")]
        zoom: u16,
    },

    /// Set gimbal roll angle in tenths of a degree (-1800..1800)
    Roll {
        #[arg(allow_hyphen_values = true, value_name = "-1800..1800")]
        roll: i16,
    },

    /// Set AI tracking composition pixel bias offset
    Bias {
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        x: i16,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        y: i16,
    },

    /// Reset AI tracking composition bias to center (0x7fff, 0x7fff)
    ResetBias,

    /// Set digital host PTZ crop window (Link 2C / Host mode)
    HostCrop {
        #[arg(long, value_name = "U16")]
        x: u16,
        #[arg(long, value_name = "U16")]
        y: u16,
        #[arg(long, value_name = "U16")]
        width: u16,
        #[arg(long, value_name = "U16")]
        height: u16,
    },

    /// Reset gimbal to home position
    Reset,
}

#[derive(Subcommand, Debug)]
pub enum PresetCommand {
    /// List saved preset slots and active preset index
    List,

    /// Recall a saved preset slot by index
    Recall {
        #[arg(value_name = "INDEX")]
        index: u8,
    },

    /// Save current PTZ position to preset slot index
    Save {
        #[arg(value_name = "INDEX")]
        index: u8,
    },

    /// Delete a saved preset slot by index
    Delete {
        #[arg(value_name = "INDEX")]
        index: u8,
    },
}

#[derive(Subcommand, Debug)]
pub enum ModeCommand {
    /// Get active video mode and auxiliary parameters
    Get,

    /// Set active video mode and optional pitch/zoom auxiliary parameters
    Set {
        #[arg(value_enum)]
        mode: VideoModeArg,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        pitch: Option<i16>,
        #[arg(long, value_name = "U16")]
        zoom: Option<u16>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum VideoModeArg {
    #[value(name = "normal")]
    Normal,
    #[value(name = "auto-tracking", alias = "tracking")]
    AutoTracking,
    #[value(name = "whiteboard")]
    Whiteboard,
    #[value(name = "overhead", alias = "overhead-shot")]
    Overhead,
    #[value(name = "deskview")]
    Deskview,
    #[value(name = "auto-framing", alias = "framing")]
    AutoFraming,
    #[value(name = "smart-whiteboard")]
    SmartWhiteboard,
}

impl From<VideoModeArg> for VideoMode {
    fn from(val: VideoModeArg) -> Self {
        match val {
            VideoModeArg::Normal => VideoMode::Normal,
            VideoModeArg::AutoTracking => VideoMode::AutoTracking,
            VideoModeArg::Whiteboard => VideoMode::WhiteBoard,
            VideoModeArg::Overhead => VideoMode::OverheadShot,
            VideoModeArg::Deskview => VideoMode::DeskView,
            VideoModeArg::AutoFraming => VideoMode::AutoFraming,
            VideoModeArg::SmartWhiteboard => VideoMode::SmartWhiteBoard,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum TrackingCommand {
    /// Set gimbal AI tracking speed
    Speed {
        #[arg(value_enum)]
        speed: TrackSpeedArg,
    },

    /// Set AI tracking / framing composition style
    Style {
        #[arg(value_enum)]
        style: CompositionStyleArg,
    },

    /// Query or configure hand gesture recognition toggles
    Gestures {
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        all: Option<bool>,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        ok: Option<bool>,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        palm: Option<bool>,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        l: Option<bool>,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        v: Option<bool>,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        rock: Option<bool>,
    },

    /// List currently detected trackable object bounding boxes
    Objects,

    /// Select a specific tracked object bounding box
    Select {
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        x: i16,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        y: i16,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        width: i16,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        height: i16,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true, value_name = "I16")]
        id: i16,
    },

    /// Query current hand gesture function bindings
    GestureBindings,

    /// Bind a hand gesture to a tracking/framing function
    BindGesture {
        #[arg(value_enum, value_name = "GESTURE")]
        gesture: GestureArg,
        #[arg(value_enum, value_name = "FUNC")]
        func: GestureFuncArg,
    },

    /// Configure tracking forbidden area zones
    #[command(subcommand)]
    ForbiddenAreas(ForbiddenAreasCommand),

    /// Force set smart whiteboard mode
    ForceSmartWhiteboard {
        #[arg(value_name = "MODE")]
        mode: u8,
    },
}

pub type TrackingCmd = TrackingCommand;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TrackSpeedArg {
    #[value(name = "slow")]
    Slow,
    #[value(name = "normal")]
    Normal,
    #[value(name = "fast")]
    Fast,
}

impl From<TrackSpeedArg> for TrackSpeed {
    fn from(val: TrackSpeedArg) -> Self {
        match val {
            TrackSpeedArg::Slow => TrackSpeed::Slow,
            TrackSpeedArg::Normal => TrackSpeed::Normal,
            TrackSpeedArg::Fast => TrackSpeed::Fast,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CompositionStyleArg {
    #[value(name = "head")]
    Head,
    #[value(name = "half-body")]
    HalfBody,
    #[value(name = "whole-body")]
    WholeBody,
    #[value(name = "custom")]
    Custom,
}

impl From<CompositionStyleArg> for CompositionStyle {
    fn from(val: CompositionStyleArg) -> Self {
        match val {
            CompositionStyleArg::Head => CompositionStyle::Head,
            CompositionStyleArg::HalfBody => CompositionStyle::HalfBody,
            CompositionStyleArg::WholeBody => CompositionStyle::WholeBody,
            CompositionStyleArg::Custom => CompositionStyle::Custom,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum ForbiddenAreasCommand {
    /// List configured tracking forbidden areas
    List,

    /// Enter tracking forbidden area setup mode
    EnterSetup,

    /// Exit tracking forbidden area setup mode
    ExitSetup,

    /// Enable or disable tracking forbidden area feature
    Enable {
        #[arg(action = ArgAction::Set, value_name = "BOOL", value_parser = parse_bool)]
        enable: bool,
    },

    /// Set a tracking forbidden area boundary zone
    Set {
        #[arg(long, value_name = "0..3")]
        index: u8,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        enabled: bool,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        pan_min: i16,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        pan_max: i16,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        tilt_min: i16,
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        tilt_max: i16,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GestureArg {
    #[value(name = "ok")]
    Ok,
    #[value(name = "palm")]
    Palm,
    #[value(name = "l")]
    L,
    #[value(name = "v")]
    V,
    #[value(name = "rock")]
    Rock,
}

impl From<GestureArg> for Gesture {
    fn from(val: GestureArg) -> Self {
        match val {
            GestureArg::Ok => Gesture::Ok,
            GestureArg::Palm => Gesture::Palm,
            GestureArg::L => Gesture::LShape,
            GestureArg::V => Gesture::VShape,
            GestureArg::Rock => Gesture::Rock,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GestureFuncArg {
    #[value(name = "none")]
    None,
    #[value(name = "auto-tracking")]
    AutoTracking,
    #[value(name = "zoom")]
    Zoom,
    #[value(name = "whiteboard")]
    Whiteboard,
    #[value(name = "auto-framing")]
    AutoFraming,
}

impl From<GestureFuncArg> for GestureFunction {
    fn from(val: GestureFuncArg) -> Self {
        match val {
            GestureFuncArg::None => GestureFunction::None,
            GestureFuncArg::AutoTracking => GestureFunction::AutoTracking,
            GestureFuncArg::Zoom => GestureFunction::Zoom,
            GestureFuncArg::Whiteboard => GestureFunction::WhiteBoard,
            GestureFuncArg::AutoFraming => GestureFunction::AutoFraming,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum ImageCommand {
    /// Get all image, exposure, white balance, filter, and HDR parameters
    Get,

    /// Set one or more image, exposure, white balance, filter, or HDR parameters
    Set {
        #[arg(long, allow_hyphen_values = true, value_name = "I16")]
        brightness: Option<i16>,
        #[arg(long, value_name = "U16")]
        contrast: Option<u16>,
        #[arg(long, value_name = "U16")]
        saturation: Option<u16>,
        #[arg(long, value_name = "U16")]
        sharpness: Option<u16>,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        auto_wb: Option<bool>,
        #[arg(long, value_name = "U16")]
        wb_kelvin: Option<u16>,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        auto_exposure: Option<bool>,
        #[arg(long, value_name = "U16")]
        iso: Option<u16>,
        #[arg(long, value_name = "U32")]
        shutter: Option<u32>,
        #[arg(long, allow_hyphen_values = true, value_name = "F32")]
        ev: Option<f32>,
        #[arg(long, value_name = "BOOL", value_parser = parse_bool)]
        hdr: Option<bool>,
        #[arg(long, value_enum, value_name = "FILTER")]
        filter: Option<FilterModeArg>,
    },

    /// Manage onboard camera image templates
    #[command(subcommand)]
    Template(TemplateCommand),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FilterModeArg {
    #[value(name = "none")]
    None,
    #[value(name = "style1")]
    Style1,
    #[value(name = "style2")]
    Style2,
    #[value(name = "fresh")]
    Fresh,
    #[value(name = "amber")]
    Amber,
    #[value(name = "film1")]
    Film1,
    #[value(name = "film2")]
    Film2,
    #[value(name = "cool")]
    Cool,
    #[value(name = "warm")]
    Warm,
}

impl From<FilterModeArg> for FilterMode {
    fn from(val: FilterModeArg) -> Self {
        match val {
            FilterModeArg::None => FilterMode::None,
            FilterModeArg::Style1 => FilterMode::Style1,
            FilterModeArg::Style2 => FilterMode::Style2,
            FilterModeArg::Fresh => FilterMode::Fresh,
            FilterModeArg::Amber => FilterMode::Amber,
            FilterModeArg::Film1 => FilterMode::Film1,
            FilterModeArg::Film2 => FilterMode::Film2,
            FilterModeArg::Cool => FilterMode::Cool,
            FilterModeArg::Warm => FilterMode::Warm,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum TemplateCommand {
    /// Get image template bitmask and active template ID
    Info,

    /// Apply a saved image template by ID
    Apply {
        #[arg(value_name = "ID")]
        id: i8,
    },

    /// Save current image settings to template slot ID
    Save {
        #[arg(value_name = "ID")]
        id: i8,
        #[arg(long, value_name = "STR")]
        name: Option<String>,
    },

    /// Delete a saved image template by ID
    Delete {
        #[arg(value_name = "ID")]
        id: i8,
    },
}

#[derive(Subcommand, Debug)]
pub enum AudioCommand {
    /// Get current audio capture mode and AI noise reduction status
    Get,

    /// Set microphone audio capture mode
    Mode {
        #[arg(value_enum)]
        mode: AudioModeArg,
    },

    /// Enable or disable AI audio noise reduction
    Denoise {
        #[arg(action = ArgAction::Set, value_name = "BOOL", value_parser = parse_bool)]
        enable: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AudioModeArg {
    #[value(name = "voice-focus")]
    VoiceFocus,
    #[value(name = "voice-suppression")]
    VoiceSuppression,
    #[value(name = "music-balance")]
    MusicBalance,
    #[value(name = "original")]
    Original,
    #[value(name = "sound-wall")]
    SoundWall,
}

impl From<AudioModeArg> for AudioCaptureMode {
    fn from(val: AudioModeArg) -> Self {
        match val {
            AudioModeArg::VoiceFocus => AudioCaptureMode::VoiceFocus,
            AudioModeArg::VoiceSuppression => AudioCaptureMode::VoiceSuppression,
            AudioModeArg::MusicBalance => AudioCaptureMode::MusicBalance,
            AudioModeArg::Original => AudioCaptureMode::Original,
            AudioModeArg::SoundWall => AudioCaptureMode::SoundWall,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum SystemCommand {
    /// Enable or disable an extended hardware feature bit (`XU1 0x0a`)
    Feature {
        #[arg(value_enum)]
        feature: ExtendFunctionArg,
        #[arg(action = ArgAction::Set, value_name = "BOOL", value_parser = parse_bool)]
        enable: bool,
    },

    /// Enable or disable hardware privacy mode (`XU2 0x08`)
    Privacy {
        #[arg(action = ArgAction::Set, value_name = "BOOL", value_parser = parse_bool)]
        enable: bool,
    },

    /// Enable or disable horizontal correction (`XU2 0x0d`)
    HorizontalCorrection {
        #[arg(action = ArgAction::Set, value_name = "BOOL", value_parser = parse_bool)]
        enable: bool,
    },

    /// Set power-line anti-flicker frequency (`PU 0x05`)
    AntiFlicker {
        #[arg(value_enum)]
        frequency: AntiFlickerArg,
    },

    /// Restore factory defaults (`XU1 0x01` & `XU2 0x02`)
    FactoryReset {
        /// Perform full factory reset and reboot device
        #[arg(long)]
        reboot: bool,
    },

    /// Switch camera USB interface to Read-Only Mass Storage Class (`XU1 0x0f`)
    MscReadonly,

    /// Configure image quality / system feature switch bit (`XU2 0x02`)
    IqSwitch {
        #[arg(long, value_name = "0..31")]
        bit: u8,
        #[arg(action = ArgAction::Set, value_name = "BOOL", value_parser = parse_bool)]
        enable: bool,
        #[arg(long, value_name = "U32")]
        mask: Option<u32>,
    },

    /// Query the hardware optical/digital real zoom value (`XU2 0x0e`)
    RealZoom,

    /// Query fill light parameters (mode, level) (`XU2 0x08`)
    FillLight,
}

pub type SystemCmd = SystemCommand;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExtendFunctionArg {
    #[value(name = "ai-zoom")]
    AiZoom,
    #[value(name = "hdr")]
    Hdr,
    #[value(name = "horizontal-mirror")]
    HorizontalMirror,
    #[value(name = "all-gestures")]
    AllGestures,
    #[value(name = "vertical-mode")]
    VerticalMode,
    #[value(name = "startup-params")]
    StartupParams,
    #[value(name = "single-tap-tracking")]
    SingleTapTracking,
    #[value(name = "auto-tracking")]
    AutoTracking,
    #[value(name = "smart-adjustment")]
    SmartAdjustment,
    #[value(name = "forced-vertical")]
    ForcedVertical,
    #[value(name = "ultimate-privacy")]
    UltimatePrivacy,
    #[value(name = "vertical-mirror")]
    VerticalMirror,
    #[value(name = "low-resolution")]
    LowResolution,
    #[value(name = "track-forbidden-area")]
    TrackForbiddenArea,
}

impl From<ExtendFunctionArg> for ExtendFunction {
    fn from(val: ExtendFunctionArg) -> Self {
        match val {
            ExtendFunctionArg::AiZoom => ExtendFunction::AiZoom,
            ExtendFunctionArg::Hdr => ExtendFunction::Hdr,
            ExtendFunctionArg::HorizontalMirror => ExtendFunction::HorizontalMirror,
            ExtendFunctionArg::AllGestures => ExtendFunction::AllGestures,
            ExtendFunctionArg::VerticalMode => ExtendFunction::VerticalMode,
            ExtendFunctionArg::StartupParams => ExtendFunction::StartupParams,
            ExtendFunctionArg::SingleTapTracking => ExtendFunction::SingleTapTracking,
            ExtendFunctionArg::AutoTracking => ExtendFunction::AutoTracking,
            ExtendFunctionArg::SmartAdjustment => ExtendFunction::SmartAdjustment,
            ExtendFunctionArg::ForcedVertical => ExtendFunction::ForcedVertical,
            ExtendFunctionArg::UltimatePrivacy => ExtendFunction::UltimatePrivacyMode,
            ExtendFunctionArg::VerticalMirror => ExtendFunction::VerticalMirror,
            ExtendFunctionArg::LowResolution => ExtendFunction::LowResolution,
            ExtendFunctionArg::TrackForbiddenArea => ExtendFunction::TrackForbiddenArea,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AntiFlickerArg {
    #[value(name = "disabled", alias = "off")]
    Disabled,
    #[value(name = "50hz", alias = "50")]
    Hz50,
    #[value(name = "60hz", alias = "60")]
    Hz60,
    #[value(name = "auto")]
    Auto,
}

impl From<AntiFlickerArg> for PowerLineFrequency {
    fn from(val: AntiFlickerArg) -> Self {
        match val {
            AntiFlickerArg::Disabled => PowerLineFrequency::Disabled,
            AntiFlickerArg::Hz50 => PowerLineFrequency::Hz50,
            AntiFlickerArg::Hz60 => PowerLineFrequency::Hz60,
            AntiFlickerArg::Auto => PowerLineFrequency::Auto,
        }
    }
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" | "enable" | "enabled" => Ok(true),
        "false" | "0" | "off" | "no" | "disable" | "disabled" => Ok(false),
        other => Err(format!(
            "invalid boolean value '{other}' (expected true/false, on/off, or 1/0)"
        )),
    }
}

enum CliTransport {
    Mock(MockCameraTransport),
    Usb(NusbTransport),
}

impl UvcControlTransport for CliTransport {
    fn camera_info(&self) -> &DiscoveredCamera {
        match self {
            Self::Mock(m) => m.camera_info(),
            Self::Usb(u) => u.camera_info(),
        }
    }

    fn set_cur(&self, unit_id: u8, selector: u8, data: &[u8]) -> Result<(), TransportError> {
        match self {
            Self::Mock(m) => m.set_cur(unit_id, selector, data),
            Self::Usb(u) => u.set_cur(unit_id, selector, data),
        }
    }

    fn get_req(
        &self,
        req: UvcRequest,
        unit_id: u8,
        selector: u8,
        len: usize,
    ) -> Result<Vec<u8>, TransportError> {
        match self {
            Self::Mock(m) => m.get_req(req, unit_id, selector, len),
            Self::Usb(u) => u.get_req(req, unit_id, selector, len),
        }
    }
}

fn open_controller(
    mock: Option<MockModelArg>,
    device: Option<&str>,
) -> Result<Insta360CameraController<CliTransport>, TransportError> {
    if let Some(model_arg) = mock {
        let transport = MockCameraTransport::new(model_arg.into());
        return Ok(Insta360CameraController::new(CliTransport::Mock(transport)));
    }

    if let Some(selector) = device {
        let cameras = list_connected_cameras()?;
        let chosen = if let Some(exact) = cameras.iter().find(|c| c.unique_id == selector) {
            exact.clone()
        } else if let Ok(index) = selector.parse::<usize>() {
            cameras
                .get(index)
                .cloned()
                .ok_or(TransportError::DeviceNotFound)?
        } else {
            let lower = selector.to_ascii_lowercase();
            cameras
                .into_iter()
                .find(|c| {
                    c.unique_id.to_ascii_lowercase().contains(&lower)
                        || c.name.to_ascii_lowercase().contains(&lower)
                        || c.model.internal_code().eq_ignore_ascii_case(&lower)
                })
                .ok_or(TransportError::DeviceNotFound)?
        };
        let transport = NusbTransport::open_by_info(chosen)?;
        Ok(Insta360CameraController::new(CliTransport::Usb(transport)))
    } else {
        let transport = NusbTransport::open_first()?;
        Ok(Insta360CameraController::new(CliTransport::Usb(transport)))
    }
}

fn print_output<T: Serialize>(json: bool, value: &T, human_fn: impl FnOnce(&T)) {
    if json {
        match serde_json::to_string_pretty(value) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("JSON serialization error: {e}"),
        }
    } else {
        human_fn(value);
    }
}

#[derive(Serialize)]
struct CameraListEntry {
    index: usize,
    vendor_id: u16,
    product_id: u16,
    vendor_id_hex: String,
    product_id_hex: String,
    model_name: String,
    internal_code: String,
    camera: DiscoveredCamera,
}

#[derive(Serialize)]
struct FullCameraInfoReport {
    camera: DiscoveredCamera,
    model_display_name: String,
    internal_code: String,
    device_info: DeviceInfo,
    ptz_version: PtzVersionInfo,
    status_bits: u32,
    status_flags: Vec<String>,
    video_mode: VideoMode,
    video_mode_name: String,
    video_mode_aux: VideoModeAuxData,
    extend_functions: Vec<(ExtendFunction, bool)>,
}

#[derive(Serialize)]
struct PtzStatusReport {
    pan_arcsec: i32,
    tilt_arcsec: i32,
    zoom: u16,
    roll: i16,
    ptz_version: PtzVersionInfo,
}

#[derive(Serialize)]
struct ImageStatusReport {
    brightness: i16,
    contrast: u16,
    saturation: u16,
    sharpness: u16,
    auto_wb: bool,
    wb_kelvin: u16,
    auto_exposure: bool,
    iso: u16,
    shutter: u32,
    ev: f32,
    filter: FilterMode,
    hdr: bool,
}

#[derive(Serialize)]
struct AudioStatusReport {
    mode: AudioCaptureMode,
    mode_name: String,
    denoise: bool,
}

#[derive(Serialize)]
struct ActionAck<T: Serialize> {
    ok: bool,
    action: &'static str,
    details: T,
}

fn format_status_flags(status: DeviceStatus) -> Vec<String> {
    let mut flags = Vec::new();
    if status.contains(DeviceStatus::TRACKING_ACTIVE) {
        flags.push("TRACKING_ACTIVE".to_string());
    }
    if status.contains(DeviceStatus::WHITEBOARD_ACTIVE) {
        flags.push("WHITEBOARD_ACTIVE".to_string());
    }
    if status.contains(DeviceStatus::GESTURE_DETECTED) {
        flags.push("GESTURE_DETECTED".to_string());
    }
    if status.contains(DeviceStatus::PRIVACY_SLEEP) {
        flags.push("PRIVACY_SLEEP".to_string());
    }
    if status.contains(DeviceStatus::OVERHEAD_MODE) {
        flags.push("OVERHEAD_MODE".to_string());
    }
    if status.contains(DeviceStatus::DESKVIEW_MODE) {
        flags.push("DESKVIEW_MODE".to_string());
    }
    if status.contains(DeviceStatus::UPGRADING) {
        flags.push("UPGRADING".to_string());
    }
    flags
}

fn query_image_status<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
) -> Result<ImageStatusReport, TransportError> {
    let brightness = ctrl.get_brightness()?;
    let contrast = ctrl.get_contrast()?;
    let saturation = ctrl.get_saturation()?;
    let sharpness = ctrl.get_sharpness()?;
    let auto_wb = ctrl.get_auto_white_balance()?;
    let wb_kelvin = ctrl.get_white_balance_temperature().unwrap_or(5000);
    let auto_exposure = ctrl.get_auto_exposure().unwrap_or(true);
    let iso = ctrl.get_iso().unwrap_or(100);
    let shutter = ctrl.get_shutter_speed().unwrap_or(200);
    let ev = ctrl.get_exposure_compensation().unwrap_or(0.0);
    let filter = ctrl.get_filter().unwrap_or(FilterMode::None);
    let ext = ctrl.get_extend_function_status().unwrap_or_default();
    let hdr = ext
        .into_iter()
        .find(|(f, _)| *f == ExtendFunction::Hdr)
        .map(|(_, en)| en)
        .unwrap_or(false);

    Ok(ImageStatusReport {
        brightness,
        contrast,
        saturation,
        sharpness,
        auto_wb,
        wb_kelvin,
        auto_exposure,
        iso,
        shutter,
        ev,
        filter,
        hdr,
    })
}

fn run(cli: Cli) -> Result<(), TransportError> {
    match cli.command {
        Commands::List => {
            let cameras = if let Some(mock_model) = cli.mock {
                let mock = MockCameraTransport::new(mock_model.into());
                vec![mock.camera_info().clone()]
            } else {
                list_connected_cameras()?
            };

            let entries: Vec<CameraListEntry> = cameras
                .into_iter()
                .enumerate()
                .map(|(index, cam)| CameraListEntry {
                    index,
                    vendor_id: cam.vendor_id,
                    product_id: cam.product_id,
                    vendor_id_hex: format!("0x{:04x}", cam.vendor_id),
                    product_id_hex: format!("0x{:04x}", cam.product_id),
                    model_name: cam.model.display_name().to_string(),
                    internal_code: cam.model.internal_code().to_string(),
                    camera: cam,
                })
                .collect();

            print_output(cli.json, &entries, |list| {
                if list.is_empty() {
                    println!("No Insta360 Link family cameras found.");
                    return;
                }
                for entry in list {
                    let c = &entry.camera;
                    println!(
                        "[{}] {} ({}) - VID:PID {:04x}:{:04x} (internal: {}, bus: {}, addr: {}, id: {})",
                        entry.index,
                        c.name,
                        entry.model_name,
                        c.vendor_id,
                        c.product_id,
                        entry.internal_code,
                        c.bus_number,
                        c.device_address,
                        c.unique_id
                    );
                    println!(
                        "    UVC Units: interface={}, IT={}, PU={}, XU1={}, XU2={}",
                        c.unit_ids.control_interface,
                        c.unit_ids.input_terminal_id,
                        c.unit_ids.processing_unit_id,
                        c.unit_ids.extension_unit_id,
                        c.unit_ids.extension_unit2_id
                    );
                    println!(
                        "    Capabilities: gimbal={}, ptz_drag={}, auto_track={}, auto_framing={}, smart_adj={}, area_track={}, roll={}, templates={}, privacy={}, ai_denoise={}, audio_modes={}, sound_wall={}, horiz_corr={}",
                        c.capabilities.has_physical_gimbal,
                        c.capabilities.support_ptz_drag,
                        c.capabilities.support_auto_track,
                        c.capabilities.support_auto_framing,
                        c.capabilities.support_smart_adjustment,
                        c.capabilities.support_area_track,
                        c.capabilities.support_roll_adjustment,
                        c.capabilities.support_image_template,
                        c.capabilities.support_privacy_mode,
                        c.capabilities.support_ai_denoise,
                        c.capabilities.support_audio_capture_mode,
                        c.capabilities.support_sound_wall,
                        c.capabilities.support_horizontal_correction
                    );
                }
            });
        }

        Commands::Info => {
            let ctrl = open_controller(cli.mock, cli.device.as_deref())?;
            let cam = ctrl.camera_info().clone();
            let device_info = ctrl.get_device_info()?;
            let ptz_version = ctrl.get_ptz_version_and_angles()?;
            let status = ctrl.get_device_status()?;
            let (video_mode, video_mode_aux) = ctrl
                .get_video_mode()
                .unwrap_or((VideoMode::Normal, VideoModeAuxData::default()));
            let extend_functions = ctrl.get_extend_function_status()?;

            let report = FullCameraInfoReport {
                model_display_name: cam.model.display_name().to_string(),
                internal_code: cam.model.internal_code().to_string(),
                camera: cam,
                device_info,
                ptz_version,
                status_bits: status.bits(),
                status_flags: format_status_flags(status),
                video_mode,
                video_mode_name: video_mode.name().to_string(),
                video_mode_aux,
                extend_functions,
            };

            print_output(cli.json, &report, |r| {
                println!(
                    "Camera: {} ({}, VID:PID {:04x}:{:04x})",
                    r.camera.name, r.internal_code, r.camera.vendor_id, r.camera.product_id
                );
                println!("Device Info:");
                println!("  Name:             {}", r.device_info.device_name);
                println!("  Firmware Version: {}", r.device_info.firmware_version);
                println!("  Serial Number:    {}", r.device_info.serial_number);
                println!("PTZ Version Info:");
                println!("  Gimbal Firmware:  {}", r.ptz_version.version);
                println!(
                    "  Pan / Tilt:       {} / {} (0.1 deg)",
                    r.ptz_version.pan, r.ptz_version.tilt
                );
                println!("  Zoom:             {}", r.ptz_version.zoom);
                println!("  Status Byte:      {}", r.ptz_version.status);
                println!(
                    "Device Status:      0x{:08x} ({:?})",
                    r.status_bits, r.status_flags
                );
                println!(
                    "Video Mode:         {:?} ({}) [pitch={}, zoom={}, flags=0x{:02x}]",
                    r.video_mode,
                    r.video_mode_name,
                    r.video_mode_aux.pitch,
                    r.video_mode_aux.zoom,
                    r.video_mode_aux.flags
                );
                println!("Extended Functions:");
                for (func, enabled) in &r.extend_functions {
                    println!("  {:22}: {}", format!("{:?}", func), enabled);
                }
            });
        }

        Commands::Ptz(sub) => {
            let ctrl = open_controller(cli.mock, cli.device.as_deref())?;
            match sub {
                PtzCommand::Get => {
                    let (pan_arcsec, tilt_arcsec) = ctrl.get_pantilt_absolute()?;
                    let zoom = ctrl.get_zoom()?;
                    let roll = ctrl.get_roll()?;
                    let ptz_version = ctrl.get_ptz_version_and_angles()?;
                    let report = PtzStatusReport {
                        pan_arcsec,
                        tilt_arcsec,
                        zoom,
                        roll,
                        ptz_version,
                    };
                    print_output(cli.json, &report, |r| {
                        println!("Pan (arcsec):     {}", r.pan_arcsec);
                        println!("Tilt (arcsec):    {}", r.tilt_arcsec);
                        println!("Zoom:             {}", r.zoom);
                        println!("Roll (0.1 deg):   {}", r.roll);
                        println!(
                            "PTZ Info:         fw={}, pan={}, tilt={}, zoom={}, status={}",
                            r.ptz_version.version,
                            r.ptz_version.pan,
                            r.ptz_version.tilt,
                            r.ptz_version.zoom,
                            r.ptz_version.status
                        );
                    });
                }
                PtzCommand::Set { pan, tilt } => {
                    ctrl.set_pantilt_absolute(pan, tilt)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.set_pantilt_absolute",
                        details: serde_json::json!({ "pan_arcsec": pan, "tilt_arcsec": tilt }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set absolute pan={pan} arcsec, tilt={tilt} arcsec");
                    });
                }
                PtzCommand::Move {
                    pan_speed,
                    pan_dir,
                    tilt_speed,
                    tilt_dir,
                } => {
                    ctrl.set_pantilt_relative_ext(pan_speed, pan_dir, tilt_speed, tilt_dir)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.set_pantilt_relative_ext",
                        details: serde_json::json!({
                            "pan_speed": pan_speed,
                            "pan_dir": pan_dir,
                            "tilt_speed": tilt_speed,
                            "tilt_dir": tilt_dir,
                        }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!(
                            "Moving gimbal: pan(speed={pan_speed}, dir={pan_dir}), tilt(speed={tilt_speed}, dir={tilt_dir})"
                        );
                    });
                }
                PtzCommand::Stop => {
                    ctrl.stop_pantilt_relative()?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.stop_pantilt_relative",
                        details: serde_json::json!({}),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Stopped relative pan/tilt movement");
                    });
                }
                PtzCommand::Zoom { zoom } => {
                    ctrl.set_zoom(zoom)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.set_zoom",
                        details: serde_json::json!({ "zoom": zoom }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set zoom to {zoom}");
                    });
                }
                PtzCommand::Roll { roll } => {
                    ctrl.set_roll(roll)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.set_roll",
                        details: serde_json::json!({ "roll": roll }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set roll to {roll}");
                    });
                }
                PtzCommand::Bias { x, y } => {
                    ctrl.set_composition_bias(x, y)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.set_composition_bias",
                        details: serde_json::json!({ "x": x, "y": y }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set composition bias to x={x}, y={y}");
                    });
                }
                PtzCommand::ResetBias => {
                    ctrl.reset_composition_bias()?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.reset_composition_bias",
                        details: serde_json::json!({ "x": 0x7fff, "y": 0x7fff }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Reset composition bias");
                    });
                }
                PtzCommand::HostCrop {
                    x,
                    y,
                    width,
                    height,
                } => {
                    let info = HostPtzInfo {
                        x,
                        y,
                        width,
                        height,
                    };
                    ctrl.set_host_ptz(&info)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.set_host_ptz",
                        details: info,
                    };
                    print_output(cli.json, &ack, |a| {
                        println!(
                            "Set host PTZ crop: x={}, y={}, width={}, height={}",
                            a.details.x, a.details.y, a.details.width, a.details.height
                        );
                    });
                }
                PtzCommand::Reset => {
                    ctrl.reset_gimbal()?;
                    let ack = ActionAck {
                        ok: true,
                        action: "ptz.reset_gimbal",
                        details: serde_json::json!({}),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Reset gimbal to home position");
                    });
                }
            }
        }

        Commands::Preset(sub) => {
            let ctrl = open_controller(cli.mock, cli.device.as_deref())?;
            match sub {
                PresetCommand::List => {
                    let flags = ctrl.get_preset_flags()?;
                    print_output(cli.json, &flags, |f| {
                        println!(
                            "Presets (current_slot={}, max_slots={}, mask=0b{:08b}):",
                            f.current_slot, f.max_slots, f.slot_mask
                        );
                        for slot in 0..f.max_slots {
                            let occupied = (f.slot_mask & (1 << slot)) != 0;
                            let active = f.current_slot == slot;
                            println!(
                                "  Slot {}: {}{}",
                                slot,
                                if occupied { "saved" } else { "empty" },
                                if active { " [current]" } else { "" }
                            );
                        }
                    });
                }
                PresetCommand::Recall { index } => {
                    ctrl.recall_preset_position(index)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "preset.recall",
                        details: serde_json::json!({ "index": index }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Recalled preset {index}");
                    });
                }
                PresetCommand::Save { index } => {
                    ctrl.add_preset_position(index)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "preset.save",
                        details: serde_json::json!({ "index": index }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Saved preset {index}");
                    });
                }
                PresetCommand::Delete { index } => {
                    ctrl.delete_preset_position(index)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "preset.delete",
                        details: serde_json::json!({ "index": index }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Deleted preset {index}");
                    });
                }
            }
        }

        Commands::Mode(sub) => {
            let ctrl = open_controller(cli.mock, cli.device.as_deref())?;
            match sub {
                ModeCommand::Get => {
                    let (mode, aux) = ctrl.get_video_mode()?;
                    let out = serde_json::json!({
                        "mode": mode,
                        "mode_name": mode.name(),
                        "aux": aux,
                    });
                    print_output(cli.json, &out, |_| {
                        println!(
                            "Video Mode: {:?} ({}) [pitch={}, zoom={}, flags=0x{:02x}]",
                            mode,
                            mode.name(),
                            aux.pitch,
                            aux.zoom,
                            aux.flags
                        );
                    });
                }
                ModeCommand::Set { mode, pitch, zoom } => {
                    let target_mode: VideoMode = mode.into();
                    let (_, mut aux) = ctrl
                        .get_video_mode()
                        .unwrap_or_else(|_| (VideoMode::Normal, VideoModeAuxData::default()));
                    if let Some(p) = pitch {
                        aux.pitch = p;
                    }
                    if let Some(z) = zoom {
                        aux.zoom = z;
                    }
                    ctrl.set_video_mode(target_mode, &aux)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "mode.set",
                        details: serde_json::json!({
                            "mode": target_mode,
                            "mode_name": target_mode.name(),
                            "pitch": aux.pitch,
                            "zoom": aux.zoom,
                        }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!(
                            "Set video mode to {:?} ({}) [pitch={}, zoom={}]",
                            target_mode,
                            target_mode.name(),
                            aux.pitch,
                            aux.zoom
                        );
                    });
                }
            }
        }

        Commands::Tracking(sub) => {
            let ctrl = open_controller(cli.mock, cli.device.as_deref())?;
            match sub {
                TrackingCommand::Speed { speed } => {
                    let s: TrackSpeed = speed.into();
                    ctrl.set_track_speed(s)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "tracking.speed",
                        details: serde_json::json!({ "speed": s }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set tracking speed to {s:?}");
                    });
                }
                TrackingCommand::Style { style } => {
                    let st: CompositionStyle = style.into();
                    ctrl.set_composition_style(st)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "tracking.style",
                        details: serde_json::json!({ "style": st }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set tracking composition style to {st:?}");
                    });
                }
                TrackingCommand::Gestures {
                    all,
                    ok,
                    palm,
                    l,
                    v,
                    rock,
                } => {
                    if let Some(en) = all {
                        ctrl.set_all_gestures_enabled(en)?;
                    }
                    if let Some(en) = ok {
                        ctrl.set_gesture_enabled(Gesture::Ok, en)?;
                    }
                    if let Some(en) = palm {
                        ctrl.set_gesture_enabled(Gesture::Palm, en)?;
                    }
                    if let Some(en) = l {
                        ctrl.set_gesture_enabled(Gesture::LShape, en)?;
                    }
                    if let Some(en) = v {
                        ctrl.set_gesture_enabled(Gesture::VShape, en)?;
                    }
                    if let Some(en) = rock {
                        ctrl.set_gesture_enabled(Gesture::Rock, en)?;
                    }
                    let status = ctrl.get_gesture_status()?;
                    print_output(cli.json, &status, |st| {
                        println!("Gesture Status:");
                        for (g, en) in st {
                            println!("  {:8}: {}", format!("{:?}", g), en);
                        }
                    });
                }
                TrackingCommand::Objects => {
                    let objects = ctrl.get_track_objects()?;
                    print_output(cli.json, &objects, |objs| {
                        println!("Detected Trackable Objects ({}):", objs.len());
                        for obj in objs {
                            println!(
                                "  id={}: x={}, y={}, width={}, height={}",
                                obj.track_id, obj.x, obj.y, obj.width, obj.height
                            );
                        }
                    });
                }
                TrackingCommand::Select {
                    x,
                    y,
                    width,
                    height,
                    id,
                } => {
                    let rect = UvcRect {
                        x,
                        y,
                        width,
                        height,
                        track_id: id,
                    };
                    ctrl.select_track_object(&rect)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "tracking.select",
                        details: rect,
                    };
                    print_output(cli.json, &ack, |a| {
                        println!(
                            "Selected track object id={}: x={}, y={}, width={}, height={}",
                            a.details.track_id,
                            a.details.x,
                            a.details.y,
                            a.details.width,
                            a.details.height
                        );
                    });
                }
                TrackingCommand::GestureBindings => {
                    let map = ctrl.get_gesture_binding_map()?;
                    print_output(cli.json, &map, |entries| {
                        println!("Gesture Bindings:");
                        for (gesture, func) in entries {
                            println!("  {:8}: {:?}", format!("{:?}", gesture), func);
                        }
                    });
                }
                TrackingCommand::BindGesture { gesture, func } => {
                    let g: Gesture = gesture.into();
                    let f: GestureFunction = func.into();
                    ctrl.set_gesture_binding_func(g, f)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "tracking.bind_gesture",
                        details: serde_json::json!({
                            "gesture": g,
                            "func": f,
                        }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Bound gesture {g:?} to {f:?}");
                    });
                }
                TrackingCommand::ForbiddenAreas(sub_area) => match sub_area {
                    ForbiddenAreasCommand::List => {
                        let areas = ctrl.get_track_forbidden_areas()?;
                        print_output(cli.json, &areas, |list| {
                            if list.is_empty() {
                                println!("No tracking forbidden areas configured.");
                                return;
                            }
                            println!("Tracking Forbidden Areas ({}):", list.len());
                            for area in list {
                                println!(
                                    "  Area {}: enabled={}, pan=[{}, {}], tilt=[{}, {}]",
                                    area.index,
                                    area.enabled,
                                    area.pan_min,
                                    area.pan_max,
                                    area.tilt_min,
                                    area.tilt_max
                                );
                            }
                        });
                    }
                    ForbiddenAreasCommand::EnterSetup => {
                        ctrl.enter_track_forbidden_area_setting()?;
                        let ack = ActionAck {
                            ok: true,
                            action: "tracking.forbidden_areas.enter_setup",
                            details: serde_json::json!({}),
                        };
                        print_output(cli.json, &ack, |_| {
                            println!("Entered tracking forbidden area setup mode");
                        });
                    }
                    ForbiddenAreasCommand::ExitSetup => {
                        ctrl.exit_track_forbidden_area_setting()?;
                        let ack = ActionAck {
                            ok: true,
                            action: "tracking.forbidden_areas.exit_setup",
                            details: serde_json::json!({}),
                        };
                        print_output(cli.json, &ack, |_| {
                            println!("Exited tracking forbidden area setup mode");
                        });
                    }
                    ForbiddenAreasCommand::Enable { enable } => {
                        ctrl.set_track_forbidden_area_enabled(enable)?;
                        let ack = ActionAck {
                            ok: true,
                            action: "tracking.forbidden_areas.enable",
                            details: serde_json::json!({ "enabled": enable }),
                        };
                        print_output(cli.json, &ack, |_| {
                            println!("Set tracking forbidden area enabled = {enable}");
                        });
                    }
                    ForbiddenAreasCommand::Set {
                        index,
                        enabled,
                        pan_min,
                        pan_max,
                        tilt_min,
                        tilt_max,
                    } => {
                        let mut areas = ctrl.get_track_forbidden_areas().unwrap_or_default();
                        while areas.len() < 4 {
                            let idx = areas.len() as u8;
                            areas.push(ForbiddenAreaInfo {
                                enabled: false,
                                index: idx,
                                pan_min: 0,
                                pan_max: 0,
                                tilt_min: 0,
                                tilt_max: 0,
                            });
                        }
                        if let Some(target) = areas.iter_mut().find(|a| a.index == index) {
                            target.enabled = enabled;
                            target.pan_min = pan_min;
                            target.pan_max = pan_max;
                            target.tilt_min = tilt_min;
                            target.tilt_max = tilt_max;
                        } else if (index as usize) < areas.len() {
                            areas[index as usize] = ForbiddenAreaInfo {
                                enabled,
                                index,
                                pan_min,
                                pan_max,
                                tilt_min,
                                tilt_max,
                            };
                        }
                        ctrl.set_track_forbidden_areas(&areas)?;
                        let ack = ActionAck {
                            ok: true,
                            action: "tracking.forbidden_areas.set",
                            details: serde_json::json!({
                                "index": index,
                                "enabled": enabled,
                                "pan_min": pan_min,
                                "pan_max": pan_max,
                                "tilt_min": tilt_min,
                                "tilt_max": tilt_max,
                            }),
                        };
                        print_output(cli.json, &ack, |_| {
                            println!(
                                "Updated forbidden area {index}: enabled={enabled}, pan=[{pan_min}, {pan_max}], tilt=[{tilt_min}, {tilt_max}]"
                            );
                        });
                    }
                },
                TrackingCommand::ForceSmartWhiteboard { mode } => {
                    ctrl.force_smart_whiteboard(mode)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "tracking.force_smart_whiteboard",
                        details: serde_json::json!({ "mode": mode }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Forced smart whiteboard mode to {mode}");
                    });
                }
            }
        }

        Commands::Image(sub) => {
            let ctrl = open_controller(cli.mock, cli.device.as_deref())?;
            match sub {
                ImageCommand::Get => {
                    let report = query_image_status(&ctrl)?;
                    print_output(cli.json, &report, |r| {
                        println!("Brightness:     {}", r.brightness);
                        println!("Contrast:       {}", r.contrast);
                        println!("Saturation:     {}", r.saturation);
                        println!("Sharpness:      {}", r.sharpness);
                        println!("Auto WB:        {}", r.auto_wb);
                        println!("WB Kelvin:      {}", r.wb_kelvin);
                        println!("Auto Exposure:  {}", r.auto_exposure);
                        println!("ISO:            {}", r.iso);
                        println!("Shutter:        {}", r.shutter);
                        println!("EV:             {:.2}", r.ev);
                        println!("Filter:         {:?}", r.filter);
                        println!("HDR:            {}", r.hdr);
                    });
                }
                ImageCommand::Set {
                    brightness,
                    contrast,
                    saturation,
                    sharpness,
                    auto_wb,
                    wb_kelvin,
                    auto_exposure,
                    iso,
                    shutter,
                    ev,
                    hdr,
                    filter,
                } => {
                    if let Some(v) = brightness {
                        ctrl.set_brightness(v)?;
                    }
                    if let Some(v) = contrast {
                        ctrl.set_contrast(v)?;
                    }
                    if let Some(v) = saturation {
                        ctrl.set_saturation(v)?;
                    }
                    if let Some(v) = sharpness {
                        ctrl.set_sharpness(v)?;
                    }
                    if let Some(v) = auto_wb {
                        ctrl.set_auto_white_balance(v)?;
                    }
                    if let Some(v) = wb_kelvin {
                        ctrl.set_white_balance_temperature(v)?;
                    }
                    if let Some(v) = auto_exposure {
                        ctrl.set_auto_exposure(v)?;
                    }
                    if let Some(v) = iso {
                        ctrl.set_iso(v)?;
                    }
                    if let Some(v) = shutter {
                        ctrl.set_shutter_speed(v)?;
                    }
                    if let Some(v) = ev {
                        ctrl.set_exposure_compensation(v)?;
                    }
                    if let Some(v) = hdr {
                        ctrl.set_hdr(v)?;
                    }
                    if let Some(v) = filter {
                        ctrl.set_filter(v.into())?;
                    }
                    let report = query_image_status(&ctrl)?;
                    print_output(cli.json, &report, |r| {
                        println!(
                            "Updated image settings: brightness={}, contrast={}, saturation={}, sharpness={}, auto_wb={}, wb={}K, auto_exp={}, iso={}, shutter={}, ev={:.2}, filter={:?}, hdr={}",
                            r.brightness,
                            r.contrast,
                            r.saturation,
                            r.sharpness,
                            r.auto_wb,
                            r.wb_kelvin,
                            r.auto_exposure,
                            r.iso,
                            r.shutter,
                            r.ev,
                            r.filter,
                            r.hdr
                        );
                    });
                }
                ImageCommand::Template(tpl) => match tpl {
                    TemplateCommand::Info => {
                        let info = ctrl.get_image_template_info()?;
                        print_output(cli.json, &info, |i| {
                            println!(
                                "Image Templates: current_id={}, max_templates={}, bitmask=0x{:016x}",
                                i.current_id, i.max_templates, i.bitmask
                            );
                        });
                    }
                    TemplateCommand::Apply { id } => {
                        ctrl.set_image_template(id, ImageOperation::Apply)?;
                        let ack = ActionAck {
                            ok: true,
                            action: "image.template.apply",
                            details: serde_json::json!({ "id": id }),
                        };
                        print_output(cli.json, &ack, |_| {
                            println!("Applied image template {id}");
                        });
                    }
                    TemplateCommand::Save { id, name } => {
                        ctrl.set_image_template(id, ImageOperation::SaveOrUpdate)?;
                        if let Some(ref tpl_name) = name {
                            ctrl.set_image_template_name(id, tpl_name)?;
                        }
                        let ack = ActionAck {
                            ok: true,
                            action: "image.template.save",
                            details: serde_json::json!({ "id": id, "name": name }),
                        };
                        print_output(cli.json, &ack, |_| {
                            println!("Saved image template {id}");
                        });
                    }
                    TemplateCommand::Delete { id } => {
                        ctrl.delete_image_template(id)?;
                        let ack = ActionAck {
                            ok: true,
                            action: "image.template.delete",
                            details: serde_json::json!({ "id": id }),
                        };
                        print_output(cli.json, &ack, |_| {
                            println!("Deleted image template {id}");
                        });
                    }
                },
            }
        }

        Commands::Audio(sub) => {
            let ctrl = open_controller(cli.mock, cli.device.as_deref())?;
            match sub {
                AudioCommand::Get => {
                    let mode = ctrl.get_audio_capture_mode()?;
                    let denoise = ctrl.get_audio_noise_reduction()?;
                    let report = AudioStatusReport {
                        mode,
                        mode_name: mode.name().to_string(),
                        denoise,
                    };
                    print_output(cli.json, &report, |r| {
                        println!("Audio Capture Mode: {:?} ({})", r.mode, r.mode_name);
                        println!("AI Noise Reduction: {}", r.denoise);
                    });
                }
                AudioCommand::Mode { mode } => {
                    let m: AudioCaptureMode = mode.into();
                    ctrl.set_audio_capture_mode(m)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "audio.mode",
                        details: serde_json::json!({ "mode": m, "mode_name": m.name() }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set audio capture mode to {:?} ({})", m, m.name());
                    });
                }
                AudioCommand::Denoise { enable } => {
                    ctrl.set_audio_noise_reduction(enable)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "audio.denoise",
                        details: serde_json::json!({ "denoise": enable }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set audio noise reduction to {enable}");
                    });
                }
            }
        }

        Commands::System(sub) => {
            let ctrl = open_controller(cli.mock, cli.device.as_deref())?;
            match sub {
                SystemCommand::Feature { feature, enable } => {
                    let func: ExtendFunction = feature.into();
                    ctrl.set_extend_function(func, enable)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "system.feature",
                        details: serde_json::json!({ "feature": func, "enabled": enable }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set extended feature {func:?} = {enable}");
                    });
                }
                SystemCommand::Privacy { enable } => {
                    ctrl.set_privacy_mode(enable)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "system.privacy",
                        details: serde_json::json!({ "privacy": enable }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set privacy mode = {enable}");
                    });
                }
                SystemCommand::HorizontalCorrection { enable } => {
                    ctrl.set_horizontal_correction(enable)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "system.horizontal_correction",
                        details: serde_json::json!({ "horizontal_correction": enable }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set horizontal correction = {enable}");
                    });
                }
                SystemCommand::AntiFlicker { frequency } => {
                    let freq: PowerLineFrequency = frequency.into();
                    ctrl.set_power_line_frequency(freq)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "system.anti_flicker",
                        details: serde_json::json!({ "frequency": freq }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set anti-flicker frequency = {freq:?}");
                    });
                }
                SystemCommand::FactoryReset { reboot } => {
                    ctrl.load_factory_default()?;
                    ctrl.restore_factory_settings(reboot)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "system.factory_reset",
                        details: serde_json::json!({ "reboot": reboot }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Executed factory reset (reboot={reboot})");
                    });
                }
                SystemCommand::MscReadonly => {
                    ctrl.switch_msc_readonly_class()?;
                    let ack = ActionAck {
                        ok: true,
                        action: "system.msc_readonly",
                        details: serde_json::json!({}),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Switched camera to MSC read-only USB mode");
                    });
                }
                SystemCommand::IqSwitch { bit, enable, mask } => {
                    let current_mask = mask.unwrap_or(0);
                    ctrl.set_iq_switch(bit, enable, current_mask)?;
                    let ack = ActionAck {
                        ok: true,
                        action: "system.iq_switch",
                        details: serde_json::json!({
                            "bit": bit,
                            "enable": enable,
                            "mask": current_mask,
                        }),
                    };
                    print_output(cli.json, &ack, |_| {
                        println!("Set IQ switch bit {bit} = {enable} (mask=0x{current_mask:08x})");
                    });
                }
                SystemCommand::RealZoom => {
                    let zoom = ctrl.get_real_zoom_value()?;
                    let out = serde_json::json!({ "real_zoom": zoom });
                    print_output(cli.json, &out, |_| {
                        println!("Real Zoom: {zoom}");
                    });
                }
                SystemCommand::FillLight => {
                    let (mode, level) = ctrl.get_fill_light_param()?;
                    let out = serde_json::json!({
                        "mode": mode,
                        "level": level,
                    });
                    print_output(cli.json, &out, |_| {
                        println!("Fill Light: mode={mode}, level={level}");
                    });
                }
            }
        }
    }

    Ok(())
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let json = cli.json;
    if let Err(err) = run(cli) {
        if json {
            let err_obj = serde_json::json!({
                "ok": false,
                "error": err.to_string(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&err_obj).unwrap_or_default()
            );
        } else {
            eprintln!("Error: {err}");
        }
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
