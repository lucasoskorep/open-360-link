use crate::types::*;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum CodecError {
    #[error("payload too short: expected at least {expected} bytes, got {actual}")]
    PayloadTooShort { expected: usize, actual: usize },
    #[error("invalid argument: {0}")]
    InvalidArgument(&'static str),
}

/// Encodes UVC `bmRequestType`, `bRequest`, `wValue`, `wIndex` for a UVC control transfer
/// matching `MacUVCCameraControl::SetData` (`0x100192880`) and `GetData` (`0x10019282c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UvcControlPacketHeader {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
}

impl UvcControlPacketHeader {
    pub fn new(req: UvcRequest, control_selector: u8, unit_or_terminal_id: u8, interface_num: u8) -> Self {
        let bm_request_type = match req {
            UvcRequest::SetCur => 0x21, // Host-to-device, Class, Interface (`0x0121` in little-endian u16 with bRequest=0x01)
            _ => 0xa1,                  // Device-to-host, Class, Interface
        };
        Self {
            bm_request_type,
            b_request: req as u8,
            w_value: (control_selector as u16) << 8,
            w_index: ((unit_or_terminal_id as u16) << 8) | (interface_num as u16),
        }
    }
}

// ============================================================================
// XU1 Selector 0x01: System, Presets, Gimbal Reset, Composition Bias
// ============================================================================

/// `SetCurPresetPosition(unsigned char)` (`0x100199918`): `[0, index, 0, 0]`
pub fn encode_set_cur_preset_position(index: u8) -> [u8; 4] {
    [0x00, index, 0x00, 0x00]
}

/// `AddPresetPosition(unsigned char)` (`0x1001999b4`): `[1, index, 0, 0]`
pub fn encode_add_preset_position(index: u8) -> [u8; 4] {
    [0x01, index, 0x00, 0x00]
}

/// `DeletePresetPosition(unsigned char)` (`0x100199a54`): `[2, index, 0, 0]`
pub fn encode_delete_preset_position(index: u8) -> [u8; 4] {
    [0x02, index, 0x00, 0x00]
}

/// `ResetGimbal()` (`0x100196fb0`): `[3, 0, 0, 0]`
pub fn encode_reset_gimbal() -> [u8; 4] {
    [0x03, 0x00, 0x00, 0x00]
}

/// `LoadFactoryDefault()` (`0x10019701c`): `[4, 0, 0, 0]`
pub fn encode_load_factory_default() -> [u8; 4] {
    [0x04, 0x00, 0x00, 0x00]
}

/// `SetCompositionBias(short, short)` (`0x10019933c`): `[6, x_lo, x_hi, y_lo, y_hi]`
pub fn encode_set_composition_bias(x: i16, y: i16) -> [u8; 5] {
    let xb = x.to_le_bytes();
    let yb = y.to_le_bytes();
    [0x06, xb[0], xb[1], yb[0], yb[1]]
}

/// `EnterTrackForbiddenAreaSetting()` (`0x10019bf74`): `[7, 1, 0, 0]`
pub fn encode_enter_track_forbidden_area_setting() -> [u8; 4] {
    [0x07, 0x01, 0x00, 0x00]
}

/// `ExitTrackForbiddenAreaSetting()` (`0x10019bfb4`): `[8, 1, 0, 0]`
pub fn encode_exit_track_forbidden_area_setting() -> [u8; 4] {
    [0x08, 0x01, 0x00, 0x00]
}

// ============================================================================
// XU1 Selector 0x02: Video Mode & Auxiliary Data (`0x100197088`, `0x1001978b8`)
// ============================================================================

pub fn encode_set_video_mode(mode: VideoMode, aux: &VideoModeAuxData, payload_len: usize) -> Vec<u8> {
    let len = payload_len.clamp(8, 36);
    let mut buf = vec![0u8; 36];
    buf[0] = mode as u8;
    buf[1] = aux.flags;
    for (i, &val) in aux.rect.iter().enumerate() {
        let off = 2 + i * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    }
    buf[18..20].copy_from_slice(&aux.pitch.to_le_bytes());
    buf[20..22].copy_from_slice(&aux.zoom.to_le_bytes());
    buf[22..36].copy_from_slice(&aux.extra);
    if len == 8 && aux.rect == [0, 0, 0, 0] {
        buf[2..4].copy_from_slice(&aux.pitch.to_le_bytes());
        buf[4..6].copy_from_slice(&aux.zoom.to_le_bytes());
    }
    buf.truncate(len);
    buf
}

pub fn decode_get_video_mode(data: &[u8]) -> Result<(VideoMode, VideoModeAuxData), CodecError> {
    if data.len() < 8 {
        return Err(CodecError::PayloadTooShort {
            expected: 8,
            actual: data.len(),
        });
    }
    if data[0] == 0xff {
        return Err(CodecError::InvalidArgument(
            "camera video mode is transitioning (0xff)",
        ));
    }
    let mode = VideoMode::from_u8(data[0]);
    let mut aux = VideoModeAuxData {
        flags: data[1],
        ..Default::default()
    };
    if data.len() >= 22 {
        for i in 0..4 {
            let off = 2 + i * 4;
            aux.rect[i] = i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        }
        aux.pitch = i16::from_le_bytes([data[18], data[19]]);
        aux.zoom = u16::from_le_bytes([data[20], data[21]]);
    }
    if data.len() >= 36 {
        aux.extra.copy_from_slice(&data[22..36]);
    }
    Ok((mode, aux))
}

// ============================================================================
// XU1 Selector 0x03: Tracking Speed, Composition Style, Gestures, Smart Whiteboard
// ============================================================================

/// `SetCompositionStyle` (`0x100197498`): 16-byte buffer `[6, style_u8, 0..]`
pub fn encode_set_composition_style(style: CompositionStyle) -> [u8; 16] {
    let mut buf = [0u8; 16];
    buf[0] = 0x06;
    buf[1] = style as u8;
    buf
}

/// `GetCompositionStyle` (`0x10019750c`): reads `data[7]` from 16-byte response
pub fn decode_get_composition_style(data: &[u8]) -> Result<CompositionStyle, CodecError> {
    if data.len() < 8 {
        return Err(CodecError::PayloadTooShort {
            expected: 8,
            actual: data.len(),
        });
    }
    Ok(CompositionStyle::from_u8(data[7]))
}

/// `SetTrackSpeed` (`0x1001975bc`): 16-byte buffer `[5, speed_u8, 0..]`
pub fn encode_set_track_speed(speed: TrackSpeed) -> [u8; 16] {
    let mut buf = [0u8; 16];
    buf[0] = 0x05;
    buf[1] = speed as u8;
    buf
}

/// `GetTrackSpeed` (`0x100197630`): reads `data[6]` from 16-byte response
pub fn decode_get_track_speed(data: &[u8]) -> Result<TrackSpeed, CodecError> {
    if data.len() < 7 {
        return Err(CodecError::PayloadTooShort {
            expected: 7,
            actual: data.len(),
        });
    }
    Ok(TrackSpeed::from_u8(data[6]))
}

/// `ForceSetSmatrWhiteBoard` (`0x1001980d0`): 16-byte buffer `[3, mode_u8, 0..]`
pub fn encode_force_smart_whiteboard(mode: u8) -> [u8; 16] {
    let mut buf = [0u8; 16];
    buf[0] = 0x03;
    buf[1] = mode;
    buf
}

/// `EnableAllGesture` (`0x100197b54`): 16-byte buffer `[0, enable_u8, 0..]`
pub fn encode_enable_all_gestures(enable: bool) -> [u8; 16] {
    let mut buf = [0u8; 16];
    buf[0] = 0x00;
    buf[1] = u8::from(enable);
    buf
}

/// `EnableGestureWork` (`0x100197bc8`):
/// Reads 6 bytes `[all, ok, palm, l, v, rock]` from selector `0x03`, updates `data[gesture + 1]`,
/// and writes `[data[0], data[1], data[2], data[3], data[4], data[5], 0..]` (16B).
pub fn encode_enable_gesture_work(current_6b: &[u8], gesture: Gesture, enable: bool) -> [u8; 16] {
    let mut buf = [0u8; 16];
    for i in 0..current_6b.len().min(6) {
        buf[i] = current_6b[i];
    }
    buf[0] = 0x01; // master gesture enable
    let idx = (gesture as usize) + 1;
    if idx < 6 {
        buf[idx] = u8::from(enable);
    }
    buf
}

/// `GetGestureStatus` (`0x100198144`): decodes `[all, ok, palm, l, v, rock]` from selector `0x03`
pub fn decode_gesture_status(data: &[u8]) -> Result<[(Gesture, bool); 5], CodecError> {
    if data.len() < 6 {
        return Err(CodecError::PayloadTooShort {
            expected: 6,
            actual: data.len(),
        });
    }
    Ok([
        (Gesture::Ok, data[1] != 0),
        (Gesture::Palm, data[2] != 0),
        (Gesture::LShape, data[3] != 0),
        (Gesture::VShape, data[4] != 0),
        (Gesture::Rock, data[5] != 0),
    ])
}

// ============================================================================
// XU1 Selector 0x04: Audio Noise Reduction & Audio Capture Mode (`0x100197798`, `0x10019bf54`)
// ============================================================================

pub fn encode_audio_control(value: u8) -> [u8; 2] {
    [value, 0x00]
}

// ============================================================================
// XU1 Selector 0x05 & 0x06 / XU2 Selector 0x0a: Track Object Rectangles (`0x100198a38`)
// ============================================================================

pub fn encode_track_rect(rect: &UvcRect) -> [u8; 10] {
    let mut buf = [0u8; 10];
    buf[0..2].copy_from_slice(&rect.x.to_le_bytes());
    buf[2..4].copy_from_slice(&rect.y.to_le_bytes());
    buf[4..6].copy_from_slice(&rect.width.to_le_bytes());
    buf[6..8].copy_from_slice(&rect.height.to_le_bytes());
    buf[8..10].copy_from_slice(&rect.track_id.to_le_bytes());
    buf
}

pub fn decode_track_rect_list(data: &[u8]) -> Vec<UvcRect> {
    let mut rects = Vec::new();
    for chunk in data.chunks_exact(10) {
        let x = i16::from_le_bytes([chunk[0], chunk[1]]);
        let y = i16::from_le_bytes([chunk[2], chunk[3]]);
        let width = i16::from_le_bytes([chunk[4], chunk[5]]);
        let height = i16::from_le_bytes([chunk[6], chunk[7]]);
        let track_id = i16::from_le_bytes([chunk[8], chunk[9]]);
        if width > 0 || height > 0 {
            rects.push(UvcRect {
                x,
                y,
                width,
                height,
                track_id,
            });
        }
    }
    rects
}

// ============================================================================
// XU1 Selector 0x07: Exposure Curve, Auto Exposure, ISO (`0x10019aaa0`–`0x10019b7d0`)
// ============================================================================

pub fn encode_exposure_curve(curve: &ExposureCurve) -> [u8; 259] {
    let mut buf = [0u8; 259];
    buf[0] = curve.mode;
    buf[1..3].copy_from_slice(&curve.iso.to_le_bytes());
    for (i, &pt) in curve.points.iter().take(128).enumerate() {
        let off = 3 + i * 2;
        buf[off..off + 2].copy_from_slice(&pt.to_le_bytes());
    }
    buf
}

pub fn decode_exposure_curve(data: &[u8]) -> Result<ExposureCurve, CodecError> {
    if data.len() < 3 {
        return Err(CodecError::PayloadTooShort {
            expected: 3,
            actual: data.len(),
        });
    }
    let mode = data[0];
    let iso = u16::from_le_bytes([data[1], data[2]]);
    let mut points = Vec::with_capacity(128);
    for chunk in data[3..].chunks_exact(2).take(128) {
        points.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok(ExposureCurve { mode, iso, points })
}

// ============================================================================
// XU1 Selector 0x08: Exposure Compensation (`0x100198f34`)
// ============================================================================

pub fn encode_exposure_compensation(ev: f32) -> [u8; 4] {
    ev.to_le_bytes()
}

pub fn decode_exposure_compensation(data: &[u8]) -> Result<f32, CodecError> {
    if data.len() < 4 {
        return Err(CodecError::PayloadTooShort {
            expected: 4,
            actual: data.len(),
        });
    }
    Ok(f32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

// ============================================================================
// XU1 Selector 0x0a: Extended Function Bitmask (`0x100199afc`–`0x10019a530`)
// ============================================================================

/// Encodes `(mask: u32, values: u32)` into the 8-byte payload for XU1 selector `0x0a`.
/// Verified at `EnableExtendFuncWork` (`0x100199afc`), `EnableVerticalScreen` (`0x10019b7d0`),
/// and `EnableLowResolution` (`0x10019b854`).
pub fn encode_extend_func_bitmask(func: ExtendFunction, enable: bool, current_values: u32) -> [u8; 8] {
    let bit = 1u32 << (func as u8);
    let new_values = if enable {
        current_values | bit
    } else {
        current_values & !bit
    };
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&bit.to_le_bytes());
    buf[4..8].copy_from_slice(&new_values.to_le_bytes());
    buf
}

/// Decodes the response from `GetExtendFuncStatus` (`0x100199a20`, XU1 selector `0x1b` or `0x0a`).
/// Hardware returns a 2-byte `u16` LE bitmask at `data[0..2]` (or 8-byte `(mask, values)` in 8B format).
pub fn decode_extend_func_status(data: &[u8]) -> Result<Vec<(ExtendFunction, bool)>, CodecError> {
    if data.len() < 2 {
        return Err(CodecError::PayloadTooShort {
            expected: 2,
            actual: data.len(),
        });
    }
    let bits = if data.len() >= 8 && (data[4] != 0 || data[5] != 0 || (data[0] == 0xff && data[1] == 0xff)) {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]])
    } else {
        u16::from_le_bytes([data[0], data[1]]) as u32
    };
    let mut out = Vec::with_capacity(ExtendFunction::ALL.len());
    for func in ExtendFunction::ALL {
        let enabled = (bits & (1u32 << (func as u8))) != 0;
        out.push((func, enabled));
    }
    Ok(out)
}

// ============================================================================
// XU1 Selector 0x0b: Relative Pan/Tilt (`0x1001990f4`)
// ============================================================================

pub fn encode_pantilt_relative_ext(pan_speed: u8, pan_dir: i8, tilt_speed: u8, tilt_dir: i8) -> [u8; 4] {
    let p_dir: u8 = if pan_dir == 0 || pan_speed == 0 {
        0
    } else if pan_dir > 0 {
        1
    } else {
        0xff
    };
    let t_dir: u8 = if tilt_dir == 0 || tilt_speed == 0 {
        0
    } else if tilt_dir > 0 {
        1
    } else {
        0xff
    };
    let p_spd = if p_dir == 0 { 1 } else { pan_speed.clamp(1, 10) };
    let t_spd = if t_dir == 0 { 1 } else { tilt_speed.clamp(1, 10) };
    [p_dir, p_spd, t_dir, t_spd]
}

// ============================================================================
// XU1 Selector 0x0d: PTZ Version & Gimbal Absolute Angles (`0x1001993bc`, `0x1001996cc`)
// ============================================================================

pub fn decode_ptz_version_and_abs(data: &[u8]) -> Result<PtzVersionInfo, CodecError> {
    if data.len() < 30 {
        return Err(CodecError::PayloadTooShort {
            expected: 30,
            actual: data.len(),
        });
    }
    let end = data[..20].iter().position(|&b| b == 0).unwrap_or(20);
    let version = String::from_utf8_lossy(&data[..end]).into_owned();
    let pan = i16::from_le_bytes([data[20], data[21]]);
    let tilt = i16::from_le_bytes([data[22], data[23]]);
    let zoom = u16::from_le_bytes([data[24], data[25]]);
    let status = data[26];
    Ok(PtzVersionInfo {
        version,
        pan,
        tilt,
        zoom,
        status,
    })
}

// ============================================================================
// XU1 Selector 0x0e: Preset Position Flags (`0x1001997c0`)
// ============================================================================

pub fn decode_preset_flags(data: &[u8]) -> Result<PresetFlag, CodecError> {
    if data.len() < 8 {
        return Err(CodecError::PayloadTooShort {
            expected: 8,
            actual: data.len(),
        });
    }
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&data[..8]);
    Ok(PresetFlag {
        slot_mask: raw[0],
        current_slot: raw[1],
        max_slots: raw[2].max(6),
        raw,
    })
}

// ============================================================================
// XU2 Selector 0x02: Timestamp, IQ Switch, Factory Restore (`0x10019bd00`–`0x10019c1ac`)
// ============================================================================

/// `SetTimestamp(unsigned long long)` (`0x10019be40`): `[1, ts_le[0..8], 0..]` (10 bytes)
pub fn encode_set_timestamp(timestamp_ms: u64) -> [u8; 10] {
    let mut buf = [0u8; 10];
    buf[0] = 0x01;
    buf[1..9].copy_from_slice(&timestamp_ms.to_le_bytes());
    buf
}

/// `SetIQSwitch(int, bool)` (`0x100199ce0`): `[2, mask_u32_le[0..4], 0..]` (10 bytes)
pub fn encode_set_iq_switch(bit_index: u8, enable: bool, current_mask: u32) -> Result<[u8; 10], CodecError> {
    if bit_index >= 32 {
        return Err(CodecError::InvalidArgument("bitset argument out of range"));
    }
    let bit = 1u32 << bit_index;
    let next = if enable {
        current_mask | bit
    } else {
        current_mask & !bit
    };
    let mut buf = [0u8; 10];
    buf[0] = 0x02;
    buf[1..5].copy_from_slice(&next.to_le_bytes());
    Ok(buf)
}

/// `RestoreFactorySet(bool)` (`0x10019c1ac`): `[4, 1, 0..]` if `reboot` else `[5, 1, 0..]` (10 bytes)
pub fn encode_restore_factory_set(full_reboot: bool) -> [u8; 10] {
    let mut buf = [0u8; 10];
    buf[0] = if full_reboot { 0x04 } else { 0x05 };
    buf[1] = 0x01;
    buf
}

// ============================================================================
// XU2 Selector 0x03: DeviceInfo (`GetDeviceInfo` at `0x100196e74`, 150 bytes)
// ============================================================================

fn extract_cstr(slice: &[u8]) -> String {
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).trim().to_string()
}

pub fn decode_device_info(data: &[u8]) -> Result<DeviceInfo, CodecError> {
    if data.len() < 150 {
        return Err(CodecError::PayloadTooShort {
            expected: 150,
            actual: data.len(),
        });
    }
    // Hardware layout (`MacUVCCameraExtendController::GetDeviceInfo` at `0x100196904`, >= 161 bytes):
    // [0x00..0x20] = serial_num (32B, e.g. "0AB586531221284"), [0x20] = need_update (1B),
    // [0x21..0x61] = device_uuid (64B), [0x61..0xa1] = cur_version (64B, e.g. "v1.4.5.8_build1")
    if data.len() >= 0xa1 {
        let sn = extract_cstr(&data[0x00..0x20]);
        let uuid = extract_cstr(&data[0x21..0x61]);
        let fw = extract_cstr(&data[0x61..0xa1]);
        if !fw.is_empty() || !sn.is_empty() || !uuid.is_empty() {
            return Ok(DeviceInfo {
                device_name: uuid,
                firmware_version: fw,
                serial_number: sn,
            });
        }
    }
    Ok(DeviceInfo {
        device_name: extract_cstr(&data[0..50]),
        firmware_version: extract_cstr(&data[50..100]),
        serial_number: extract_cstr(&data[100..150]),
    })
}

// ============================================================================
// XU2 Selector 0x04 & 0x05: Image Templates (`0x10019ba4c`–`0x10019bd90`)
// ============================================================================

pub fn decode_image_template_info(data: &[u8]) -> Result<ImageTemplateInfo, CodecError> {
    if data.len() < 11 {
        return Err(CodecError::PayloadTooShort {
            expected: 11,
            actual: data.len(),
        });
    }
    let current_id = data[0] as i8;
    let operation = data[1];
    let bitmask = u64::from_le_bytes([
        data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9],
    ]);
    let max_templates = data[10];
    Ok(ImageTemplateInfo {
        current_id,
        operation,
        bitmask,
        max_templates,
    })
}

pub fn encode_set_image_template(template_id: i8, op: ImageOperation, current_mask: u64, max_templates: u8) -> Result<[u8; 11], CodecError> {
    if template_id < 0 || (template_id as u8) >= 64 {
        return Err(CodecError::InvalidArgument("template_id out of range (0..64)"));
    }
    let next_mask = match op {
        ImageOperation::SaveOrUpdate => current_mask | (1u64 << (template_id as u8)),
        _ => current_mask,
    };
    let mut buf = [0u8; 11];
    buf[0] = template_id as u8;
    buf[1] = op as u8;
    buf[2..10].copy_from_slice(&next_mask.to_le_bytes());
    buf[10] = max_templates;
    Ok(buf)
}

pub fn encode_delete_image_template(template_id: i8, current: &ImageTemplateInfo) -> Result<[u8; 11], CodecError> {
    if template_id < 0 || (template_id as u8) >= 64 {
        return Err(CodecError::InvalidArgument("template_id out of range (0..64)"));
    }
    let next_mask = current.bitmask & !(1u64 << (template_id as u8));
    let mut buf = [0u8; 11];
    buf[0] = current.current_id as u8;
    buf[1] = current.operation;
    buf[2..10].copy_from_slice(&next_mask.to_le_bytes());
    buf[10] = current.max_templates;
    Ok(buf)
}

pub fn encode_image_template_name(template_id: i8, name: &str) -> [u8; 65] {
    let mut buf = [0u8; 65];
    buf[0] = template_id as u8;
    let bytes = name.as_bytes();
    let copy_len = bytes.len().min(63);
    buf[1..1 + copy_len].copy_from_slice(&bytes[..copy_len]);
    buf
}

pub fn decode_image_template_name(data: &[u8]) -> Result<(i8, String), CodecError> {
    if data.len() < 2 {
        return Err(CodecError::PayloadTooShort {
            expected: 2,
            actual: data.len(),
        });
    }
    let id = data[0] as i8;
    let name = extract_cstr(&data[1..]);
    Ok((id, name))
}

// ============================================================================
// XU2 Selector 0x08: Privacy Mode & Fill Light (`0x10019c0a8`–`0x10019c168`)
// ============================================================================

pub fn encode_set_privacy_mode(enable: bool) -> [u8; 4] {
    [0x02, u8::from(enable), 0x00, 0x00]
}

pub fn decode_privacy_mode(data: &[u8]) -> Result<(i32, bool), CodecError> {
    if data.len() < 2 {
        return Err(CodecError::PayloadTooShort {
            expected: 2,
            actual: data.len(),
        });
    }
    Ok((data[0] as i32, data[1] != 0))
}

// ============================================================================
// XU2 Selector 0x09: Host Digital PTZ (`SetHostPTZ` at `0x10019bec0`)
// ============================================================================

pub fn encode_host_ptz(info: &HostPtzInfo) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..2].copy_from_slice(&info.x.to_le_bytes());
    buf[2..4].copy_from_slice(&info.y.to_le_bytes());
    buf[4..6].copy_from_slice(&info.width.to_le_bytes());
    buf[6..8].copy_from_slice(&info.height.to_le_bytes());
    buf
}

// ============================================================================
// XU2 Selector 0x01: Gesture Binding Map (`GetGestureBindingMap` / `SetGestureBindingFunc` at `0x100198384`)
// ============================================================================

pub fn encode_gesture_binding(gesture: Gesture, func: GestureFunction) -> [u8; 16] {
    let mut buf = [0u8; 16];
    buf[0] = gesture as u8;
    buf[1] = func as u8;
    buf
}

pub fn decode_gesture_binding_map(data: &[u8]) -> Result<[(Gesture, GestureFunction); 5], CodecError> {
    if data.len() < 5 {
        return Err(CodecError::PayloadTooShort {
            expected: 5,
            actual: data.len(),
        });
    }
    Ok([
        (Gesture::Ok, GestureFunction::from_u8(data[0])),
        (Gesture::Palm, GestureFunction::from_u8(data[1])),
        (Gesture::LShape, GestureFunction::from_u8(data[2])),
        (Gesture::VShape, GestureFunction::from_u8(data[3])),
        (Gesture::Rock, GestureFunction::from_u8(data[4])),
    ])
}

// ============================================================================
// XU2 Selector 0x0b: Tracking Forbidden Areas (`SetTrackForbiddenArea` / `GetTrackForbiddenArea` at `0x10019bff4`)
// ============================================================================

pub fn encode_forbidden_areas(areas: &[ForbiddenAreaInfo]) -> Vec<u8> {
    let count = areas.len().clamp(1, 4);
    let mut buf = vec![0u8; count * 10];
    for (i, area) in areas.iter().take(count).enumerate() {
        let off = i * 10;
        buf[off] = u8::from(area.enabled);
        buf[off + 1] = area.index;
        buf[off + 2..off + 4].copy_from_slice(&area.pan_min.to_le_bytes());
        buf[off + 4..off + 6].copy_from_slice(&area.pan_max.to_le_bytes());
        buf[off + 6..off + 8].copy_from_slice(&area.tilt_min.to_le_bytes());
        buf[off + 8..off + 10].copy_from_slice(&area.tilt_max.to_le_bytes());
    }
    buf
}

pub fn decode_forbidden_areas(data: &[u8]) -> Vec<ForbiddenAreaInfo> {
    let mut out = Vec::new();
    for chunk in data.chunks_exact(10) {
        out.push(ForbiddenAreaInfo {
            enabled: chunk[0] != 0,
            index: chunk[1],
            pan_min: i16::from_le_bytes([chunk[2], chunk[3]]),
            pan_max: i16::from_le_bytes([chunk[4], chunk[5]]),
            tilt_min: i16::from_le_bytes([chunk[6], chunk[7]]),
            tilt_max: i16::from_le_bytes([chunk[8], chunk[9]]),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::CameraModel;

    #[test]
    fn test_uvc_header_encoding() {
        let set_hdr = UvcControlPacketHeader::new(UvcRequest::SetCur, 0x02, 0x09, 0x00);
        assert_eq!(set_hdr.bm_request_type, 0x21);
        assert_eq!(set_hdr.b_request, 0x01);
        assert_eq!(set_hdr.w_value, 0x0200);
        assert_eq!(set_hdr.w_index, 0x0900);

        let get_hdr = UvcControlPacketHeader::new(UvcRequest::GetCur, 0x03, 0x0a, 0x00);
        assert_eq!(get_hdr.bm_request_type, 0xa1);
        assert_eq!(get_hdr.b_request, 0x81);
        assert_eq!(get_hdr.w_value, 0x0300);
        assert_eq!(get_hdr.w_index, 0x0a00);
    }

    #[test]
    fn test_video_mode_payload_lengths() {
        assert_eq!(CameraModel::Link.video_mode_payload_len(), 8);
        assert_eq!(CameraModel::Link2.video_mode_payload_len(), 8);
        assert_eq!(CameraModel::Link2C.video_mode_payload_len(), 36);
        assert_eq!(CameraModel::Puc3Host.video_mode_payload_len(), 36);
    }

    #[test]
    fn test_extend_func_bitmask_roundtrip() {
        let encoded = encode_extend_func_bitmask(ExtendFunction::Hdr, true, 0);
        assert_eq!(u32::from_le_bytes(encoded[0..4].try_into().unwrap()), 1 << 2);
        assert_eq!(u32::from_le_bytes(encoded[4..8].try_into().unwrap()), 1 << 2);

        let decoded = decode_extend_func_status(&encoded).unwrap();
        let hdr_state = decoded.into_iter().find(|(f, _)| *f == ExtendFunction::Hdr).unwrap().1;
        assert!(hdr_state);
    }

    #[test]
    fn test_device_info_decode() {
        let mut raw = [0u8; 150];
        raw[0..15].copy_from_slice(b"Insta360 Link 2");
        raw[50..65].copy_from_slice(b"v2.5.8.8_build1");
        raw[100..114].copy_from_slice(b"INK2A240912345");
        let info = decode_device_info(&raw).unwrap();
        assert_eq!(info.device_name, "Insta360 Link 2");
        assert_eq!(info.firmware_version, "v2.5.8.8_build1");
        assert_eq!(info.serial_number, "INK2A240912345");
    }
}
