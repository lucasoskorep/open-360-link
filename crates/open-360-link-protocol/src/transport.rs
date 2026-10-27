use crate::codec::UvcControlPacketHeader;
use crate::ids::*;
use crate::types::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("USB error: {0}")]
    Usb(String),
    #[error("camera not found")]
    DeviceNotFound,
    #[error("UVC control unit {0} not available on this device")]
    MissingUnit(&'static str),
    #[error("codec error: {0}")]
    Codec(#[from] crate::codec::CodecError),
}

/// Abstract cross-platform UVC Control Transfer transport.
pub trait UvcControlTransport: Send + Sync {
    /// Returns the discovered metadata for the connected camera.
    fn camera_info(&self) -> &DiscoveredCamera;

    /// Performs a UVC `SET_CUR` (`bmRequestType = 0x21`, `bRequest = 0x01`) control transfer.
    fn set_cur(&self, unit_id: u8, selector: u8, data: &[u8]) -> Result<(), TransportError>;

    /// Performs a UVC `GET_*` (`bmRequestType = 0xa1`, `bRequest = 0x81..0x87`) control transfer.
    fn get_req(
        &self,
        req: UvcRequest,
        unit_id: u8,
        selector: u8,
        len: usize,
    ) -> Result<Vec<u8>, TransportError>;

    /// Queries `GET_LEN` (`0x85`) for a variable-length UVC control selector.
    fn get_len(&self, unit_id: u8, selector: u8) -> Result<u16, TransportError> {
        let bytes = self.get_req(UvcRequest::GetLen, unit_id, selector, 2)?;
        if bytes.len() < 2 {
            return Ok(0);
        }
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
}

/// Enumerates all connected Insta360 Link family cameras across Linux, Windows, and macOS using `nusb`.
pub fn list_connected_cameras() -> Result<Vec<DiscoveredCamera>, TransportError> {
    let devices = nusb::list_devices().map_err(|e| TransportError::Usb(e.to_string()))?;
    let mut out = Vec::new();
    for dev in devices {
        let vid = dev.vendor_id();
        let pid = dev.product_id();
        if !is_insta360_link_device(vid, pid) {
            continue;
        }
        let model = CameraModel::from_pid(pid);
        let name = dev
            .product_string()
            .map(|s| s.to_string())
            .unwrap_or_else(|| model.display_name().to_string());
        let bus = dev.bus_number();
        let addr = dev.device_address();
        let unique_id = format!("0x{:02x}{:02x}{:04x}{:04x}", bus, addr, vid, pid);

        let mut unit_ids = UvcUnitIds::default();
        if let Ok(opened) = dev.open() {
            if let Ok(active_cfg) = opened.active_configuration() {
                // Parse raw configuration descriptor bytes to find exact UVC unit IDs
                let raw: &[u8] = active_cfg.descriptors().as_bytes();
                unit_ids = parse_uvc_vc_descriptors(raw);
            }
        }
        #[cfg(target_os = "linux")]
        if unit_ids == UvcUnitIds::default() {
            if let Some((_, parsed_ids)) = open_linux_v4l2_node(vid, pid, unit_ids.extension_unit_id) {
                unit_ids = parsed_ids;
            }
        }

        out.push(DiscoveredCamera {
            vendor_id: vid,
            product_id: pid,
            model,
            name,
            unique_id,
            bus_number: bus,
            device_address: addr,
            unit_ids,
            capabilities: DeviceCapabilities::for_pid(pid),
        });
    }
    Ok(out)
}

/// Pure-Rust cross-platform USB hardware transport backed by `nusb`
/// (Linux `uvcvideo` `UVCIOC_CTRL_QUERY` + V4L2 ioctls, Windows `WinUSB`, macOS `IOKit`).
pub struct NusbTransport {
    info: DiscoveredCamera,
    device: Option<nusb::Device>,
    interface: Option<nusb::Interface>,
    #[cfg(target_os = "linux")]
    v4l_file: std::sync::Mutex<Option<std::fs::File>>,
    timeout: Duration,
}

#[cfg(target_os = "linux")]
fn open_linux_v4l2_node(vid: u16, pid: u16, xu1_id: u8) -> Option<(std::fs::File, UvcUnitIds)> {
    let vid_hex = format!("{:04x}", vid);
    let pid_hex = format!("{:04x}", pid);
    let entries = std::fs::read_dir("/sys/class/video4linux").ok()?;
    let mut candidates: Vec<(u32, std::path::PathBuf, String)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(num_str) = name.strip_prefix("video") {
            if let Ok(num) = num_str.parse::<u32>() {
                candidates.push((num, entry.path(), name));
            }
        }
    }
    candidates.sort_by_key(|(num, _, _)| *num);

    let mut fallback: Option<(std::fs::File, UvcUnitIds)> = None;
    for (_, path, name) in candidates {
        let dev_link = path.join("device");
        let Ok(canon) = std::fs::canonicalize(&dev_link) else {
            continue;
        };
        // Parent directory of the USB interface (`B-P:1.0`) holds `idVendor`, `idProduct`, and `descriptors`.
        if let Some(usb_dev_dir) = canon.parent() {
            let v = std::fs::read_to_string(usb_dev_dir.join("idVendor"))
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            let p = std::fs::read_to_string(usb_dev_dir.join("idProduct"))
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            if v == vid_hex && p == pid_hex {
                let parsed_ids = std::fs::read(usb_dev_dir.join("descriptors"))
                    .ok()
                    .map(|raw| {
                        // Skip the 18-byte USB Device Descriptor if present at offset 0
                        let slice = if raw.len() > 18 && raw[0] == 18 && raw[1] == 1 {
                            &raw[18..]
                        } else {
                            &raw[..]
                        };
                        parse_uvc_vc_descriptors(slice)
                    })
                    .unwrap_or_default();
                let probe_xu1 = if parsed_ids.extension_unit_id != 0 {
                    parsed_ids.extension_unit_id
                } else {
                    xu1_id
                };
                let dev_path = format!("/dev/{}", name);
                if let Ok(file) = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&dev_path)
                {
                    // Verify this V4L2 node is the primary video capture node (which supports
                    // UVCIOC_CTRL_QUERY), not the secondary metadata node (/dev/video1).
                    let mut len_buf = [0u8; 2];
                    if linux_uvcioc_ctrl_query(
                        &file,
                        probe_xu1,
                        0x03,
                        UvcRequest::GetLen as u8,
                        &mut len_buf,
                    )
                    .is_ok()
                    {
                        return Some((file, parsed_ids));
                    }
                    if fallback.is_none() {
                        fallback = Some((file, parsed_ids));
                    }
                }
            }
        }
    }
    fallback
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct LinuxUvcXuControlQuery {
    unit: u8,
    selector: u8,
    query: u8,
    size: u16,
    data: *mut u8,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct LinuxV4l2Control {
    id: u32,
    value: i32,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct LinuxV4l2QueryCtrl {
    id: u32,
    ctrl_type: u32,
    name: [u8; 32],
    minimum: i32,
    maximum: i32,
    step: i32,
    default_value: i32,
    flags: u32,
    reserved: [u32; 2],
}

#[cfg(target_os = "linux")]
const UVCIOC_CTRL_QUERY: libc::c_ulong = 0xC010_7521;
#[cfg(target_os = "linux")]
const VIDIOC_G_CTRL: libc::c_ulong = 0xC008_561B;
#[cfg(target_os = "linux")]
const VIDIOC_S_CTRL: libc::c_ulong = 0xC008_561C;
#[cfg(target_os = "linux")]
const VIDIOC_QUERYCTRL: libc::c_ulong = 0xC044_5624;

#[cfg(target_os = "linux")]
const V4L2_CID_BRIGHTNESS: u32 = 0x0098_0900;
#[cfg(target_os = "linux")]
const V4L2_CID_CONTRAST: u32 = 0x0098_0901;
#[cfg(target_os = "linux")]
const V4L2_CID_SATURATION: u32 = 0x0098_0902;
#[cfg(target_os = "linux")]
const V4L2_CID_AUTO_WHITE_BALANCE: u32 = 0x0098_090C;
#[cfg(target_os = "linux")]
const V4L2_CID_POWER_LINE_FREQUENCY: u32 = 0x0098_0918;
#[cfg(target_os = "linux")]
const V4L2_CID_WHITE_BALANCE_TEMPERATURE: u32 = 0x0098_091A;
#[cfg(target_os = "linux")]
const V4L2_CID_SHARPNESS: u32 = 0x0098_091B;

#[cfg(target_os = "linux")]
const V4L2_CID_PAN_ABSOLUTE: u32 = 0x009A_0908;
#[cfg(target_os = "linux")]
const V4L2_CID_TILT_ABSOLUTE: u32 = 0x009A_0909;
#[cfg(target_os = "linux")]
const V4L2_CID_FOCUS_ABSOLUTE: u32 = 0x009A_090A;
#[cfg(target_os = "linux")]
const V4L2_CID_FOCUS_AUTO: u32 = 0x009A_090C;
#[cfg(target_os = "linux")]
const V4L2_CID_ZOOM_ABSOLUTE: u32 = 0x009A_090D;

#[cfg(target_os = "linux")]
fn linux_uvc_to_v4l2_cid(unit_ids: &UvcUnitIds, unit_id: u8, selector: u8) -> Option<u32> {
    if unit_id == unit_ids.processing_unit_id {
        match selector {
            pu_selectors::BRIGHTNESS => Some(V4L2_CID_BRIGHTNESS),
            pu_selectors::CONTRAST => Some(V4L2_CID_CONTRAST),
            pu_selectors::SATURATION => Some(V4L2_CID_SATURATION),
            pu_selectors::SHARPNESS => Some(V4L2_CID_SHARPNESS),
            pu_selectors::WHITE_BALANCE_TEMPERATURE => Some(V4L2_CID_WHITE_BALANCE_TEMPERATURE),
            pu_selectors::WHITE_BALANCE_TEMPERATURE_AUTO => Some(V4L2_CID_AUTO_WHITE_BALANCE),
            pu_selectors::POWER_LINE_FREQUENCY => Some(V4L2_CID_POWER_LINE_FREQUENCY),
            _ => None,
        }
    } else if unit_id == unit_ids.input_terminal_id {
        match selector {
            ct_selectors::ZOOM_ABSOLUTE => Some(V4L2_CID_ZOOM_ABSOLUTE),
            ct_selectors::FOCUS_ABSOLUTE => Some(V4L2_CID_FOCUS_ABSOLUTE),
            ct_selectors::FOCUS_AUTO => Some(V4L2_CID_FOCUS_AUTO),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn linux_v4l2_set_ctrl(file: &std::fs::File, id: u32, value: i32) -> Result<(), TransportError> {
    use std::os::unix::io::AsRawFd;
    let mut ctrl = LinuxV4l2Control { id, value };
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), VIDIOC_S_CTRL, &mut ctrl) };
    if rc < 0 {
        Err(TransportError::Usb(
            std::io::Error::last_os_error().to_string(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn linux_v4l2_get_ctrl(file: &std::fs::File, id: u32) -> Result<i32, TransportError> {
    use std::os::unix::io::AsRawFd;
    let mut ctrl = LinuxV4l2Control { id, value: 0 };
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), VIDIOC_G_CTRL, &mut ctrl) };
    if rc < 0 {
        Err(TransportError::Usb(
            std::io::Error::last_os_error().to_string(),
        ))
    } else {
        Ok(ctrl.value)
    }
}

#[cfg(target_os = "linux")]
fn linux_v4l2_query_ctrl(
    file: &std::fs::File,
    id: u32,
) -> Result<LinuxV4l2QueryCtrl, TransportError> {
    use std::os::unix::io::AsRawFd;
    let mut q = LinuxV4l2QueryCtrl {
        id,
        ctrl_type: 0,
        name: [0u8; 32],
        minimum: 0,
        maximum: 0,
        step: 0,
        default_value: 0,
        flags: 0,
        reserved: [0u32; 2],
    };
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), VIDIOC_QUERYCTRL, &mut q) };
    if rc < 0 {
        Err(TransportError::Usb(
            std::io::Error::last_os_error().to_string(),
        ))
    } else {
        Ok(q)
    }
}

#[cfg(target_os = "linux")]
fn linux_uvcioc_ctrl_query(
    file: &std::fs::File,
    unit: u8,
    selector: u8,
    query: u8,
    buf: &mut [u8],
) -> Result<(), TransportError> {
    use std::os::unix::io::AsRawFd;
    let mut q = LinuxUvcXuControlQuery {
        unit,
        selector,
        query,
        size: buf.len() as u16,
        data: buf.as_mut_ptr(),
    };
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), UVCIOC_CTRL_QUERY, &mut q) };
    if rc < 0 {
        Err(TransportError::Usb(
            std::io::Error::last_os_error().to_string(),
        ))
    } else {
        Ok(())
    }
}

impl NusbTransport {
    pub fn open_first() -> Result<Self, TransportError> {
        let cameras = list_connected_cameras()?;
        let first = cameras.into_iter().next().ok_or(TransportError::DeviceNotFound)?;
        Self::open_by_info(first)
    }

    pub fn open_by_info(mut info: DiscoveredCamera) -> Result<Self, TransportError> {
        let dev_info = nusb::list_devices()
            .map_err(|e| TransportError::Usb(e.to_string()))?
            .find(|d| {
                d.vendor_id() == info.vendor_id
                    && d.product_id() == info.product_id
                    && d.bus_number() == info.bus_number
                    && d.device_address() == info.device_address
            })
            .ok_or(TransportError::DeviceNotFound)?;

        let device = dev_info.open().ok();
        if let Some(ref dev) = device {
            if let Ok(cfg) = dev.active_configuration() {
                let raw: &[u8] = cfg.descriptors().as_bytes();
                info.unit_ids = parse_uvc_vc_descriptors(raw);
            }
        }

        // Claim the UVC VideoControl interface without detaching the OS kernel UVC video streaming
        // driver (`uvcvideo` on Linux, `UVCAssistant` on macOS, `usbvideo.sys` on Windows) so that
        // live webcam video capture and UVC/XU control transfers operate simultaneously.
        let interface = device
            .as_ref()
            .and_then(|d| d.claim_interface(info.unit_ids.control_interface).ok());

        #[cfg(target_os = "linux")]
        let v4l_file = match open_linux_v4l2_node(
            info.vendor_id,
            info.product_id,
            info.unit_ids.extension_unit_id,
        ) {
            Some((f, parsed_ids)) => {
                if device.is_none() {
                    info.unit_ids = parsed_ids;
                }
                Some(f)
            }
            None => None,
        };

        #[cfg(target_os = "linux")]
        if device.is_none() && v4l_file.is_none() {
            return Err(TransportError::Usb(
                "Could not open V4L2 device node or USB device".to_string(),
            ));
        }

        #[cfg(not(target_os = "linux"))]
        if device.is_none() {
            return Err(TransportError::Usb(
                "Could not open USB device".to_string(),
            ));
        }

        Ok(Self {
            info,
            device,
            interface,
            #[cfg(target_os = "linux")]
            v4l_file: std::sync::Mutex::new(v4l_file),
            timeout: Duration::from_millis(1500),
        })
    }

    #[cfg(target_os = "linux")]
    fn with_v4l_file<R, F>(&self, mut op: F) -> Option<R>
    where
        F: FnMut(&std::fs::File) -> Result<R, TransportError>,
    {
        let mut guard = self.v4l_file.lock().ok()?;
        if let Some(ref f) = *guard {
            if let Ok(res) = op(f) {
                return Some(res);
            }
        }
        // Re-open V4L2 node if the camera re-enumerated over USB (e.g., when switching
        // VerticalMode / High Frame Rate UVC descriptors).
        if let Some((new_f, _)) = open_linux_v4l2_node(
            self.info.vendor_id,
            self.info.product_id,
            self.info.unit_ids.extension_unit_id,
        ) {
            let res = op(&new_f).ok();
            *guard = Some(new_f);
            return res;
        }
        None
    }
}

impl UvcControlTransport for NusbTransport {
    fn camera_info(&self) -> &DiscoveredCamera {
        &self.info
    }

    fn set_cur(&self, unit_id: u8, selector: u8, data: &[u8]) -> Result<(), TransportError> {
        let is_xu = unit_id == self.info.unit_ids.extension_unit_id
            || unit_id == self.info.unit_ids.extension_unit2_id;

        // Match `MacUVCCameraExtendController::SendCommand` (`0x100195db4`): query `GET_LEN (0x85)`
        // for Extension Unit selectors and zero-pad the payload to the descriptor's exact wLength.
        let padded_buf: Vec<u8>;
        let payload: &[u8] = if is_xu {
            if let Ok(uvc_len) = self.get_len(unit_id, selector) {
                let target_len = uvc_len as usize;
                if target_len > 0 && target_len != data.len() {
                    let mut b = vec![0u8; target_len];
                    let copy_n = data.len().min(target_len);
                    b[..copy_n].copy_from_slice(&data[..copy_n]);
                    padded_buf = b;
                    &padded_buf
                } else {
                    data
                }
            } else {
                data
            }
        } else {
            data
        };

        #[cfg(target_os = "linux")]
        if self
            .with_v4l_file(|v4l| {
                if is_xu {
                    let mut buf = payload.to_vec();
                    linux_uvcioc_ctrl_query(
                        v4l,
                        unit_id,
                        selector,
                        UvcRequest::SetCur as u8,
                        &mut buf,
                    )
                } else if unit_id == self.info.unit_ids.input_terminal_id
                    && selector == ct_selectors::PANTILT_ABSOLUTE
                    && payload.len() >= 8
                {
                    let pan = i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
                    let tilt = i32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
                    let r1 = linux_v4l2_set_ctrl(v4l, V4L2_CID_PAN_ABSOLUTE, pan);
                    let r2 = linux_v4l2_set_ctrl(v4l, V4L2_CID_TILT_ABSOLUTE, tilt);
                    r1.or(r2)
                } else if let Some(cid) =
                    linux_uvc_to_v4l2_cid(&self.info.unit_ids, unit_id, selector)
                {
                    let val: i32 = match payload.len() {
                        1 => payload[0] as i32,
                        2 => {
                            if cid == V4L2_CID_BRIGHTNESS {
                                i16::from_le_bytes([payload[0], payload[1]]) as i32
                            } else {
                                u16::from_le_bytes([payload[0], payload[1]]) as i32
                            }
                        }
                        4 => i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]),
                        _ => 0,
                    };
                    if linux_v4l2_set_ctrl(v4l, cid, val).is_ok() {
                        return Ok(());
                    }
                    if cid == V4L2_CID_POWER_LINE_FREQUENCY && val == 3 {
                        return linux_v4l2_set_ctrl(v4l, cid, 2);
                    }
                    Err(TransportError::Usb("V4L2 set_ctrl failed".to_string()))
                } else {
                    Err(TransportError::Usb("unmapped V4L2 control".to_string()))
                }
            })
            .is_some()
        {
            return Ok(());
        }

        let hdr = UvcControlPacketHeader::new(
            UvcRequest::SetCur,
            selector,
            unit_id,
            self.info.unit_ids.control_interface,
        );
        let control = nusb::transfer::Control {
            control_type: nusb::transfer::ControlType::Class,
            recipient: nusb::transfer::Recipient::Interface,
            request: hdr.b_request,
            value: hdr.w_value,
            index: hdr.w_index,
        };
        if let Some(iface) = &self.interface {
            iface
                .control_out_blocking(control, payload, self.timeout)
                .map(|_| ())
                .map_err(|e| TransportError::Usb(e.to_string()))
        } else {
            #[cfg(not(target_os = "windows"))]
            if let Some(dev) = &self.device {
                return dev
                    .control_out_blocking(control, payload, self.timeout)
                    .map(|_| ())
                    .map_err(|e| TransportError::Usb(e.to_string()));
            }
            Err(TransportError::Usb(
                "UVC control transfer failed".to_string(),
            ))
        }
    }

    fn get_req(
        &self,
        req: UvcRequest,
        unit_id: u8,
        selector: u8,
        len: usize,
    ) -> Result<Vec<u8>, TransportError> {
        let is_xu = unit_id == self.info.unit_ids.extension_unit_id
            || unit_id == self.info.unit_ids.extension_unit2_id;

        // Match `MacUVCCameraExtendController::GetCommand` (`0x1001963d0`): query `GET_LEN (0x85)`
        // first on Extension Unit `GET_CUR` requests so the buffer matches the firmware's wLength.
        let actual_len = if is_xu && req == UvcRequest::GetCur {
            match self.get_len(unit_id, selector) {
                Ok(uvc_len) if uvc_len > 0 => uvc_len as usize,
                _ => len,
            }
        } else {
            len
        };

        #[cfg(target_os = "linux")]
        if let Some(out) = self.with_v4l_file(|v4l| {
            if is_xu {
                let mut buf = vec![0u8; actual_len];
                linux_uvcioc_ctrl_query(v4l, unit_id, selector, req as u8, &mut buf)?;
                Ok(buf)
            } else if unit_id == self.info.unit_ids.input_terminal_id
                && selector == ct_selectors::PANTILT_ABSOLUTE
                && req == UvcRequest::GetCur
            {
                let pan = linux_v4l2_get_ctrl(v4l, V4L2_CID_PAN_ABSOLUTE)?;
                let tilt = linux_v4l2_get_ctrl(v4l, V4L2_CID_TILT_ABSOLUTE)?;
                let mut out = vec![0u8; 8];
                out[0..4].copy_from_slice(&pan.to_le_bytes());
                out[4..8].copy_from_slice(&tilt.to_le_bytes());
                Ok(out)
            } else if let Some(cid) =
                linux_uvc_to_v4l2_cid(&self.info.unit_ids, unit_id, selector)
            {
                let val: i32 = match req {
                    UvcRequest::GetCur => linux_v4l2_get_ctrl(v4l, cid)?,
                    UvcRequest::GetMin => linux_v4l2_query_ctrl(v4l, cid)?.minimum,
                    UvcRequest::GetMax => linux_v4l2_query_ctrl(v4l, cid)?.maximum,
                    UvcRequest::GetRes => linux_v4l2_query_ctrl(v4l, cid)?.step,
                    UvcRequest::GetDef => linux_v4l2_query_ctrl(v4l, cid)?.default_value,
                    _ => return Err(TransportError::Usb("unsupported V4L2 request".to_string())),
                };
                Ok(match actual_len {
                    1 => vec![val as u8],
                    2 => (val as i16).to_le_bytes().to_vec(),
                    4 => val.to_le_bytes().to_vec(),
                    _ => {
                        let mut b = vec![0u8; actual_len];
                        let le = val.to_le_bytes();
                        let n = actual_len.min(4);
                        b[..n].copy_from_slice(&le[..n]);
                        b
                    }
                })
            } else {
                Err(TransportError::Usb("unmapped V4L2 control".to_string()))
            }
        }) {
            return Ok(out);
        }

        let hdr = UvcControlPacketHeader::new(
            req,
            selector,
            unit_id,
            self.info.unit_ids.control_interface,
        );
        let mut buf = vec![0u8; actual_len];
        let control = nusb::transfer::Control {
            control_type: nusb::transfer::ControlType::Class,
            recipient: nusb::transfer::Recipient::Interface,
            request: hdr.b_request,
            value: hdr.w_value,
            index: hdr.w_index,
        };
        let n = if let Some(iface) = &self.interface {
            iface
                .control_in_blocking(control, &mut buf, self.timeout)
                .map_err(|e| TransportError::Usb(e.to_string()))?
        } else {
            #[cfg(not(target_os = "windows"))]
            if let Some(dev) = &self.device {
                let n = dev
                    .control_in_blocking(control, &mut buf, self.timeout)
                    .map_err(|e| TransportError::Usb(e.to_string()))?;
                buf.truncate(n);
                return Ok(buf);
            }
            return Err(TransportError::Usb(
                "UVC control transfer failed".to_string(),
            ));
        };
        buf.truncate(n);
        Ok(buf)
    }
}

/// Stateful in-memory simulated Insta360 Link / Link 2 / Link 2C camera transport.
/// Ideal for testing, CI, and interactive GUI exploration when no physical webcam is plugged in.
#[derive(Clone)]
pub struct MockCameraTransport {
    info: DiscoveredCamera,
    state: Arc<Mutex<MockState>>,
}

#[derive(Debug, Clone)]
struct MockState {
    registers: HashMap<(u8, u8), Vec<u8>>,
    pan_arcsec: i32,
    tilt_arcsec: i32,
    zoom: u16,
    roll: i16,
    focus: u16,
    auto_focus: bool,
    brightness: i16,
    contrast: u16,
    saturation: u16,
    sharpness: u16,
    wb_kelvin: u16,
    wb_auto: bool,
    power_line: u8,
    shutter: u32,
    extend_bits: u32,
    video_mode: u8,
    video_aux: [u8; 36],
    track_speed: u8,
    composition_style: u8,
    gestures: [u8; 6],
    audio_denoise: bool,
    audio_mode: u8,
    ev_comp: f32,
    iso: u16,
    auto_exposure: bool,
    filter_mode: u8,
    privacy_mode: bool,
    horiz_correction: bool,
    default_track_mode: u8,
    preset_mask: u8,
    current_preset: u8,
    template_mask: u64,
    current_template: i8,
    template_names: HashMap<i8, String>,
    gesture_bindings: [u8; 5],
    forbidden_areas: Vec<u8>,
    command_log: Vec<(UvcRequest, u8, u8, Vec<u8>)>,
}

impl MockCameraTransport {
    pub fn new(model: CameraModel) -> Self {
        let pid = model.pid();
        let unit_ids = UvcUnitIds::default();
        let info = DiscoveredCamera {
            vendor_id: INSTA360_VENDOR_ID,
            product_id: pid,
            model,
            name: format!("{} (Simulated)", model.display_name()),
            unique_id: format!("mock-0x{:04x}-0x{:04x}", INSTA360_VENDOR_ID, pid),
            bus_number: 1,
            device_address: 2,
            unit_ids,
            capabilities: DeviceCapabilities::for_pid(pid),
        };
        let mut video_aux = [0u8; 36];
        video_aux[20..22].copy_from_slice(&100u16.to_le_bytes());
        let state = MockState {
            registers: HashMap::new(),
            pan_arcsec: 0,
            tilt_arcsec: 0,
            zoom: 100,
            roll: 0,
            focus: 50,
            auto_focus: true,
            brightness: 50,
            contrast: 50,
            saturation: 50,
            sharpness: 50,
            wb_kelvin: 5000,
            wb_auto: true,
            power_line: 2, // 60Hz
            shutter: 200,
            extend_bits: (1 << (ExtendFunction::AutoTracking as u8))
                | (1 << (ExtendFunction::AllGestures as u8)),
            video_mode: VideoMode::Normal as u8,
            video_aux,
            track_speed: TrackSpeed::Normal as u8,
            composition_style: CompositionStyle::HalfBody as u8,
            gestures: [1, 1, 1, 1, 1, 0],
            audio_denoise: true,
            audio_mode: AudioCaptureMode::VoiceFocus as u8,
            ev_comp: 0.0,
            iso: 100,
            auto_exposure: true,
            filter_mode: 0,
            privacy_mode: false,
            horiz_correction: pid == pids::LINK_2,
            default_track_mode: 0,
            preset_mask: 0b0000_0011,
            current_preset: 0,
            template_mask: 0b0000_0010,
            current_template: 1,
            template_names: HashMap::from([(1, "Studio Warm".to_string())]),
            gesture_bindings: [
                GestureFunction::None as u8,
                GestureFunction::AutoTracking as u8,
                GestureFunction::Zoom as u8,
                GestureFunction::WhiteBoard as u8,
                GestureFunction::AutoFraming as u8,
            ],
            forbidden_areas: vec![0u8; 40],
            command_log: Vec::new(),
        };
        Self {
            info,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn command_log(&self) -> Vec<(UvcRequest, u8, u8, Vec<u8>)> {
        self.state.lock().unwrap().command_log.clone()
    }
}

impl UvcControlTransport for MockCameraTransport {
    fn camera_info(&self) -> &DiscoveredCamera {
        &self.info
    }

    fn set_cur(&self, unit_id: u8, selector: u8, data: &[u8]) -> Result<(), TransportError> {
        let mut st = self.state.lock().unwrap();
        st.command_log
            .push((UvcRequest::SetCur, unit_id, selector, data.to_vec()));
        st.registers.insert((unit_id, selector), data.to_vec());

        let ids = self.info.unit_ids;
        if unit_id == ids.input_terminal_id {
            match selector {
                ct_selectors::ZOOM_ABSOLUTE if data.len() >= 2 => {
                    st.zoom = u16::from_le_bytes([data[0], data[1]]);
                }
                ct_selectors::PANTILT_ABSOLUTE if data.len() >= 8 => {
                    st.pan_arcsec = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                    st.tilt_arcsec = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                }
                ct_selectors::PANTILT_RELATIVE if data.len() >= 4 => {
                    let pan_dir = data[0] as i8;
                    let pan_spd = data[1] as i32;
                    let tilt_dir = data[2] as i8;
                    let tilt_spd = data[3] as i32;
                    st.pan_arcsec = (st.pan_arcsec + (pan_dir as i32) * pan_spd * 3600).clamp(-468000, 468000);
                    st.tilt_arcsec = (st.tilt_arcsec + (tilt_dir as i32) * tilt_spd * 3600).clamp(-324000, 324000);
                }
                ct_selectors::ROLL_ABSOLUTE if data.len() >= 2 => {
                    st.roll = i16::from_le_bytes([data[0], data[1]]);
                }
                ct_selectors::FOCUS_ABSOLUTE if data.len() >= 2 => {
                    st.focus = u16::from_le_bytes([data[0], data[1]]);
                }
                ct_selectors::FOCUS_AUTO if !data.is_empty() => {
                    st.auto_focus = data[0] != 0;
                }
                ct_selectors::EXPOSURE_TIME_ABSOLUTE if data.len() >= 4 => {
                    st.shutter = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                }
                _ => {}
            }
        } else if unit_id == ids.processing_unit_id {
            match selector {
                pu_selectors::BRIGHTNESS if data.len() >= 2 => {
                    st.brightness = i16::from_le_bytes([data[0], data[1]]);
                }
                pu_selectors::CONTRAST if data.len() >= 2 => {
                    st.contrast = u16::from_le_bytes([data[0], data[1]]);
                }
                pu_selectors::SATURATION if data.len() >= 2 => {
                    st.saturation = u16::from_le_bytes([data[0], data[1]]);
                }
                pu_selectors::SHARPNESS if data.len() >= 2 => {
                    st.sharpness = u16::from_le_bytes([data[0], data[1]]);
                }
                pu_selectors::WHITE_BALANCE_TEMPERATURE if data.len() >= 2 => {
                    st.wb_kelvin = u16::from_le_bytes([data[0], data[1]]);
                }
                pu_selectors::WHITE_BALANCE_TEMPERATURE_AUTO if !data.is_empty() => {
                    st.wb_auto = data[0] != 0;
                }
                pu_selectors::POWER_LINE_FREQUENCY if !data.is_empty() => {
                    st.power_line = data[0];
                }
                _ => {}
            }
        } else if unit_id == ids.extension_unit_id {
            match selector {
                xu1_selectors::SYSTEM_AND_PRESET_CMD if data.len() >= 2 => match data[0] {
                    0 => st.current_preset = data[1],
                    1 => {
                        st.preset_mask |= 1 << (data[1] & 7);
                        st.current_preset = data[1];
                    }
                    2 => st.preset_mask &= !(1 << (data[1] & 7)),
                    3 => {
                        st.pan_arcsec = 0;
                        st.tilt_arcsec = 0;
                        st.zoom = 100;
                        st.roll = 0;
                    }
                    4 => {
                        st.brightness = 50;
                        st.contrast = 50;
                        st.saturation = 50;
                        st.sharpness = 50;
                        st.wb_auto = true;
                        st.auto_exposure = true;
                        st.video_mode = 0;
                    }
                    _ => {}
                },
                xu1_selectors::VIDEO_MODE if !data.is_empty() => {
                    st.video_mode = data[0];
                    if data.len() == 8 {
                        st.video_aux[0] = data[0];
                        st.video_aux[1] = data[1];
                        st.video_aux[18..20].copy_from_slice(&data[2..4]);
                        st.video_aux[20..22].copy_from_slice(&data[4..6]);
                    } else {
                        let copy_len = data.len().min(36);
                        st.video_aux[..copy_len].copy_from_slice(&data[..copy_len]);
                    }
                }
                xu1_selectors::TRACKING_AND_GESTURE if data.len() >= 2 => match data[0] {
                    0 => st.gestures[0] = data[1],
                    1 if data.len() >= 6 => st.gestures.copy_from_slice(&data[..6]),
                    5 => st.track_speed = data[1],
                    6 => st.composition_style = data[1],
                    _ => {}
                },
                xu1_selectors::AUDIO_CONTROL if !data.is_empty() => {
                    st.audio_mode = data[0];
                    if data[0] <= 1 {
                        st.audio_denoise = data[0] != 0;
                    }
                }
                xu1_selectors::EXPOSURE_CURVE_AND_ISO if data.len() >= 3 => {
                    st.auto_exposure = data[0] != 0;
                    st.iso = u16::from_le_bytes([data[1], data[2]]);
                }
                xu1_selectors::EXPOSURE_COMPENSATION if data.len() >= 4 => {
                    st.ev_comp = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                }
                xu1_selectors::EXTEND_FUNC_BITMASK if data.len() >= 8 => {
                    st.extend_bits = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                }
                xu1_selectors::PANTILT_RELATIVE_EXT if data.len() >= 4 => {
                    let pan_spd = data[0] as i32;
                    let pan_dir = data[1] as i8 as i32;
                    let tilt_spd = data[2] as i32;
                    let tilt_dir = data[3] as i8 as i32;
                    st.pan_arcsec = (st.pan_arcsec + pan_dir * pan_spd * 3600).clamp(-468000, 468000);
                    st.tilt_arcsec = (st.tilt_arcsec + tilt_dir * tilt_spd * 3600).clamp(-324000, 324000);
                }
                _ => {}
            }
        } else if unit_id == ids.extension_unit2_id {
            match selector {
                xu2_selectors::GESTURE_BINDING_MAP if data.len() >= 2 => {
                    let g = data[0] as usize;
                    if g < 5 {
                        st.gesture_bindings[g] = data[1];
                    }
                }
                xu2_selectors::TRACK_FORBIDDEN_AREA if !data.is_empty() => {
                    st.forbidden_areas = data.to_vec();
                }
                xu2_selectors::IMAGE_TEMPLATE_INFO if data.len() >= 11 => {
                    st.current_template = data[0] as i8;
                    st.template_mask = u64::from_le_bytes(data[2..10].try_into().unwrap());
                }
                xu2_selectors::IMAGE_TEMPLATE_NAME if data.len() >= 2 => {
                    let id = data[0] as i8;
                    let end = data[1..].iter().position(|&b| b == 0).unwrap_or(data.len() - 1);
                    let name = String::from_utf8_lossy(&data[1..1 + end]).to_string();
                    st.template_names.insert(id, name);
                }
                xu2_selectors::FILTER_MODE if !data.is_empty() => {
                    st.filter_mode = data[0];
                }
                xu2_selectors::PRIVACY_AND_FILL_LIGHT if data.len() >= 2 => {
                    if data[0] == 2 {
                        st.privacy_mode = data[1] != 0;
                    }
                }
                xu2_selectors::DEFAULT_TRACK_MODE if !data.is_empty() => {
                    st.default_track_mode = data[0];
                }
                xu2_selectors::HORIZONTAL_CORRECTION if !data.is_empty() => {
                    st.horiz_correction = data[0] != 0;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn get_req(
        &self,
        req: UvcRequest,
        unit_id: u8,
        selector: u8,
        len: usize,
    ) -> Result<Vec<u8>, TransportError> {
        let st = self.state.lock().unwrap();
        let ids = self.info.unit_ids;

        if req == UvcRequest::GetLen {
            let l: u16 = if unit_id == ids.extension_unit_id {
                match selector {
                    xu1_selectors::SYSTEM_AND_PRESET_CMD => 4,
                    xu1_selectors::VIDEO_MODE => 36,
                    xu1_selectors::TRACKING_AND_GESTURE => 16,
                    xu1_selectors::AUDIO_CONTROL => 2,
                    xu1_selectors::TRACK_OBJECT_SELECT => 10,
                    xu1_selectors::TRACK_OBJECT_LIST_V1 => 20,
                    xu1_selectors::EXPOSURE_CURVE_AND_ISO => 259,
                    xu1_selectors::EXPOSURE_COMPENSATION => 4,
                    xu1_selectors::WHITE_BALANCE_EXT => 129,
                    xu1_selectors::EXTEND_FUNC_BITMASK => 8,
                    xu1_selectors::PANTILT_RELATIVE_EXT => 4,
                    xu1_selectors::DEVICE_STATUS => 4,
                    xu1_selectors::PTZ_VERSION_AND_ABS => 30,
                    xu1_selectors::PRESET_POSITION_FLAGS => 8,
                    _ => len as u16,
                }
            } else if unit_id == ids.extension_unit2_id {
                match selector {
                    xu2_selectors::GESTURE_BINDING_MAP => 16,
                    xu2_selectors::SYSTEM_TIME_AND_IQ => 10,
                    xu2_selectors::DEVICE_INFO => 150,
                    xu2_selectors::IMAGE_TEMPLATE_INFO => 11,
                    xu2_selectors::IMAGE_TEMPLATE_NAME => 65,
                    xu2_selectors::FILTER_MODE => 1,
                    xu2_selectors::PRIVACY_AND_FILL_LIGHT => 4,
                    xu2_selectors::HOST_PTZ => 8,
                    xu2_selectors::TRACK_OBJECT_LIST_V2 => 20,
                    xu2_selectors::TRACK_FORBIDDEN_AREA => 40,
                    xu2_selectors::DEFAULT_TRACK_MODE => 1,
                    xu2_selectors::HORIZONTAL_CORRECTION => 1,
                    xu2_selectors::REAL_ZOOM_AND_UDISK => 4,
                    _ => len as u16,
                }
            } else {
                len as u16
            };
            return Ok(l.to_le_bytes().to_vec());
        }

        if unit_id == ids.input_terminal_id {
            match selector {
                ct_selectors::ZOOM_ABSOLUTE => {
                    let v: u16 = match req {
                        UvcRequest::GetMin => 100,
                        UvcRequest::GetMax => 400,
                        UvcRequest::GetRes => 1,
                        UvcRequest::GetDef => 100,
                        _ => st.zoom,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                ct_selectors::PANTILT_ABSOLUTE => {
                    let (p, t): (i32, i32) = match req {
                        UvcRequest::GetMin => (-468000, -324000),
                        UvcRequest::GetMax => (468000, 324000),
                        UvcRequest::GetRes => (3600, 3600),
                        UvcRequest::GetDef => (0, 0),
                        _ => (st.pan_arcsec, st.tilt_arcsec),
                    };
                    let mut out = [0u8; 8];
                    out[0..4].copy_from_slice(&p.to_le_bytes());
                    out[4..8].copy_from_slice(&t.to_le_bytes());
                    return Ok(out.to_vec());
                }
                ct_selectors::ROLL_ABSOLUTE => {
                    let v: i16 = match req {
                        UvcRequest::GetMin => -1800,
                        UvcRequest::GetMax => 1800,
                        UvcRequest::GetRes => 1,
                        UvcRequest::GetDef => 0,
                        _ => st.roll,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                ct_selectors::FOCUS_ABSOLUTE => {
                    let v: u16 = match req {
                        UvcRequest::GetMin => 0,
                        UvcRequest::GetMax => 100,
                        UvcRequest::GetRes => 1,
                        UvcRequest::GetDef => 50,
                        _ => st.focus,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                ct_selectors::FOCUS_AUTO => {
                    return Ok(vec![u8::from(st.auto_focus)]);
                }
                ct_selectors::EXPOSURE_TIME_ABSOLUTE => {
                    let v: u32 = match req {
                        UvcRequest::GetMin => 10,
                        UvcRequest::GetMax => 1000,
                        UvcRequest::GetRes => 1,
                        UvcRequest::GetDef => 200,
                        _ => st.shutter,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                _ => {}
            }
        } else if unit_id == ids.processing_unit_id {
            match selector {
                pu_selectors::BRIGHTNESS => {
                    let v: i16 = match req {
                        UvcRequest::GetMin => 0,
                        UvcRequest::GetMax => 100,
                        UvcRequest::GetRes => 1,
                        UvcRequest::GetDef => 50,
                        _ => st.brightness,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                pu_selectors::CONTRAST => {
                    let v: u16 = match req {
                        UvcRequest::GetMin => 0,
                        UvcRequest::GetMax => 100,
                        UvcRequest::GetRes => 1,
                        UvcRequest::GetDef => 50,
                        _ => st.contrast,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                pu_selectors::SATURATION => {
                    let v: u16 = match req {
                        UvcRequest::GetMin => 0,
                        UvcRequest::GetMax => 100,
                        UvcRequest::GetRes => 1,
                        UvcRequest::GetDef => 50,
                        _ => st.saturation,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                pu_selectors::SHARPNESS => {
                    let v: u16 = match req {
                        UvcRequest::GetMin => 0,
                        UvcRequest::GetMax => 100,
                        UvcRequest::GetRes => 1,
                        UvcRequest::GetDef => 50,
                        _ => st.sharpness,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                pu_selectors::WHITE_BALANCE_TEMPERATURE => {
                    let v: u16 = match req {
                        UvcRequest::GetMin => 2000,
                        UvcRequest::GetMax => 10000,
                        UvcRequest::GetRes => 50,
                        UvcRequest::GetDef => 5000,
                        _ => st.wb_kelvin,
                    };
                    return Ok(v.to_le_bytes().to_vec());
                }
                pu_selectors::WHITE_BALANCE_TEMPERATURE_AUTO => {
                    return Ok(vec![u8::from(st.wb_auto)]);
                }
                pu_selectors::POWER_LINE_FREQUENCY => {
                    return Ok(vec![st.power_line]);
                }
                _ => {}
            }
        } else if unit_id == ids.extension_unit_id {
            match selector {
                xu1_selectors::VIDEO_MODE => {
                    let mut out = st.video_aux;
                    out[0] = st.video_mode;
                    return Ok(out.to_vec());
                }
                xu1_selectors::TRACKING_AND_GESTURE => {
                    let mut out = [0u8; 16];
                    out[..6].copy_from_slice(&st.gestures);
                    out[6] = st.track_speed;
                    out[7] = st.composition_style;
                    return Ok(out.to_vec());
                }
                xu1_selectors::AUDIO_CONTROL => {
                    return Ok(vec![st.audio_mode, u8::from(st.audio_denoise)]);
                }
                xu1_selectors::EXPOSURE_CURVE_AND_ISO => {
                    let mut out = vec![0u8; 259];
                    out[0] = u8::from(st.auto_exposure);
                    out[1..3].copy_from_slice(&st.iso.to_le_bytes());
                    return Ok(out);
                }
                xu1_selectors::EXPOSURE_COMPENSATION => {
                    return Ok(st.ev_comp.to_le_bytes().to_vec());
                }
                xu1_selectors::EXTEND_FUNC_BITMASK => {
                    let mut out = [0u8; 8];
                    out[0..4].copy_from_slice(&0xffff_ffffu32.to_le_bytes());
                    out[4..8].copy_from_slice(&st.extend_bits.to_le_bytes());
                    return Ok(out.to_vec());
                }
                xu1_selectors::PANTILT_RELATIVE_EXT => {
                    return Ok(vec![1, 10, 1, 10]);
                }
                xu1_selectors::DEVICE_STATUS => {
                    let mut flags = 0u32;
                    if st.video_mode == VideoMode::AutoTracking as u8 {
                        flags |= DeviceStatus::TRACKING_ACTIVE.bits();
                    }
                    if st.privacy_mode {
                        flags |= DeviceStatus::PRIVACY_SLEEP.bits();
                    }
                    return Ok(flags.to_le_bytes().to_vec());
                }
                xu1_selectors::PTZ_VERSION_AND_ABS => {
                    let mut out = [0u8; 30];
                    let ver = b"v1.3.1.0_build0";
                    out[..ver.len()].copy_from_slice(ver);
                    let pan_deg10 = (st.pan_arcsec / 360) as i16;
                    let tilt_deg10 = (st.tilt_arcsec / 360) as i16;
                    out[20..22].copy_from_slice(&pan_deg10.to_le_bytes());
                    out[22..24].copy_from_slice(&tilt_deg10.to_le_bytes());
                    out[24..26].copy_from_slice(&st.zoom.to_le_bytes());
                    return Ok(out.to_vec());
                }
                xu1_selectors::PRESET_POSITION_FLAGS => {
                    let mut out = [0u8; 8];
                    out[0] = st.preset_mask;
                    out[1] = st.current_preset;
                    out[2] = 6;
                    return Ok(out.to_vec());
                }
                _ => {}
            }
        } else if unit_id == ids.extension_unit2_id {
            match selector {
                xu2_selectors::GESTURE_BINDING_MAP => {
                    let mut out = [0u8; 16];
                    out[..5].copy_from_slice(&st.gesture_bindings);
                    return Ok(out.to_vec());
                }
                xu2_selectors::TRACK_FORBIDDEN_AREA => {
                    return Ok(st.forbidden_areas.clone());
                }
                xu2_selectors::DEVICE_INFO => {
                    let mut out = [0u8; 150];
                    let name = self.info.model.display_name().as_bytes();
                    out[..name.len().min(49)].copy_from_slice(&name[..name.len().min(49)]);
                    let fw = b"v2.5.8.8_build1";
                    out[50..50 + fw.len()].copy_from_slice(fw);
                    let sn = b"INSTA360SIM0001";
                    out[100..100 + sn.len()].copy_from_slice(sn);
                    return Ok(out.to_vec());
                }
                xu2_selectors::IMAGE_TEMPLATE_INFO => {
                    let mut out = [0u8; 11];
                    out[0] = st.current_template as u8;
                    out[1] = 0;
                    out[2..10].copy_from_slice(&st.template_mask.to_le_bytes());
                    out[10] = 10;
                    return Ok(out.to_vec());
                }
                xu2_selectors::IMAGE_TEMPLATE_NAME => {
                    let mut out = [0u8; 65];
                    out[0] = st.current_template as u8;
                    if let Some(name) = st.template_names.get(&st.current_template) {
                        let b = name.as_bytes();
                        out[1..1 + b.len().min(63)].copy_from_slice(&b[..b.len().min(63)]);
                    }
                    return Ok(out.to_vec());
                }
                xu2_selectors::FILTER_MODE => {
                    return Ok(vec![st.filter_mode]);
                }
                xu2_selectors::PRIVACY_AND_FILL_LIGHT => {
                    return Ok(vec![0, u8::from(st.privacy_mode), 0, 0]);
                }
                xu2_selectors::DEFAULT_TRACK_MODE => {
                    return Ok(vec![st.default_track_mode]);
                }
                xu2_selectors::HORIZONTAL_CORRECTION => {
                    return Ok(vec![u8::from(st.horiz_correction)]);
                }
                xu2_selectors::REAL_ZOOM_AND_UDISK => {
                    let mut out = [0u8; 4];
                    out[0..2].copy_from_slice(&st.zoom.to_le_bytes());
                    return Ok(out.to_vec());
                }
                _ => {}
            }
        }

        if let Some(saved) = st.registers.get(&(unit_id, selector)) {
            let mut v = saved.clone();
            v.resize(len, 0);
            return Ok(v);
        }

        Ok(vec![0u8; len])
    }
}
