use serde::{Deserialize, Serialize};

/// Arashi Vision / Insta360 USB Vendor ID (`0x2e1a` = 11802)
/// Verified at `MacUVCCameraControlImp::GetUVCCameraList` (`0x100191478`)
/// and `UVCCameraDeviceWatcherImp::StartWatch` (`0x10019e29c`).
pub const INSTA360_VENDOR_ID: u16 = 0x2e1a;

/// Supported USB Product IDs for Insta360 Link family cameras.
/// Verified at `0x100191480` (`0x4c01..=0x4c06`) and `0x100191664` (`0x00c2`),
/// as well as `UVCCameraExtendControllerImp::FirmwareUpgrade` jump table at `0x10286f37d`.
pub mod pids {
    /// Insta360 Link (1st Generation) - `link1`, firmware `a:\Insta360WebCamFW.bin`
    pub const LINK_1: u16 = 0x4c01;
    /// Insta360 PUC3 Gimbal - firmware `a:\Insta360PUC3FW_GIMBAL.bin`
    pub const PUC3_GIMBAL_A: u16 = 0x4c02;
    /// Insta360 Link 2C (Fixed-lens 4K webcam) - `link2_main`, firmware `a:\Insta360LINK2CFW_HOST.bin`
    pub const LINK_2C: u16 = 0x4c03;
    /// Insta360 Link 2 (2nd Gen 2-axis gimbal 4K webcam) - `link2_gimbal`, firmware `a:\Insta360LINK2FW_GIMBAL.bin`
    pub const LINK_2: u16 = 0x4c04;
    /// Insta360 PUC3 Host - firmware `a:\Insta360PUC3FW_HOST.bin`
    pub const PUC3_HOST: u16 = 0x4c05;
    /// Insta360 PUC3 Gimbal Variant - firmware `a:\Insta360PUC3FW_GIMBAL.bin`
    pub const PUC3_GIMBAL_B: u16 = 0x4c06;
    /// Insta360 Link Simple-Class / Firmware Upgrade Mode
    pub const LINK_SIMPLE_CLASS: u16 = 0x00c2;
}

/// Extension Unit 1 GUID: `FAF1672D-B71B-4793-8C91-7B1C9B7F95F8`
/// Verified at CFString `0x102a9d098` and `FindControllerInterfaceForServiceObject` (`0x100191140`).
/// Mixed-endian USB UVC `guidExtensionCode` 16-byte representation:
/// `2d 67 f1 fa 1b b7 93 47 8c 91 7b 1c 9b 7f 95 f8`
pub const XU1_GUID_STR: &str = "FAF1672D-B71B-4793-8C91-7B1C9B7F95F8";
pub const XU1_GUID_BYTES: [u8; 16] = [
    0x2d, 0x67, 0xf1, 0xfa, 0x1b, 0xb7, 0x93, 0x47,
    0x8c, 0x91, 0x7b, 0x1c, 0x9b, 0x7f, 0x95, 0xf8,
];

/// Extension Unit 2 GUID: `E307E649-4618-A3FF-82FC-2D8B5F216773`
/// Verified at CFString `0x102a9d0b8` and `FindControllerInterfaceForServiceObject` (`0x100191168`).
/// Mixed-endian USB UVC `guidExtensionCode` 16-byte representation:
/// `49 e6 07 e3 18 46 ff a3 82 fc 2d 8b 5f 21 67 73`
pub const XU2_GUID_STR: &str = "E307E649-4618-A3FF-82FC-2D8B5F216773";
pub const XU2_GUID_BYTES: [u8; 16] = [
    0x49, 0xe6, 0x07, 0xe3, 0x18, 0x46, 0xff, 0xa3,
    0x82, 0xfc, 0x2d, 0x8b, 0x5f, 0x21, 0x67, 0x73,
];

/// Default UVC unit IDs observed on Insta360 Link family cameras when descriptors are not manually overridden.
pub const DEFAULT_CONTROL_INTERFACE: u8 = 0;
pub const DEFAULT_INPUT_TERMINAL_ID: u8 = 1;
pub const DEFAULT_PROCESSING_UNIT_ID: u8 = 2;
pub const DEFAULT_EXTENSION_UNIT_ID: u8 = 9;
pub const DEFAULT_EXTENSION_UNIT2_ID: u8 = 10;

/// Identified Insta360 Link camera model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CameraModel {
    /// Insta360 Link (`0x4c01`)
    Link,
    /// Insta360 Link 2 (`0x4c04`)
    Link2,
    /// Insta360 Link 2C (`0x4c03`, gimbal-less 4K)
    Link2C,
    /// Insta360 PUC3 Gimbal (`0x4c02` / `0x4c06`)
    Puc3Gimbal(u16),
    /// Insta360 PUC3 Host (`0x4c05`)
    Puc3Host,
    /// Insta360 Link Simple-Class / DFU (`0x00c2`)
    LinkSimpleClass,
    /// Unknown Insta360 PID
    Unknown(u16),
}

impl CameraModel {
    pub fn from_pid(pid: u16) -> Self {
        match pid {
            pids::LINK_1 => Self::Link,
            pids::LINK_2C => Self::Link2C,
            pids::LINK_2 => Self::Link2,
            pids::PUC3_GIMBAL_A | pids::PUC3_GIMBAL_B => Self::Puc3Gimbal(pid),
            pids::PUC3_HOST => Self::Puc3Host,
            pids::LINK_SIMPLE_CLASS => Self::LinkSimpleClass,
            other => Self::Unknown(other),
        }
    }

    pub fn pid(&self) -> u16 {
        match *self {
            Self::Link => pids::LINK_1,
            Self::Link2C => pids::LINK_2C,
            Self::Link2 => pids::LINK_2,
            Self::Puc3Gimbal(pid) => pid,
            Self::Puc3Host => pids::PUC3_HOST,
            Self::LinkSimpleClass => pids::LINK_SIMPLE_CLASS,
            Self::Unknown(pid) => pid,
        }
    }

    /// Returns the internal model identifier used by `Webcam::CameraInsta::GetCameraModel()` (`0x10005f92c`).
    pub fn internal_code(&self) -> &'static str {
        match self {
            Self::Link | Self::LinkSimpleClass => "link1",
            Self::Link2C | Self::Puc3Host => "link2_main",
            Self::Link2 | Self::Puc3Gimbal(_) => "link2_gimbal",
            Self::Unknown(_) => "unknown",
        }
    }

    /// Returns the human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Link => "Insta360 Link",
            Self::Link2 => "Insta360 Link 2",
            Self::Link2C => "Insta360 Link 2C",
            Self::Puc3Gimbal(_) => "Insta360 Link 2 Pro (Gimbal)",
            Self::Puc3Host => "Insta360 Link 2C Pro (Host)",
            Self::LinkSimpleClass => "Insta360 Link (Firmware Mode)",
            Self::Unknown(_) => "Insta360 UVC Camera",
        }
    }

    /// Returns the target firmware filename on the camera's virtual MSC drive (`0x1001a0a0c`).
    pub fn firmware_filename(&self) -> Option<&'static str> {
        match self {
            Self::Link | Self::LinkSimpleClass => Some(r"a:\Insta360WebCamFW.bin"),
            Self::Link2 => Some(r"a:\Insta360LINK2FW_GIMBAL.bin"),
            Self::Link2C => Some(r"a:\Insta360LINK2CFW_HOST.bin"),
            Self::Puc3Gimbal(_) => Some(r"a:\Insta360PUC3FW_GIMBAL.bin"),
            Self::Puc3Host => Some(r"a:\Insta360PUC3FW_HOST.bin"),
            Self::Unknown(_) => None,
        }
    }

    /// Whether this camera uses 36-byte extended `SetVideoMode` payloads (`0x1001970f4`: bitmask `0x2b` for `pid - 0x4c01`).
    /// Wait: in `SetVideoMode` (`0x1001970f4`), if `len == 0x24` and `((1 << (pid - 0x4c01)) & 0x2b) != 0`
    /// (`0x4c01`, `0x4c02`, `0x4c04`, `0x4c06` = bits 0, 1, 3, 5 = `0b101011` = `0x2b`),
    /// it sends 8 bytes (`w4 = 8`) instead of 36 bytes (`0x24`), whereas `0x4c03` (Link 2C) and `0x4c05` (PUC3 Host)
    /// send the full 36-byte `SetVideoMode` buffer!
    pub fn video_mode_payload_len(&self) -> usize {
        let pid = self.pid();
        if (0x4c01..=0x4c06).contains(&pid) {
            let bit = 1u32 << (pid - 0x4c01);
            if (bit & 0x2b) != 0 {
                return 8;
            }
        }
        36
    }
}

/// Hardware capability matrix derived from `Webcam::CameraDeviceSupporter::UpdateUIStatus` (`0x10004a8b8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub has_physical_gimbal: bool,
    pub support_ptz_drag: bool,
    pub support_auto_framing: bool,
    pub support_highlighting: bool,
    pub support_mirror_vertically: bool,
    pub support_track_speed: bool,
    pub support_image_template: bool,
    pub support_privacy_mode: bool,
    pub support_area_track: bool,
    pub support_roll_adjustment: bool,
    pub support_ai_denoise: bool,
    pub support_sound_wall: bool,
    pub support_speak_tracking: bool,
    pub support_auto_track: bool,
    pub support_smart_adjustment: bool,
    pub support_audio_capture_mode: bool,
    pub support_horizontal_correction: bool,
}

impl DeviceCapabilities {
    pub fn for_pid(pid: u16) -> Self {
        let is_not_link2c_or_host = pid != pids::LINK_2C && pid != pids::PUC3_HOST;
        let is_not_link1 = pid != pids::LINK_1;
        let is_link2_or_puc3_gimbal = (pid & 0xfffd) == pids::LINK_2; // 0x4c04 or 0x4c06
        let is_puc3 = pid == pids::PUC3_HOST || pid == pids::PUC3_GIMBAL_B;

        Self {
            has_physical_gimbal: is_not_link2c_or_host,
            support_ptz_drag: is_not_link2c_or_host,
            support_auto_framing: is_not_link1,
            support_highlighting: is_not_link1,
            support_mirror_vertically: is_not_link1,
            support_track_speed: is_not_link2c_or_host,
            support_image_template: is_not_link1,
            support_privacy_mode: is_not_link1,
            support_area_track: is_link2_or_puc3_gimbal,
            support_roll_adjustment: pid == pids::LINK_1,
            support_ai_denoise: is_not_link1,
            support_sound_wall: is_puc3,
            support_speak_tracking: is_link2_or_puc3_gimbal,
            support_auto_track: is_not_link2c_or_host,
            support_smart_adjustment: is_not_link2c_or_host,
            support_audio_capture_mode: is_not_link1,
            support_horizontal_correction: pid == pids::LINK_2,
        }
    }
}

/// Discovered UVC unit IDs and interface number for an Insta360 Link camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UvcUnitIds {
    pub control_interface: u8,
    pub input_terminal_id: u8,
    pub processing_unit_id: u8,
    pub extension_unit_id: u8,
    pub extension_unit2_id: u8,
}

impl Default for UvcUnitIds {
    fn default() -> Self {
        Self {
            control_interface: DEFAULT_CONTROL_INTERFACE,
            input_terminal_id: DEFAULT_INPUT_TERMINAL_ID,
            processing_unit_id: DEFAULT_PROCESSING_UNIT_ID,
            extension_unit_id: DEFAULT_EXTENSION_UNIT_ID,
            extension_unit2_id: DEFAULT_EXTENSION_UNIT2_ID,
        }
    }
}

/// Parses raw USB configuration descriptor bytes to extract UVC Input Terminal, Processing Unit,
/// and Insta360 Extension Unit 1 & 2 IDs (`FindControllerInterfaceForServiceObject` at `0x100190fe0`).
pub fn parse_uvc_vc_descriptors(config_desc: &[u8]) -> UvcUnitIds {
    let mut ids = UvcUnitIds::default();
    let mut offset = 0usize;
    let mut in_vc_interface = false;

    while offset + 2 <= config_desc.len() {
        let b_length = config_desc[offset] as usize;
        let b_descriptor_type = config_desc[offset + 1];
        if b_length < 2 || offset + b_length > config_desc.len() {
            break;
        }
        let desc = &config_desc[offset..offset + b_length];

        // USB_DT_INTERFACE = 0x04
        if b_descriptor_type == 0x04 && desc.len() >= 9 {
            let b_interface_number = desc[2];
            let b_interface_class = desc[5];
            let b_interface_subclass = desc[6];
            // CC_VIDEO = 0x0e, SC_VIDEOCONTROL = 0x01
            if b_interface_class == 0x0e && b_interface_subclass == 0x01 {
                in_vc_interface = true;
                ids.control_interface = b_interface_number;
            } else {
                in_vc_interface = false;
            }
        } else if in_vc_interface && b_descriptor_type == 0x24 && desc.len() >= 4 {
            // CS_INTERFACE = 0x24
            let b_descriptor_subtype = desc[2];
            let unit_or_terminal_id = desc[3];
            match b_descriptor_subtype {
                // VC_INPUT_TERMINAL = 0x02
                0x02 if desc.len() >= 6 => {
                    let w_terminal_type = u16::from_le_bytes([desc[4], desc[5]]);
                    // ITT_CAMERA = 0x0201
                    if w_terminal_type == 0x0201 || ids.input_terminal_id == DEFAULT_INPUT_TERMINAL_ID {
                        ids.input_terminal_id = unit_or_terminal_id;
                    }
                }
                // VC_PROCESSING_UNIT = 0x05
                0x05 => {
                    ids.processing_unit_id = unit_or_terminal_id;
                }
                // VC_EXTENSION_UNIT = 0x06
                0x06 if desc.len() >= 20 => {
                    let guid = &desc[4..20];
                    if guid == XU1_GUID_BYTES {
                        ids.extension_unit_id = unit_or_terminal_id;
                    } else if guid == XU2_GUID_BYTES {
                        ids.extension_unit2_id = unit_or_terminal_id;
                    }
                }
                _ => {}
            }
        }

        offset += b_length;
    }

    ids
}

/// Returns true if a `(vendor_id, product_id)` pair is an Insta360 Link family device.
pub fn is_insta360_link_device(vid: u16, pid: u16) -> bool {
    vid == INSTA360_VENDOR_ID
        && ((pids::LINK_1..=pids::PUC3_GIMBAL_B).contains(&pid) || pid == pids::LINK_SIMPLE_CLASS)
}
