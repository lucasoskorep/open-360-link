use crate::codec::*;
use crate::ids::*;
use crate::transport::{TransportError, UvcControlTransport};
use crate::types::*;

/// High-level cross-platform controller for Insta360 Link, Link 2, and Link 2C cameras.
/// Implements all standard UVC Processing Unit (`PU_*`), Camera Terminal (`CT_*`),
/// and proprietary Insta360 Extension Unit 1 & 2 (`XU1_*`, `XU2_*`) commands.
pub struct Insta360CameraController<T: UvcControlTransport> {
    transport: T,
}

impl<T: UvcControlTransport> Insta360CameraController<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn camera_info(&self) -> &DiscoveredCamera {
        self.transport.camera_info()
    }

    pub fn model(&self) -> CameraModel {
        self.transport.camera_info().model
    }

    pub fn capabilities(&self) -> DeviceCapabilities {
        self.transport.camera_info().capabilities
    }

    fn unit_ids(&self) -> UvcUnitIds {
        self.transport.camera_info().unit_ids
    }

    fn is_hardware(&self) -> bool {
        !self.transport.camera_info().unique_id.starts_with("mock-")
    }

    /// Queries a standard 2-byte signed/unsigned UVC range (`GET_MIN`, `GET_MAX`, `GET_RES`, `GET_DEF`).
    fn query_range_u16(&self, unit_id: u8, selector: u8, signed: bool) -> Result<ControlRange, TransportError> {
        let read_val = |req: UvcRequest| -> Result<i64, TransportError> {
            let b = self.transport.get_req(req, unit_id, selector, 2)?;
            if b.len() < 2 {
                return Ok(0);
            }
            if signed {
                Ok(i16::from_le_bytes([b[0], b[1]]) as i64)
            } else {
                Ok(u16::from_le_bytes([b[0], b[1]]) as i64)
            }
        };
        Ok(ControlRange {
            min: read_val(UvcRequest::GetMin)?,
            max: read_val(UvcRequest::GetMax)?,
            step: read_val(UvcRequest::GetRes)?,
            default: read_val(UvcRequest::GetDef)?,
            flags: 0,
        })
    }

    // ========================================================================
    // Standard UVC Processing Unit Controls (Brightness, Contrast, Saturation, Sharpness, WB, PowerLine)
    // ========================================================================

    pub fn get_brightness_range(&self) -> Result<ControlRange, TransportError> {
        self.query_range_u16(self.unit_ids().processing_unit_id, pu_selectors::BRIGHTNESS, true)
    }

    pub fn get_brightness(&self) -> Result<i16, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().processing_unit_id,
            pu_selectors::BRIGHTNESS,
            2,
        )?;
        Ok(i16::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]))
    }

    pub fn set_brightness(&self, value: i16) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().processing_unit_id,
            pu_selectors::BRIGHTNESS,
            &value.to_le_bytes(),
        )
    }

    pub fn get_contrast_range(&self) -> Result<ControlRange, TransportError> {
        self.query_range_u16(self.unit_ids().processing_unit_id, pu_selectors::CONTRAST, false)
    }

    pub fn get_contrast(&self) -> Result<u16, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().processing_unit_id,
            pu_selectors::CONTRAST,
            2,
        )?;
        Ok(u16::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]))
    }

    pub fn set_contrast(&self, value: u16) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().processing_unit_id,
            pu_selectors::CONTRAST,
            &value.to_le_bytes(),
        )
    }

    pub fn get_saturation_range(&self) -> Result<ControlRange, TransportError> {
        self.query_range_u16(self.unit_ids().processing_unit_id, pu_selectors::SATURATION, false)
    }

    pub fn get_saturation(&self) -> Result<u16, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().processing_unit_id,
            pu_selectors::SATURATION,
            2,
        )?;
        Ok(u16::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]))
    }

    pub fn set_saturation(&self, value: u16) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().processing_unit_id,
            pu_selectors::SATURATION,
            &value.to_le_bytes(),
        )
    }

    pub fn get_sharpness_range(&self) -> Result<ControlRange, TransportError> {
        self.query_range_u16(self.unit_ids().processing_unit_id, pu_selectors::SHARPNESS, false)
    }

    pub fn get_sharpness(&self) -> Result<u16, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().processing_unit_id,
            pu_selectors::SHARPNESS,
            2,
        )?;
        Ok(u16::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]))
    }

    pub fn set_sharpness(&self, value: u16) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().processing_unit_id,
            pu_selectors::SHARPNESS,
            &value.to_le_bytes(),
        )
    }

    pub fn get_white_balance_range(&self) -> Result<ControlRange, TransportError> {
        self.query_range_u16(
            self.unit_ids().processing_unit_id,
            pu_selectors::WHITE_BALANCE_TEMPERATURE,
            false,
        )
    }

    pub fn get_white_balance_temperature(&self) -> Result<u16, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().processing_unit_id,
            pu_selectors::WHITE_BALANCE_TEMPERATURE,
            2,
        )?;
        Ok(u16::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]))
    }

    pub fn set_white_balance_temperature(&self, kelvin: u16) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().processing_unit_id,
            pu_selectors::WHITE_BALANCE_TEMPERATURE,
            &kelvin.to_le_bytes(),
        )
    }

    pub fn get_auto_white_balance(&self) -> Result<bool, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().processing_unit_id,
            pu_selectors::WHITE_BALANCE_TEMPERATURE_AUTO,
            1,
        )?;
        Ok(b.first().copied().unwrap_or(0) != 0)
    }

    pub fn set_auto_white_balance(&self, auto: bool) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().processing_unit_id,
            pu_selectors::WHITE_BALANCE_TEMPERATURE_AUTO,
            &[u8::from(auto)],
        )
    }

    pub fn get_power_line_frequency(&self) -> Result<PowerLineFrequency, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().processing_unit_id,
            pu_selectors::POWER_LINE_FREQUENCY,
            1,
        )?;
        Ok(PowerLineFrequency::from_u8(b.first().copied().unwrap_or(0)))
    }

    pub fn set_power_line_frequency(&self, freq: PowerLineFrequency) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().processing_unit_id,
            pu_selectors::POWER_LINE_FREQUENCY,
            &[freq as u8],
        )
    }

    // ========================================================================
    // Standard UVC Camera Terminal Controls (Zoom, Focus, Roll, Shutter, Pan/Tilt)
    // ========================================================================

    pub fn get_zoom_range(&self) -> Result<ControlRange, TransportError> {
        self.query_range_u16(self.unit_ids().input_terminal_id, ct_selectors::ZOOM_ABSOLUTE, false)
    }

    pub fn get_zoom(&self) -> Result<u16, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().input_terminal_id,
            ct_selectors::ZOOM_ABSOLUTE,
            2,
        )?;
        Ok(u16::from_le_bytes([b.first().copied().unwrap_or(100), b.get(1).copied().unwrap_or(0)]))
    }

    pub fn set_zoom(&self, zoom: u16) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().input_terminal_id,
            ct_selectors::ZOOM_ABSOLUTE,
            &zoom.to_le_bytes(),
        )
    }

    pub fn get_auto_focus(&self) -> Result<bool, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().input_terminal_id,
            ct_selectors::FOCUS_AUTO,
            1,
        )?;
        Ok(b.first().copied().unwrap_or(0) != 0)
    }

    pub fn set_auto_focus(&self, enabled: bool) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().input_terminal_id,
            ct_selectors::FOCUS_AUTO,
            &[u8::from(enabled)],
        )
    }

    pub fn get_focus_range(&self) -> Result<ControlRange, TransportError> {
        self.query_range_u16(self.unit_ids().input_terminal_id, ct_selectors::FOCUS_ABSOLUTE, false)
    }

    pub fn get_focus(&self) -> Result<u16, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().input_terminal_id,
            ct_selectors::FOCUS_ABSOLUTE,
            2,
        )?;
        Ok(u16::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]))
    }

    pub fn set_focus(&self, focus: u16) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().input_terminal_id,
            ct_selectors::FOCUS_ABSOLUTE,
            &focus.to_le_bytes(),
        )
    }

    pub fn get_roll(&self) -> Result<i16, TransportError> {
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().input_terminal_id,
            ct_selectors::ROLL_ABSOLUTE,
            2,
        )?;
        Ok(i16::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]))
    }

    pub fn set_roll(&self, roll_tenths_deg: i16) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().input_terminal_id,
            ct_selectors::ROLL_ABSOLUTE,
            &roll_tenths_deg.to_le_bytes(),
        )
    }

    pub fn get_shutter_speed(&self) -> Result<u32, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraController::GetShutterSpeed` (`0x100195804`): XU1 selector `0x1d` (2 bytes u16_le)
            if let Ok(b) = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x1d,
                2,
            ) {
                if b.len() >= 2 {
                    return Ok(u16::from_le_bytes([b[0], b[1]]) as u32);
                }
            }
        }
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().input_terminal_id,
            ct_selectors::EXPOSURE_TIME_ABSOLUTE,
            4,
        )?;
        if b.len() < 4 {
            return Ok(0);
        }
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn set_shutter_speed(&self, exposure_time: u32) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraController::SetShutterSpeed` (`0x10019577c`): XU1 selector `0x1d` (2 bytes u16_le)
            let denom = (exposure_time as u16).to_le_bytes();
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x1d,
                &denom,
            );
        }
        self.transport.set_cur(
            self.unit_ids().input_terminal_id,
            ct_selectors::EXPOSURE_TIME_ABSOLUTE,
            &exposure_time.to_le_bytes(),
        )
    }

    pub fn get_pantilt_absolute(&self) -> Result<(i32, i32), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetPanTiltAbsoluteValue` (`0x10019a614`) & `savePTZ` (`0x100098b94`..`0x100098c8c`):
            // XU1 selector `0x1a` (8 bytes) stores `[tilt_arcsec_i32_le, pan_arcsec_i32_le]`.
            if let Ok(b) = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x1a,
                8,
            ) {
                if b.len() >= 8 {
                    let tilt = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                    let pan = i32::from_le_bytes([b[4], b[5], b[6], b[7]]);
                    return Ok((pan, tilt));
                }
            }
        }
        let b = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().input_terminal_id,
            ct_selectors::PANTILT_ABSOLUTE,
            8,
        )?;
        if b.len() < 8 {
            return Ok((0, 0));
        }
        let pan = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let tilt = i32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        Ok((pan, tilt))
    }

    pub fn set_pantilt_absolute(&self, pan_arcsec: i32, tilt_arcsec: i32) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetPanTiltAbsolute` (`0x10019a6b0`) & `usePTZ` (`0x10009a760`..`0x10009a770`):
            // XU1 selector `0x1a` (8 bytes) expects `[tilt_arcsec_i32_le, pan_arcsec_i32_le]`.
            let mut xu_payload = [0u8; 8];
            xu_payload[0..4].copy_from_slice(&tilt_arcsec.to_le_bytes());
            xu_payload[4..8].copy_from_slice(&pan_arcsec.to_le_bytes());
            if self
                .transport
                .set_cur(self.unit_ids().extension_unit_id, 0x1a, &xu_payload)
                .is_ok()
            {
                return Ok(());
            }
        }
        let mut payload = [0u8; 8];
        payload[0..4].copy_from_slice(&pan_arcsec.to_le_bytes());
        payload[4..8].copy_from_slice(&tilt_arcsec.to_le_bytes());
        self.transport.set_cur(
            self.unit_ids().input_terminal_id,
            ct_selectors::PANTILT_ABSOLUTE,
            &payload,
        )
    }

    // ========================================================================
    // Insta360 Extension Unit 1 & 2 Controls (`MacUVCCameraExtendController`)
    // ========================================================================

    pub fn get_device_info(&self) -> Result<DeviceInfo, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetDeviceInfo` (`0x1001967f0`): XU1 selector `0x03`
            // plus `GetSerialNumber` (`0x100196770`): XU1 selector `0x0c`
            let data = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x03,
                255,
            )?;
            let mut info = decode_device_info(&data).unwrap_or_else(|_| DeviceInfo {
                device_name: self.camera_info().name.clone(),
                firmware_version: String::new(),
                serial_number: String::new(),
            });
            if info.device_name.is_empty() {
                info.device_name = self.camera_info().name.clone();
            }
            if info.serial_number.is_empty() {
                if let Ok(sn_bytes) = self.transport.get_req(
                    UvcRequest::GetCur,
                    self.unit_ids().extension_unit_id,
                    0x0c,
                    64,
                ) {
                    let end = sn_bytes.iter().position(|&b| b == 0).unwrap_or(sn_bytes.len());
                    let sn = String::from_utf8_lossy(&sn_bytes[..end]).trim().to_string();
                    if !sn.is_empty() {
                        info.serial_number = sn;
                    }
                }
            }
            return Ok(info);
        }
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            xu2_selectors::DEVICE_INFO,
            150,
        )?;
        Ok(decode_device_info(&data)?)
    }

    pub fn get_ptz_version_and_angles(&self) -> Result<PtzVersionInfo, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetPTZVersionInfo` (`0x100199398`):
            // Sends magic header `[0xa5, 0xd0, 0x03, 0x00, 0xf1, 0x32, 0..]` to XU1 selector `0x04`,
            // then reads 255 bytes from XU1 selector `0x04`.
            let mut magic = [0u8; 255];
            magic[0..6].copy_from_slice(&[0xa5, 0xd0, 0x03, 0x00, 0xf1, 0x32]);
            let _ = self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x04,
                &magic,
            );
            let data = self
                .transport
                .get_req(UvcRequest::GetCur, self.unit_ids().extension_unit_id, 0x04, 255)
                .unwrap_or_default();
            let version = if data.len() >= 16 {
                format!("{}.{}.{}.{}", data[4], data[5], data[6], data[7])
            } else {
                String::from("v1.0")
            };
            let (pan_arcsec, tilt_arcsec) = self.get_pantilt_absolute().unwrap_or((0, 0));
            let zoom = self.get_zoom().unwrap_or(100);
            return Ok(PtzVersionInfo {
                version,
                pan: (pan_arcsec / 360) as i16,
                tilt: (tilt_arcsec / 360) as i16,
                zoom,
                status: data.get(20).copied().unwrap_or(0),
            });
        }
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::PTZ_VERSION_AND_ABS,
            30,
        )?;
        Ok(decode_ptz_version_and_abs(&data)?)
    }

    pub fn get_device_status(&self) -> Result<DeviceStatus, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetDeviceStatus` (`0x100197864`): XU1 selector `0x0b`
            let data = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x0b,
                8,
            )?;
            let mut status = DeviceStatus::empty();
            if data.first().copied().unwrap_or(0) != 0 {
                status |= DeviceStatus::TRACKING_ACTIVE;
            }
            if data.get(3).copied().unwrap_or(0) == 1 {
                status |= DeviceStatus::GESTURE_DETECTED;
            }
            if data.get(4).copied().unwrap_or(0) == 1 {
                status |= DeviceStatus::WHITEBOARD_ACTIVE;
            }
            return Ok(status);
        }
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::DEVICE_STATUS,
            4,
        )?;
        if data.len() < 4 {
            return Ok(DeviceStatus::empty());
        }
        let raw = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        Ok(DeviceStatus::from_bits_retain(raw))
    }

    pub fn reset_gimbal(&self) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `Webcam::CameraInsta::resetPTZ` (`0x100097ce4`): resets pan/tilt to (0, 0) and zoom to 100
            let _ = self.set_pantilt_absolute(0, 0);
            let _ = self.set_zoom(100);
            return Ok(());
        }
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            &encode_reset_gimbal(),
        )
    }

    pub fn load_factory_default(&self) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::LoadFactoryDefault` (`0x100196f50`): XU1 selector `0x0e`, `[0x81]`
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x0e,
                &[0x81],
            );
        }
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            &encode_load_factory_default(),
        )
    }

    pub fn restore_factory_settings(&self, full_reboot: bool) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::RestoreFactorySet(bool)` (`0x10019b590`): XU2 selector `0x0c`, 1 byte
            let _ = self.transport.set_cur(
                self.unit_ids().extension_unit2_id,
                0x0c,
                &[u8::from(full_reboot)],
            );
            return self.load_factory_default();
        }
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::SYSTEM_TIME_AND_IQ,
            &encode_restore_factory_set(full_reboot),
        )
    }

    pub fn set_timestamp(&self, timestamp_ms: u64) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetTimestamp` (`0x10019b0b0`): XU2 selector `0x07`, 10 bytes
            let mut buf = [0u8; 10];
            buf[0..8].copy_from_slice(&timestamp_ms.to_le_bytes());
            return self.transport.set_cur(
                self.unit_ids().extension_unit2_id,
                0x07,
                &buf,
            );
        }
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::SYSTEM_TIME_AND_IQ,
            &encode_set_timestamp(timestamp_ms),
        )
    }

    pub fn set_iq_switch(&self, bit_index: u8, enable: bool, current_mask: u32) -> Result<(), TransportError> {
        let payload = encode_set_iq_switch(bit_index, enable, current_mask)?;
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::SYSTEM_TIME_AND_IQ,
            &payload,
        )
    }

    pub fn set_pantilt_relative_ext(
        &self,
        pan_speed: u8,
        pan_dir: i8,
        tilt_speed: u8,
        tilt_dir: i8,
    ) -> Result<(), TransportError> {
        let payload = encode_pantilt_relative_ext(pan_speed, pan_dir, tilt_speed, tilt_dir);
        let sel = if self.is_hardware() {
            0x16 // `MacUVCCameraExtendController::SetPanTiltRelative` (`0x100199170`)
        } else {
            xu1_selectors::PANTILT_RELATIVE_EXT
        };
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            sel,
            &payload,
        )
    }

    pub fn stop_pantilt_relative(&self) -> Result<(), TransportError> {
        self.set_pantilt_relative_ext(0, 0, 0, 0)
    }

    pub fn set_composition_bias(&self, x: i16, y: i16) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetCompositionBias` (`0x100199298`): XU1 selector `0x18`, 4 bytes `[x_lo, x_hi, y_lo, y_hi]`
            let xb = x.to_le_bytes();
            let yb = y.to_le_bytes();
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x18,
                &[xb[0], xb[1], yb[0], yb[1]],
            );
        }
        let payload = encode_set_composition_bias(x, y);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            &payload,
        )
    }

    pub fn reset_composition_bias(&self) -> Result<(), TransportError> {
        self.set_composition_bias(0x7fff, 0x7fff)
    }

    pub fn get_preset_flags(&self) -> Result<PresetFlag, TransportError> {
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::PRESET_POSITION_FLAGS,
            8,
        )?;
        Ok(decode_preset_flags(&data)?)
    }

    pub fn add_preset_position(&self, index: u8) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            &encode_add_preset_position(index),
        )
    }

    pub fn recall_preset_position(&self, index: u8) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            &encode_set_cur_preset_position(index),
        )
    }

    pub fn delete_preset_position(&self, index: u8) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            &encode_delete_preset_position(index),
        )
    }

    pub fn get_video_mode(&self) -> Result<(VideoMode, VideoModeAuxData), TransportError> {
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::VIDEO_MODE,
            36,
        )?;
        Ok(decode_get_video_mode(&data)?)
    }

    pub fn set_video_mode_raw(&self, mode: VideoMode, aux: &VideoModeAuxData) -> Result<(), TransportError> {
        if self.is_hardware() {
            // Exact 56-byte (`0x38`) payload from `MacUVCCameraExtendController::SetVideoMode` (`0x100197088`..`0x1001972f0`)
            // verified via Frida trace on Link 1 (`2e1a:4c01`):
            // `[mode, flags, 0..0 (36B), 1a 0e 00 00, 1a 0e 00 00, 1a 0e 00 00, 00 00, pitch_lo, pitch_hi, 00 00]`
            let mut buf = [0u8; 56];
            buf[0] = mode as u8;
            buf[1] = aux.flags;
            for (i, &val) in aux.rect.iter().enumerate() {
                let off = 2 + i * 4;
                buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
            }
            // Default sentinel values `3610` (`0x0e1a`) at 0x26, 0x2a, 0x2e so firmware does not override PTZ to (0,0)
            buf[0x26..0x2a].copy_from_slice(&3610i32.to_le_bytes());
            buf[0x2a..0x2e].copy_from_slice(&3610i32.to_le_bytes());
            buf[0x2e..0x32].copy_from_slice(&3610i32.to_le_bytes());
            buf[0x32..0x34].copy_from_slice(&0u16.to_le_bytes());
            let pitch_val = if mode == VideoMode::DeskView {
                if aux.pitch != 0 { aux.pitch } else { -100 }
            } else {
                -450
            };
            buf[0x34..0x36].copy_from_slice(&pitch_val.to_le_bytes());
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                xu1_selectors::VIDEO_MODE,
                &buf,
            );
        }
        let len = self.model().video_mode_payload_len();
        let payload = encode_set_video_mode(mode, aux, len);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::VIDEO_MODE,
            &payload,
        )
    }

    pub fn set_video_mode(&self, mode: VideoMode, aux: &VideoModeAuxData) -> Result<(), TransportError> {
        if self.is_hardware() && mode == VideoMode::AutoTracking {
            // `Webcam::CameraInsta::autoTrackClicked` (`0x10006c400`) -> `onHandleVideoModeChangeEvent` (`0x10005a4cc`):
            // First ensure the camera is in `VideoMode::Normal` (`0`) so `GetTrackObjLists` (`XU1 0x14`)
            // runs the firmware person detector, then poll `GetTrackObjLists` every 200ms until a person
            // is detected, then call `SetTrackObj` (`XU1 0x15`) + `SetVideoMode(1)` (`XU1 0x02`).
            if let Ok((cur_mode, _)) = self.get_video_mode() {
                if cur_mode != VideoMode::Normal && cur_mode != VideoMode::AutoTracking {
                    let _ = self.set_video_mode_raw(VideoMode::Normal, &VideoModeAuxData::default());
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
            for _ in 0..30 {
                if let Ok(rects) = self.get_track_objects() {
                    if let Some(r) = rects.first() {
                        let cx = ((r.x as f32) + (r.width as f32) * 0.5_f32) / 10000.0_f32;
                        let cy = ((r.y as f32) + (r.height as f32) * 0.5_f32) / 10000.0_f32;
                        if cx > 0.0_f32 && cy > 0.0_f32 {
                            let _ = self.select_track_object_norm(
                                cx.clamp(0.05, 0.95),
                                cy.clamp(0.05, 0.95),
                                0.0,
                                0.0,
                            );
                            break;
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
        self.set_video_mode_raw(mode, aux)
    }

    pub fn get_composition_style(&self) -> Result<CompositionStyle, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetCompositionStyle` (`0x100197970`): XU1 selector `0x13`
            let data = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x13,
                1,
            )?;
            return Ok(CompositionStyle::from_u8(data.first().copied().unwrap_or(0)));
        }
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            16,
        )?;
        Ok(decode_get_composition_style(&data)?)
    }

    pub fn set_composition_style(&self, style: CompositionStyle) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetCompositionStyle` (`0x100197900`): XU1 selector `0x13`, 1 byte
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x13,
                &[style as u8],
            );
        }
        let payload = encode_set_composition_style(style);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            &payload,
        )
    }

    pub fn get_track_speed(&self) -> Result<TrackSpeed, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetTrackSpeed` (`0x100197a54`): XU1 selector `0x12`
            let data = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x12,
                1,
            )?;
            return Ok(TrackSpeed::from_u8(data.first().copied().unwrap_or(1)));
        }
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            16,
        )?;
        Ok(decode_get_track_speed(&data)?)
    }

    pub fn set_track_speed(&self, speed: TrackSpeed) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetTrackSpeed` (`0x1001979e4`): XU1 selector `0x12`, 1 byte
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x12,
                &[speed as u8],
            );
        }
        let payload = encode_set_track_speed(speed);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            &payload,
        )
    }

    pub fn get_gesture_status(&self) -> Result<[(Gesture, bool); 5], TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetGestureStatus` (`0x100197ea0`): XU1 selector `0x05`
            let data = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x05,
                6,
            )?;
            let b = data.first().copied().unwrap_or(0);
            if data.len() >= 6 && (data[1] <= 1 && data[2] <= 1 && (data[1] != 0 || data[2] != 0)) {
                return Ok(decode_gesture_status(&data)?);
            }
            return Ok([
                (Gesture::Ok, (b & (1 << 0)) != 0),
                (Gesture::Palm, (b & (1 << 1)) != 0),
                (Gesture::LShape, (b & (1 << 2)) != 0),
                (Gesture::VShape, (b & (1 << 3)) != 0),
                (Gesture::Rock, (b & (1 << 4)) != 0),
            ]);
        }
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            6,
        )?;
        Ok(decode_gesture_status(&data)?)
    }

    pub fn set_all_gestures_enabled(&self, enable: bool) -> Result<(), TransportError> {
        if self.is_hardware() {
            return self.set_extend_function(ExtendFunction::AllGestures, enable);
        }
        let payload = encode_enable_all_gestures(enable);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            &payload,
        )?;
        self.set_extend_function(ExtendFunction::AllGestures, enable)
    }

    pub fn set_gesture_enabled(&self, gesture: Gesture, enable: bool) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::EnableGestureWork` (`0x100197bc8`): XU1 selector `0x05`
            let mut cur = self
                .transport
                .get_req(UvcRequest::GetCur, self.unit_ids().extension_unit_id, 0x05, 6)
                .unwrap_or_else(|_| vec![0u8; 2]);
            if cur.is_empty() {
                cur.push(0);
            }
            let bit = 1u8 << (gesture as u8);
            if enable {
                cur[0] |= bit;
            } else {
                cur[0] &= !bit;
            }
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x05,
                &cur,
            );
        }
        let cur = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            6,
        )?;
        let payload = encode_enable_gesture_work(&cur, gesture, enable);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            &payload,
        )
    }

    pub fn get_extend_function_status(&self) -> Result<Vec<(ExtendFunction, bool)>, TransportError> {
        let sel = if self.is_hardware() {
            0x1b // `MacUVCCameraExtendController::GetExtendFuncStatus` (`0x100199a20`)
        } else {
            xu1_selectors::EXTEND_FUNC_BITMASK
        };
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            sel,
            8,
        )?;
        Ok(decode_extend_func_status(&data)?)
    }

    pub fn set_extend_function(&self, func: ExtendFunction, enable: bool) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::EnableExtendFuncWork` (`0x1001996c0`):
            // Reads `u16` bitmask from XU1 selector `0x1b`, updates `1 << func`, writes back to `0x1b`.
            let mut cur = self
                .transport
                .get_req(UvcRequest::GetCur, self.unit_ids().extension_unit_id, 0x1b, 2)
                .unwrap_or_else(|_| vec![0u8; 2]);
            if cur.len() < 2 {
                cur.resize(2, 0);
            }
            let mut bits = u16::from_le_bytes([cur[0], cur[1]]);
            let mask = 1u16 << (func as u8);
            if ((bits & mask) != 0) == enable {
                return Ok(());
            }
            if enable {
                bits |= mask;
            } else {
                bits &= !mask;
            }
            let bytes = bits.to_le_bytes();
            cur[0] = bytes[0];
            cur[1] = bytes[1];
            let res = self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x1b,
                &cur,
            );
            // Toggling `VerticalMode` or `LowResolution` triggers an immediate USB re-enumeration
            // inside the camera firmware to expose the updated UVC frame/FPS descriptors.
            if matches!(
                func,
                ExtendFunction::VerticalMode | ExtendFunction::LowResolution
            ) {
                return Ok(());
            }
            return res;
        }
        let cur = self
            .transport
            .get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                xu1_selectors::EXTEND_FUNC_BITMASK,
                8,
            )
            .unwrap_or_else(|_| vec![0u8; 8]);
        let current_values = if cur.len() >= 8 {
            u32::from_le_bytes([cur[4], cur[5], cur[6], cur[7]])
        } else {
            0
        };
        let payload = encode_extend_func_bitmask(func, enable, current_values);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::EXTEND_FUNC_BITMASK,
            &payload,
        )
    }

    pub fn set_hdr(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::Hdr, enable)
    }

    pub fn set_ai_zoom(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::AiZoom, enable)
    }

    pub fn set_horizontal_mirror(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::HorizontalMirror, enable)
    }

    pub fn set_vertical_mirror(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::VerticalMirror, enable)
    }

    pub fn set_vertical_screen(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::VerticalMode, enable)
    }

    pub fn set_forced_vertical(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::ForcedVertical, enable)
    }

    pub fn set_low_resolution(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::LowResolution, enable)
    }

    pub fn set_auto_tracking_enabled(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::AutoTracking, enable)
    }

    pub fn set_single_tap_tracking_enabled(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::SingleTapTracking, enable)
    }

    pub fn set_smart_adjustment(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::SmartAdjustment, enable)
    }

    pub fn set_startup_params_enabled(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::StartupParams, enable)
    }

    pub fn set_ultimate_privacy_mode(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::UltimatePrivacyMode, enable)
    }

    pub fn get_exposure_curve(&self) -> Result<ExposureCurve, TransportError> {
        let sel = if self.is_hardware() {
            0x10 // `MacUVCCameraExtendController::GetExpouseCurve` (`0x10019b1e0`)
        } else {
            xu1_selectors::EXPOSURE_CURVE_AND_ISO
        };
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            sel,
            259,
        )?;
        Ok(decode_exposure_curve(&data)?)
    }

    pub fn set_exposure_curve(&self, curve: &ExposureCurve) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetExpouseCurve` (`0x10019af48`): XU1 selector `0x10` (255 bytes per chunk)
            let mut buf = [0u8; 255];
            buf[0] = 0;
            buf[1] = 2;
            buf[2] = 0;
            for (i, &pt) in curve.points.iter().take(126).enumerate() {
                let off = 3 + i * 2;
                buf[off..off + 2].copy_from_slice(&pt.to_le_bytes());
            }
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x10,
                &buf,
            );
        }
        let payload = encode_exposure_curve(curve);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::EXPOSURE_CURVE_AND_ISO,
            &payload,
        )
    }

    pub fn get_auto_exposure(&self) -> Result<bool, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetAutoExposureStatus` (`0x10019a978`): XU1 selector `0x1e` (2=Auto, 1=Manual)
            if let Ok(b) = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x1e,
                1,
            ) {
                return Ok(b.first().copied().unwrap_or(2) == 2);
            }
        }
        Ok(self.get_exposure_curve()?.mode != 0)
    }

    pub fn set_auto_exposure(&self, auto: bool) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::EnableAutoExposure` (`0x10019a8fc`): XU1 selector `0x1e` (2=Auto, 1=Manual)
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x1e,
                &[if auto { 2 } else { 1 }],
            );
        }
        let mut curve = self.get_exposure_curve().unwrap_or_default();
        curve.mode = u8::from(auto);
        self.set_exposure_curve(&curve)
    }

    pub fn get_iso(&self) -> Result<u16, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetISOValue` (`0x10019a7cc`): XU1 selector `0x19`
            let b = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit_id,
                0x19,
                8,
            )?;
            if b.len() >= 2 {
                return Ok(u16::from_le_bytes([b[0], b[1]]));
            }
        }
        Ok(self.get_exposure_curve()?.iso)
    }

    pub fn set_iso(&self, iso: u16) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetISOValue` (`0x10019a758`): XU1 selector `0x19`, 8 bytes
            let mut buf = [0u8; 8];
            buf[0..2].copy_from_slice(&iso.to_le_bytes());
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x19,
                &buf,
            );
        }
        let mut curve = self.get_exposure_curve().unwrap_or_default();
        curve.iso = iso;
        self.set_exposure_curve(&curve)
    }

    pub fn get_exposure_compensation(&self) -> Result<f32, TransportError> {
        let sel = if self.is_hardware() {
            0x09 // `MacUVCCameraExtendController::GetExposureCompensation` (`0x100199038`)
        } else {
            xu1_selectors::EXPOSURE_COMPENSATION
        };
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            sel,
            2,
        )?;
        if self.is_hardware() {
            if data.len() >= 2 {
                let raw = i16::from_le_bytes([data[0], data[1]]);
                return Ok((raw as f32) / 100.0_f32);
            } else if let Some(&b) = data.first() {
                return Ok((b as i8 as f32) / 10.0_f32);
            }
            return Ok(0.0);
        }
        Ok(decode_exposure_compensation(&data)?)
    }

    pub fn set_exposure_compensation(&self, ev: f32) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetExposureCompensation` (`0x100198f34`):
            // XU1 selector `0x09`, 2-byte signed `i16_le` = `(ev * 100.0) as i16`
            let raw = (ev.clamp(-3.0_f32, 3.0_f32) * 100.0_f32).round() as i16;
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x09,
                &raw.to_le_bytes(),
            );
        }
        let payload = encode_exposure_compensation(ev);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::EXPOSURE_COMPENSATION,
            &payload,
        )
    }

    pub fn get_audio_noise_reduction(&self) -> Result<bool, TransportError> {
        let sel = if self.is_hardware() {
            0x07 // `MacUVCCameraExtendController::GetAudioNoiseStatus` (`0x100197b38`)
        } else {
            xu1_selectors::AUDIO_CONTROL
        };
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            sel,
            2,
        )?;
        Ok(data.first().copied().unwrap_or(0) != 0)
    }

    pub fn set_audio_noise_reduction(&self, enable: bool) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::EnableAudioNoiseReduction` (`0x100197ac8`): XU1 selector `0x07`, 1 byte
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x07,
                &[u8::from(enable)],
            );
        }
        let payload = encode_audio_control(u8::from(enable));
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::AUDIO_CONTROL,
            &payload,
        )
    }

    pub fn get_audio_capture_mode(&self) -> Result<AudioCaptureMode, TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::GetAudioCaptureMode` (`0x10019bc5c`): XU2 selector `0x14`
            let data = self.transport.get_req(
                UvcRequest::GetCur,
                self.unit_ids().extension_unit2_id,
                0x14,
                1,
            )?;
            return Ok(AudioCaptureMode::from_u8(data.first().copied().unwrap_or(1)));
        }
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit_id,
            xu1_selectors::AUDIO_CONTROL,
            2,
        )?;
        Ok(AudioCaptureMode::from_u8(data.first().copied().unwrap_or(1)))
    }

    pub fn set_audio_capture_mode(&self, mode: AudioCaptureMode) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetAudioCaptureMode` (`0x10019bcd4`): XU2 selector `0x14`
            return self.transport.set_cur(
                self.unit_ids().extension_unit2_id,
                0x14,
                &[mode as u8],
            );
        }
        let payload = encode_audio_control(mode as u8);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::AUDIO_CONTROL,
            &payload,
        )
    }

    pub fn get_filter(&self) -> Result<FilterMode, TransportError> {
        let sel = if self.is_hardware() {
            0x16 // `MacUVCCameraExtendController::GetFilter` (`0x10019bdbc`)
        } else {
            xu2_selectors::FILTER_MODE
        };
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            sel,
            1,
        )?;
        Ok(FilterMode::from_u8(data.first().copied().unwrap_or(0)))
    }

    pub fn set_filter(&self, mode: FilterMode) -> Result<(), TransportError> {
        let sel = if self.is_hardware() {
            0x16 // `MacUVCCameraExtendController::SetFilter` (`0x10019bd4c`)
        } else {
            xu2_selectors::FILTER_MODE
        };
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            sel,
            &[mode as u8],
        )
    }

    pub fn get_privacy_mode(&self) -> Result<(i32, bool), TransportError> {
        let sel = if self.is_hardware() {
            0x0f // `MacUVCCameraExtendController::GetPrivacyMode` (`0x10019b500`)
        } else {
            xu2_selectors::PRIVACY_AND_FILL_LIGHT
        };
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            sel,
            4,
        )?;
        Ok(decode_privacy_mode(&data)?)
    }

    pub fn set_privacy_mode(&self, enable: bool) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetPrivacyMode` (`0x10019b3a4`): XU2 selector `0x0f`, 1 byte
            return self.transport.set_cur(
                self.unit_ids().extension_unit2_id,
                0x0f,
                &[u8::from(enable)],
            );
        }
        let payload = encode_set_privacy_mode(enable);
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::PRIVACY_AND_FILL_LIGHT,
            &payload,
        )
    }

    pub fn set_host_ptz(&self, info: &HostPtzInfo) -> Result<(), TransportError> {
        let sel = if self.is_hardware() {
            0x13 // `MacUVCCameraExtendController::SetHostPTZ` (`0x10019b600`)
        } else {
            xu2_selectors::HOST_PTZ
        };
        let payload = encode_host_ptz(info);
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            sel,
            &payload,
        )
    }

    pub fn get_horizontal_correction(&self) -> Result<bool, TransportError> {
        let sel = if self.is_hardware() {
            0x1c // `MacUVCCameraExtendController::GetEnableHorizontalCorrection` (`0x10019b484`)
        } else {
            xu2_selectors::HORIZONTAL_CORRECTION
        };
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            sel,
            1,
        )?;
        Ok(data.first().copied().unwrap_or(0) != 0)
    }

    pub fn set_horizontal_correction(&self, enable: bool) -> Result<(), TransportError> {
        let sel = if self.is_hardware() {
            0x1c // `MacUVCCameraExtendController::EnableHorizontalCorrection` (`0x10019b414`)
        } else {
            xu2_selectors::HORIZONTAL_CORRECTION
        };
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            sel,
            &[u8::from(enable)],
        )
    }

    pub fn get_default_track_mode(&self) -> Result<u8, TransportError> {
        let sel = if self.is_hardware() {
            0x1a // `MacUVCCameraExtendController::GetDefaultTrackMode` (`0x10019c228`)
        } else {
            xu2_selectors::DEFAULT_TRACK_MODE
        };
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            sel,
            1,
        )?;
        Ok(data.first().copied().unwrap_or(0))
    }

    pub fn set_default_track_mode(&self, mode: u8) -> Result<(), TransportError> {
        let sel = if self.is_hardware() {
            0x1a // `MacUVCCameraExtendController::SetDefaultTrackMode` (`0x10019c1b8`)
        } else {
            xu2_selectors::DEFAULT_TRACK_MODE
        };
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            sel,
            &[mode],
        )
    }

    pub fn select_track_object_norm(&self, x: f32, y: f32, w: f32, h: f32) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SetTrackObj` (`0x1001987a0`):
            // XU1 selector `0x15` (`21`): 8 bytes `[x_f32_le, y_f32_le]` on Link 1 (`0x4c01`),
            // or 16 bytes `[x_f32_le, y_f32_le, w_f32_le, h_f32_le]` on Link 2 / 2C (`0x4c03..0x4c06`).
            if self.model() == CameraModel::Link {
                let mut buf = [0u8; 8];
                buf[0..4].copy_from_slice(&x.to_le_bytes());
                buf[4..8].copy_from_slice(&y.to_le_bytes());
                return self.transport.set_cur(
                    self.unit_ids().extension_unit_id,
                    0x15,
                    &buf,
                );
            } else {
                let mut buf = [0u8; 16];
                buf[0..4].copy_from_slice(&x.to_le_bytes());
                buf[4..8].copy_from_slice(&y.to_le_bytes());
                buf[8..12].copy_from_slice(&w.to_le_bytes());
                buf[12..16].copy_from_slice(&h.to_le_bytes());
                return self.transport.set_cur(
                    self.unit_ids().extension_unit_id,
                    0x15,
                    &buf,
                );
            }
        }
        let rect = UvcRect {
            x: (x * 10000.0) as i16,
            y: (y * 10000.0) as i16,
            width: (w * 10000.0) as i16,
            height: (h * 10000.0) as i16,
            track_id: 0,
        };
        let payload = encode_track_rect(&rect);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACK_OBJECT_SELECT,
            &payload,
        )
    }

    pub fn select_track_object(&self, rect: &UvcRect) -> Result<(), TransportError> {
        if self.is_hardware() {
            let cx = if rect.width > 0 {
                ((rect.x as f32) + (rect.width as f32) * 0.5_f32) / 10000.0_f32
            } else {
                (rect.x as f32) / 10000.0_f32
            };
            let cy = if rect.height > 0 {
                ((rect.y as f32) + (rect.height as f32) * 0.5_f32) / 10000.0_f32
            } else {
                (rect.y as f32) / 10000.0_f32
            };
            let w = (rect.width as f32) / 10000.0_f32;
            let h = (rect.height as f32) / 10000.0_f32;
            return self.select_track_object_norm(cx, cy, w, h);
        }
        let payload = encode_track_rect(rect);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACK_OBJECT_SELECT,
            &payload,
        )
    }

    pub fn get_track_objects(&self) -> Result<Vec<UvcRect>, TransportError> {
        if self.is_hardware() {
            if self.model() == CameraModel::Link {
                // `MacUVCCameraExtendController::GetTrackObjLists` (`0x100198d94`):
                // XU1 selector `0x14` (`20`), 240 bytes: `data[0] = count (<= 10)`, followed by
                // `count` 16-byte `UVCRect` structs `[x_f32_le, y_f32_le, w_f32_le, h_f32_le]` at `data[1..]`.
                let data = self.transport.get_req(
                    UvcRequest::GetCur,
                    self.unit_ids().extension_unit_id,
                    0x14,
                    240,
                )?;
                let count = data.first().copied().unwrap_or(0).min(10) as usize;
                let mut rects = Vec::with_capacity(count);
                for i in 0..count {
                    let off = 1 + i * 16;
                    if off + 16 > data.len() {
                        break;
                    }
                    let x = f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
                    let y = f32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
                    let w = f32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
                    let h = f32::from_le_bytes([data[off + 12], data[off + 13], data[off + 14], data[off + 15]]);
                    if w > 0.0_f32 || h > 0.0_f32 {
                        rects.push(UvcRect {
                            x: (x * 10000.0_f32) as i16,
                            y: (y * 10000.0_f32) as i16,
                            width: (w * 10000.0_f32) as i16,
                            height: (h * 10000.0_f32) as i16,
                            track_id: i as i16,
                        });
                    }
                }
                return Ok(rects);
            } else {
                // `MacUVCCameraExtendController::GetNewTrackObjLists` (`0x100198880`):
                // XU2 selector `0x18` (`24`), `data[0] = count (<= 6)`, followed by
                // `count` 8-byte `u16[4]` structs `[x_u16, y_u16, w_u16, h_u16]` in `0..=10000` at `data[1..]`.
                let data = self.transport.get_req(
                    UvcRequest::GetCur,
                    self.unit_ids().extension_unit2_id,
                    0x18,
                    64,
                )?;
                let count = data.first().copied().unwrap_or(0).min(6) as usize;
                let mut rects = Vec::with_capacity(count);
                for i in 0..count {
                    let off = 1 + i * 8;
                    if off + 8 > data.len() {
                        break;
                    }
                    let x = u16::from_le_bytes([data[off], data[off + 1]]) as i16;
                    let y = u16::from_le_bytes([data[off + 2], data[off + 3]]) as i16;
                    let w = u16::from_le_bytes([data[off + 4], data[off + 5]]) as i16;
                    let h = u16::from_le_bytes([data[off + 6], data[off + 7]]) as i16;
                    if w > 0 || h > 0 {
                        rects.push(UvcRect {
                            x,
                            y,
                            width: w,
                            height: h,
                            track_id: i as i16,
                        });
                    }
                }
                return Ok(rects);
            }
        }
        let (unit, sel) = if self.model() == CameraModel::Link {
            (self.unit_ids().extension_unit_id, xu1_selectors::TRACK_OBJECT_LIST_V1)
        } else {
            (self.unit_ids().extension_unit2_id, xu2_selectors::TRACK_OBJECT_LIST_V2)
        };
        let len = self.transport.get_len(unit, sel).unwrap_or(20) as usize;
        let data = self.transport.get_req(UvcRequest::GetCur, unit, sel, len.max(10))?;
        Ok(decode_track_rect_list(&data))
    }

    pub fn get_image_template_info(&self) -> Result<ImageTemplateInfo, TransportError> {
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            xu2_selectors::IMAGE_TEMPLATE_INFO,
            11,
        )?;
        Ok(decode_image_template_info(&data)?)
    }

    pub fn set_image_template(&self, template_id: i8, op: ImageOperation) -> Result<(), TransportError> {
        let info = self.get_image_template_info().unwrap_or_default();
        let payload = encode_set_image_template(template_id, op, info.bitmask, info.max_templates.max(10))?;
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::IMAGE_TEMPLATE_INFO,
            &payload,
        )
    }

    pub fn delete_image_template(&self, template_id: i8) -> Result<(), TransportError> {
        let info = self.get_image_template_info()?;
        let payload = encode_delete_image_template(template_id, &info)?;
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::IMAGE_TEMPLATE_INFO,
            &payload,
        )
    }

    pub fn set_image_template_name(&self, template_id: i8, name: &str) -> Result<(), TransportError> {
        let payload = encode_image_template_name(template_id, name);
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::IMAGE_TEMPLATE_NAME,
            &payload,
        )
    }

    pub fn switch_msc_readonly_class(&self) -> Result<(), TransportError> {
        if self.is_hardware() {
            // `MacUVCCameraExtendController::SwitchStorageMode` (`0x10019a5ac`): XU1 selector `0x0a`, `[0x02]`
            return self.transport.set_cur(
                self.unit_ids().extension_unit_id,
                0x0a,
                &[0x02],
            );
        }
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::USB_MODE_SWITCH,
            &[0x02, 0x00, 0x00, 0x00],
        )
    }

    pub fn get_image_template_name(&self, template_id: i8) -> Result<String, TransportError> {
        let mut req_buf = [0u8; 65];
        req_buf[0] = template_id as u8;
        let _ = self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::IMAGE_TEMPLATE_NAME,
            &req_buf[..1],
        );
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            xu2_selectors::IMAGE_TEMPLATE_NAME,
            65,
        )?;
        let (_, name) = decode_image_template_name(&data)?;
        Ok(name)
    }

    pub fn force_smart_whiteboard(&self, mode: u8) -> Result<(), TransportError> {
        let payload = encode_force_smart_whiteboard(mode);
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::TRACKING_AND_GESTURE,
            &payload,
        )
    }

    pub fn get_gesture_binding_map(&self) -> Result<[(Gesture, GestureFunction); 5], TransportError> {
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            xu2_selectors::GESTURE_BINDING_MAP,
            16,
        )?;
        Ok(decode_gesture_binding_map(&data)?)
    }

    pub fn set_gesture_binding_func(&self, gesture: Gesture, func: GestureFunction) -> Result<(), TransportError> {
        let payload = encode_gesture_binding(gesture, func);
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::GESTURE_BINDING_MAP,
            &payload,
        )
    }

    pub fn enter_track_forbidden_area_setting(&self) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            &encode_enter_track_forbidden_area_setting(),
        )
    }

    pub fn exit_track_forbidden_area_setting(&self) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().extension_unit_id,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            &encode_exit_track_forbidden_area_setting(),
        )
    }

    pub fn get_track_forbidden_areas(&self) -> Result<Vec<ForbiddenAreaInfo>, TransportError> {
        let len = self
            .transport
            .get_len(self.unit_ids().extension_unit2_id, xu2_selectors::TRACK_FORBIDDEN_AREA)
            .unwrap_or(40) as usize;
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            xu2_selectors::TRACK_FORBIDDEN_AREA,
            len.max(10),
        )?;
        Ok(decode_forbidden_areas(&data))
    }

    pub fn set_track_forbidden_areas(&self, areas: &[ForbiddenAreaInfo]) -> Result<(), TransportError> {
        let payload = encode_forbidden_areas(areas);
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::TRACK_FORBIDDEN_AREA,
            &payload,
        )
    }

    pub fn set_track_forbidden_area_enabled(&self, enable: bool) -> Result<(), TransportError> {
        self.set_extend_function(ExtendFunction::TrackForbiddenArea, enable)
    }

    pub fn get_fill_light_param(&self) -> Result<(i32, i32), TransportError> {
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            xu2_selectors::PRIVACY_AND_FILL_LIGHT,
            4,
        )?;
        let mode = data.get(2).copied().unwrap_or(0) as i32;
        let level = data.get(3).copied().unwrap_or(0) as i32;
        Ok((mode, level))
    }

    pub fn get_real_zoom_value(&self) -> Result<u16, TransportError> {
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            xu2_selectors::REAL_ZOOM_AND_UDISK,
            4,
        )?;
        if data.len() < 2 {
            return Ok(100);
        }
        Ok(u16::from_le_bytes([data[0], data[1]]))
    }

    pub fn get_udisk_open_count(&self) -> Result<u32, TransportError> {
        let data = self.transport.get_req(
            UvcRequest::GetCur,
            self.unit_ids().extension_unit2_id,
            xu2_selectors::REAL_ZOOM_AND_UDISK,
            4,
        )?;
        if data.len() < 4 {
            return Ok(0);
        }
        Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
    }

    pub fn set_udisk_open_count(&self, count: u32) -> Result<(), TransportError> {
        self.transport.set_cur(
            self.unit_ids().extension_unit2_id,
            xu2_selectors::REAL_ZOOM_AND_UDISK,
            &count.to_le_bytes(),
        )
    }
}
