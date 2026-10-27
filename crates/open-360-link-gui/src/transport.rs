#![allow(dead_code)]

use open_360_link_protocol::{
    ct_selectors, pu_selectors, xu1_selectors, xu2_selectors, CameraModel, DiscoveredCamera,
    MockCameraTransport, NusbTransport, TransportError, UvcControlPacketHeader,
    UvcControlTransport, UvcRequest, UvcUnitIds,
};

/// Unified GUI transport wrapper allowing runtime switching between physical USB cameras
/// (`NusbTransport`) and stateful in-memory camera simulators (`MockCameraTransport`).
pub enum GuiTransport {
    Usb(NusbTransport),
    Mock(MockCameraTransport),
}

impl GuiTransport {
    pub fn is_mock(&self) -> bool {
        matches!(self, Self::Mock(_))
    }

    pub fn command_log(&self) -> Option<Vec<(UvcRequest, u8, u8, Vec<u8>)>> {
        match self {
            Self::Usb(_) => None,
            Self::Mock(mock) => Some(mock.command_log()),
        }
    }
}

impl UvcControlTransport for GuiTransport {
    fn camera_info(&self) -> &DiscoveredCamera {
        match self {
            Self::Usb(usb) => usb.camera_info(),
            Self::Mock(mock) => mock.camera_info(),
        }
    }

    fn set_cur(&self, unit_id: u8, selector: u8, data: &[u8]) -> Result<(), TransportError> {
        match self {
            Self::Usb(usb) => usb.set_cur(unit_id, selector, data),
            Self::Mock(mock) => mock.set_cur(unit_id, selector, data),
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
            Self::Usb(usb) => usb.get_req(req, unit_id, selector, len),
            Self::Mock(mock) => mock.get_req(req, unit_id, selector, len),
        }
    }
}

/// Target device selection in the GUI header dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSelection {
    Usb(DiscoveredCamera),
    Simulate(CameraModel),
}

impl DeviceSelection {
    pub fn label(&self) -> String {
        match self {
            Self::Usb(cam) => format!(
                "USB: {} (Bus {} Addr {} • 0x{:04x})",
                cam.name, cam.bus_number, cam.device_address, cam.product_id
            ),
            Self::Simulate(model) => format!(
                "Simulate: {} (0x{:04x})",
                model.display_name(),
                model.pid()
            ),
        }
    }
}

/// Formats a human-readable UVC unit name for the protocol inspector.
pub fn describe_unit(unit_id: u8, ids: &UvcUnitIds) -> &'static str {
    if unit_id == ids.input_terminal_id {
        "IT (Camera Terminal)"
    } else if unit_id == ids.processing_unit_id {
        "PU (Processing Unit)"
    } else if unit_id == ids.extension_unit_id {
        "XU1 (Insta360 Ext 1)"
    } else if unit_id == ids.extension_unit2_id {
        "XU2 (Insta360 Ext 2)"
    } else {
        "Unknown Unit"
    }
}

/// Formats a human-readable UVC control selector symbol for the protocol inspector.
pub fn describe_selector(unit_id: u8, selector: u8, ids: &UvcUnitIds) -> &'static str {
    if unit_id == ids.input_terminal_id {
        match selector {
            ct_selectors::AE_MODE => "CT_AE_MODE_CONTROL",
            ct_selectors::EXPOSURE_TIME_ABSOLUTE => "CT_EXPOSURE_TIME_ABSOLUTE",
            ct_selectors::FOCUS_ABSOLUTE => "CT_FOCUS_ABSOLUTE",
            ct_selectors::FOCUS_AUTO => "CT_FOCUS_AUTO",
            ct_selectors::ZOOM_ABSOLUTE => "CT_ZOOM_ABSOLUTE",
            ct_selectors::PANTILT_ABSOLUTE => "CT_PANTILT_ABSOLUTE",
            ct_selectors::PANTILT_RELATIVE => "CT_PANTILT_RELATIVE",
            ct_selectors::ROLL_ABSOLUTE => "CT_ROLL_ABSOLUTE",
            _ => "CT_UNKNOWN",
        }
    } else if unit_id == ids.processing_unit_id {
        match selector {
            pu_selectors::BRIGHTNESS => "PU_BRIGHTNESS_CONTROL",
            pu_selectors::CONTRAST => "PU_CONTRAST_CONTROL",
            pu_selectors::POWER_LINE_FREQUENCY => "PU_POWER_LINE_FREQUENCY",
            pu_selectors::SATURATION => "PU_SATURATION_CONTROL",
            pu_selectors::SHARPNESS => "PU_SHARPNESS_CONTROL",
            pu_selectors::WHITE_BALANCE_TEMPERATURE => "PU_WB_TEMPERATURE",
            pu_selectors::WHITE_BALANCE_TEMPERATURE_AUTO => "PU_WB_TEMPERATURE_AUTO",
            _ => "PU_UNKNOWN",
        }
    } else if unit_id == ids.extension_unit_id {
        match selector {
            xu1_selectors::SYSTEM_AND_PRESET_CMD => "XU1_SYSTEM_AND_PRESET_CMD",
            xu1_selectors::VIDEO_MODE => "XU1_VIDEO_MODE",
            xu1_selectors::TRACKING_AND_GESTURE => "XU1_TRACKING_AND_GESTURE",
            xu1_selectors::AUDIO_CONTROL => "XU1_AUDIO_CONTROL",
            xu1_selectors::TRACK_OBJECT_SELECT => "XU1_TRACK_OBJECT_SELECT",
            xu1_selectors::TRACK_OBJECT_LIST_V1 => "XU1_TRACK_OBJECT_LIST_V1",
            xu1_selectors::EXPOSURE_CURVE_AND_ISO => "XU1_EXPOSURE_CURVE_AND_ISO",
            xu1_selectors::EXPOSURE_COMPENSATION => "XU1_EXPOSURE_COMPENSATION",
            xu1_selectors::WHITE_BALANCE_EXT => "XU1_WHITE_BALANCE_EXT",
            xu1_selectors::EXTEND_FUNC_BITMASK => "XU1_EXTEND_FUNC_BITMASK",
            xu1_selectors::PANTILT_RELATIVE_EXT => "XU1_PANTILT_RELATIVE_EXT",
            xu1_selectors::DEVICE_STATUS => "XU1_DEVICE_STATUS",
            xu1_selectors::PTZ_VERSION_AND_ABS => "XU1_PTZ_VERSION_AND_ABS",
            xu1_selectors::PRESET_POSITION_FLAGS => "XU1_PRESET_POSITION_FLAGS",
            xu1_selectors::USB_MODE_SWITCH => "XU1_USB_MODE_SWITCH",
            _ => "XU1_UNKNOWN",
        }
    } else if unit_id == ids.extension_unit2_id {
        match selector {
            xu2_selectors::GESTURE_BINDING_MAP => "XU2_GESTURE_BINDING_MAP",
            xu2_selectors::SYSTEM_TIME_AND_IQ => "XU2_SYSTEM_TIME_AND_IQ",
            xu2_selectors::DEVICE_INFO => "XU2_DEVICE_INFO",
            xu2_selectors::IMAGE_TEMPLATE_INFO => "XU2_IMAGE_TEMPLATE_INFO",
            xu2_selectors::IMAGE_TEMPLATE_NAME => "XU2_IMAGE_TEMPLATE_NAME",
            xu2_selectors::FILTER_MODE => "XU2_FILTER_MODE",
            xu2_selectors::PRIVACY_AND_FILL_LIGHT => "XU2_PRIVACY_AND_FILL_LIGHT",
            xu2_selectors::HOST_PTZ => "XU2_HOST_PTZ",
            xu2_selectors::TRACK_OBJECT_LIST_V2 => "XU2_TRACK_OBJECT_LIST_V2",
            xu2_selectors::TRACK_FORBIDDEN_AREA => "XU2_TRACK_FORBIDDEN_AREA",
            xu2_selectors::DEFAULT_TRACK_MODE => "XU2_DEFAULT_TRACK_MODE",
            xu2_selectors::HORIZONTAL_CORRECTION => "XU2_HORIZONTAL_CORRECTION",
            xu2_selectors::REAL_ZOOM_AND_UDISK => "XU2_REAL_ZOOM_AND_UDISK",
            xu2_selectors::REPORT_DATA => "XU2_REPORT_DATA",
            _ => "XU2_UNKNOWN",
        }
    } else {
        "UNKNOWN_SELECTOR"
    }
}

/// Formats a byte slice as space-separated lowercase hex.
pub fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Computes the UVC setup packet header for display in the Protocol Inspector.
pub fn packet_header(
    req: UvcRequest,
    unit_id: u8,
    selector: u8,
    ids: &UvcUnitIds,
) -> UvcControlPacketHeader {
    UvcControlPacketHeader::new(req, selector, unit_id, ids.control_interface)
}
