use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, Response},
    routing::{get, post},
    Json, Router,
};
use open_360_link_protocol::{
    list_connected_cameras, AudioCaptureMode, CameraModel, CompositionStyle, DeviceCapabilities,
    DeviceInfo, DiscoveredCamera, ExtendFunction, FilterMode, ForbiddenAreaInfo, Gesture,
    GestureFunction, HostPtzInfo, Insta360CameraController, MockCameraTransport, NusbTransport,
    PowerLineFrequency, PresetFlag, PtzVersionInfo, TrackSpeed, TransportError, UvcControlTransport,
    UvcRequest, VideoMode, VideoModeAuxData,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;

/// Unified server transport allowing runtime switching between physical USB cameras
/// (`NusbTransport`) and simulated in-memory camera devices (`MockCameraTransport`).
pub enum ServerTransport {
    Usb(NusbTransport),
    Mock(MockCameraTransport),
}

impl ServerTransport {
    pub fn is_mock(&self) -> bool {
        matches!(self, Self::Mock(_))
    }
}

impl UvcControlTransport for ServerTransport {
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

    fn get_len(&self, unit_id: u8, selector: u8) -> Result<u16, TransportError> {
        match self {
            Self::Usb(usb) => usb.get_len(unit_id, selector),
            Self::Mock(mock) => mock.get_len(unit_id, selector),
        }
    }
}

/// Shared application state wrapping the thread-safe camera controller.
pub type AppState = Arc<Mutex<Insta360CameraController<ServerTransport>>>;

/// Creates a new `AppState` instance initialized with an in-memory simulated camera model.
pub fn new_mock_state(model: CameraModel) -> AppState {
    let mock = MockCameraTransport::new(model);
    let ctrl = Insta360CameraController::new(ServerTransport::Mock(mock));
    Arc::new(Mutex::new(ctrl))
}

/// Creates a new `AppState` instance initialized with a physical USB camera transport.
pub fn new_usb_state(transport: NusbTransport) -> AppState {
    let ctrl = Insta360CameraController::new(ServerTransport::Usb(transport));
    Arc::new(Mutex::new(ctrl))
}

/// Full camera hardware and software state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraStateSnapshot {
    pub model: CameraModel,
    pub capabilities: DeviceCapabilities,
    pub device_info: DeviceInfo,
    pub ptz_info: PtzVersionInfo,
    pub device_status: u32,
    pub pan_arcsec: i32,
    pub tilt_arcsec: i32,
    pub zoom: u16,
    pub real_zoom: u16,
    pub roll: i16,
    pub focus: u16,
    pub auto_focus: bool,
    pub brightness: i16,
    pub contrast: u16,
    pub saturation: u16,
    pub sharpness: u16,
    pub wb_kelvin: u16,
    pub wb_auto: bool,
    pub power_line_frequency: PowerLineFrequency,
    pub shutter_speed: u32,
    pub auto_exposure: bool,
    pub iso: u16,
    pub ev_compensation: f32,
    pub video_mode: VideoMode,
    pub video_aux: VideoModeAuxData,
    pub composition_style: CompositionStyle,
    pub track_speed: TrackSpeed,
    pub gestures: Vec<(Gesture, bool)>,
    pub gesture_bindings: Vec<(Gesture, GestureFunction)>,
    pub forbidden_areas: Vec<ForbiddenAreaInfo>,
    pub extend_functions: Vec<(ExtendFunction, bool)>,
    pub audio_mode: AudioCaptureMode,
    pub audio_denoise: bool,
    pub filter_mode: FilterMode,
    pub privacy_mode: bool,
    pub horizontal_correction: bool,
    pub preset_flags: PresetFlag,
}

/// Helper function to capture a complete snapshot from any camera controller.
pub fn capture_snapshot<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
) -> CameraStateSnapshot {
    let model = ctrl.model();
    let capabilities = ctrl.capabilities();
    let device_info = ctrl.get_device_info().unwrap_or_default();
    let ptz_info = ctrl.get_ptz_version_and_angles().unwrap_or_default();
    let device_status = ctrl.get_device_status().map(|s| s.bits()).unwrap_or(0);
    let (pan_arcsec, tilt_arcsec) = ctrl.get_pantilt_absolute().unwrap_or((0, 0));
    let zoom = ctrl.get_zoom().unwrap_or(100);
    let real_zoom = ctrl.get_real_zoom_value().unwrap_or(zoom);
    let roll = ctrl.get_roll().unwrap_or(0);
    let focus = ctrl.get_focus().unwrap_or(0);
    let auto_focus = ctrl.get_auto_focus().unwrap_or(true);
    let brightness = ctrl.get_brightness().unwrap_or(50);
    let contrast = ctrl.get_contrast().unwrap_or(50);
    let saturation = ctrl.get_saturation().unwrap_or(50);
    let sharpness = ctrl.get_sharpness().unwrap_or(50);
    let wb_kelvin = ctrl.get_white_balance_temperature().unwrap_or(5000);
    let wb_auto = ctrl.get_auto_white_balance().unwrap_or(true);
    let power_line_frequency = ctrl.get_power_line_frequency().unwrap_or_default();
    let shutter_speed = ctrl.get_shutter_speed().unwrap_or(0);
    let auto_exposure = ctrl.get_auto_exposure().unwrap_or(true);
    let iso = ctrl.get_iso().unwrap_or(100);
    let ev_compensation = ctrl.get_exposure_compensation().unwrap_or(0.0);
    let (video_mode, video_aux) = ctrl
        .get_video_mode()
        .unwrap_or((VideoMode::Normal, VideoModeAuxData::default()));
    let composition_style = ctrl.get_composition_style().unwrap_or_default();
    let track_speed = ctrl.get_track_speed().unwrap_or_default();
    let gestures = ctrl
        .get_gesture_status()
        .map(|arr| arr.to_vec())
        .unwrap_or_else(|_| Gesture::ALL.iter().map(|&g| (g, false)).collect());
    let gesture_bindings = ctrl
        .get_gesture_binding_map()
        .map(|arr| arr.to_vec())
        .unwrap_or_else(|_| {
            Gesture::ALL
                .iter()
                .map(|&g| (g, GestureFunction::None))
                .collect()
        });
    let forbidden_areas = ctrl.get_track_forbidden_areas().unwrap_or_default();
    let extend_functions = ctrl
        .get_extend_function_status()
        .unwrap_or_else(|_| ExtendFunction::ALL.iter().map(|&f| (f, false)).collect());
    let audio_mode = ctrl.get_audio_capture_mode().unwrap_or_default();
    let audio_denoise = ctrl.get_audio_noise_reduction().unwrap_or(false);
    let filter_mode = ctrl.get_filter().unwrap_or_default();
    let privacy_mode = ctrl.get_privacy_mode().map(|(_, en)| en).unwrap_or(false);
    let horizontal_correction = ctrl.get_horizontal_correction().unwrap_or(false);
    let preset_flags = ctrl.get_preset_flags().unwrap_or_default();

    CameraStateSnapshot {
        model,
        capabilities,
        device_info,
        ptz_info,
        device_status,
        pan_arcsec,
        tilt_arcsec,
        zoom,
        real_zoom,
        roll,
        focus,
        auto_focus,
        brightness,
        contrast,
        saturation,
        sharpness,
        wb_kelvin,
        wb_auto,
        power_line_frequency,
        shutter_speed,
        auto_exposure,
        iso,
        ev_compensation,
        video_mode,
        video_aux,
        composition_style,
        track_speed,
        gestures,
        gesture_bindings,
        forbidden_areas,
        extend_functions,
        audio_mode,
        audio_denoise,
        filter_mode,
        privacy_mode,
        horizontal_correction,
        preset_flags,
    }
}

// ============================================================================
// Request Payloads
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PtzRequest {
    pub pan_arcsec: Option<i32>,
    pub tilt_arcsec: Option<i32>,
    pub pan_speed: Option<u8>,
    pub pan_dir: Option<i8>,
    pub tilt_speed: Option<u8>,
    pub tilt_dir: Option<i8>,
    pub stop: bool,
    pub zoom: Option<u16>,
    pub roll: Option<i16>,
    pub focus: Option<u16>,
    pub auto_focus: Option<bool>,
    pub bias_x: Option<i16>,
    pub bias_y: Option<i16>,
    pub reset_bias: bool,
    pub host_ptz: Option<HostPtzInfo>,
    pub reset_gimbal: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModeRequest {
    pub mode: VideoMode,
    pub pitch: Option<i16>,
    pub zoom: Option<u16>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PresetRequest {
    pub action: String,
    pub index: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageRequest {
    pub brightness: Option<i16>,
    pub contrast: Option<u16>,
    pub saturation: Option<u16>,
    pub sharpness: Option<u16>,
    pub wb_auto: Option<bool>,
    pub wb_kelvin: Option<u16>,
    pub auto_exposure: Option<bool>,
    pub iso: Option<u16>,
    pub shutter_speed: Option<u32>,
    pub ev_compensation: Option<f32>,
    pub hdr: Option<bool>,
    pub filter_mode: Option<FilterMode>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackingRequest {
    pub track_speed: Option<TrackSpeed>,
    pub composition_style: Option<CompositionStyle>,
    pub all_gestures: Option<bool>,
    pub gesture: Option<(Gesture, bool)>,
    pub gesture_binding: Option<(Gesture, GestureFunction)>,
    pub forbidden_areas: Option<Vec<ForbiddenAreaInfo>>,
    pub forbidden_area_enabled: Option<bool>,
    pub force_smart_whiteboard: Option<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioRequest {
    pub audio_mode: Option<AudioCaptureMode>,
    pub audio_denoise: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemRequest {
    pub privacy_mode: Option<bool>,
    pub horizontal_correction: Option<bool>,
    pub power_line_frequency: Option<PowerLineFrequency>,
    pub extend_function: Option<(ExtendFunction, bool)>,
    pub iq_switch: Option<(u8, bool, u32)>,
    pub factory_reset: Option<bool>,
}

/// Messages received from WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsClientMessage {
    GetState,
    Ptz(PtzRequest),
    Mode(ModeRequest),
    Preset(PresetRequest),
    Image(ImageRequest),
    Tracking(TrackingRequest),
    Audio(AudioRequest),
    System(SystemRequest),
}

/// Response payload for `GET /api/cameras`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamerasResponse {
    pub active: DiscoveredCamera,
    pub connected: Vec<DiscoveredCamera>,
}

// ============================================================================
// Command Application Helpers
// ============================================================================

pub fn apply_ptz<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
    req: &PtzRequest,
) {
    if req.reset_gimbal {
        let _ = ctrl.reset_gimbal();
    }
    if req.stop {
        let _ = ctrl.stop_pantilt_relative();
    }
    if req.pan_speed.is_some()
        || req.pan_dir.is_some()
        || req.tilt_speed.is_some()
        || req.tilt_dir.is_some()
    {
        let _ = ctrl.set_pantilt_relative_ext(
            req.pan_speed.unwrap_or(0),
            req.pan_dir.unwrap_or(0),
            req.tilt_speed.unwrap_or(0),
            req.tilt_dir.unwrap_or(0),
        );
    }
    if req.pan_arcsec.is_some() || req.tilt_arcsec.is_some() {
        let (cur_p, cur_t) = ctrl.get_pantilt_absolute().unwrap_or((0, 0));
        let p = req.pan_arcsec.unwrap_or(cur_p);
        let t = req.tilt_arcsec.unwrap_or(cur_t);
        let _ = ctrl.set_pantilt_absolute(p, t);
    }
    if let Some(zoom) = req.zoom {
        let _ = ctrl.set_zoom(zoom);
    }
    if let Some(roll) = req.roll {
        let _ = ctrl.set_roll(roll);
    }
    if let Some(focus) = req.focus {
        let _ = ctrl.set_focus(focus);
    }
    if let Some(af) = req.auto_focus {
        let _ = ctrl.set_auto_focus(af);
    }
    if req.reset_bias {
        let _ = ctrl.reset_composition_bias();
    } else if let (Some(x), Some(y)) = (req.bias_x, req.bias_y) {
        let _ = ctrl.set_composition_bias(x, y);
    }
    if let Some(hp) = req.host_ptz {
        let _ = ctrl.set_host_ptz(&hp);
    }
}

pub fn apply_mode<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
    req: &ModeRequest,
) {
    let (_, mut aux) = ctrl.get_video_mode().unwrap_or_default();
    if let Some(p) = req.pitch {
        aux.pitch = p;
    }
    if let Some(z) = req.zoom {
        aux.zoom = z;
    }
    let _ = ctrl.set_video_mode(req.mode, &aux);
}

pub fn apply_preset<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
    req: &PresetRequest,
) {
    match req.action.to_ascii_lowercase().as_str() {
        "save" | "add" => {
            let _ = ctrl.add_preset_position(req.index);
        }
        "recall" | "set" => {
            let _ = ctrl.recall_preset_position(req.index);
        }
        "delete" | "del" => {
            let _ = ctrl.delete_preset_position(req.index);
        }
        _ => {}
    }
}

pub fn apply_image<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
    req: &ImageRequest,
) {
    if let Some(b) = req.brightness {
        let _ = ctrl.set_brightness(b);
    }
    if let Some(c) = req.contrast {
        let _ = ctrl.set_contrast(c);
    }
    if let Some(s) = req.saturation {
        let _ = ctrl.set_saturation(s);
    }
    if let Some(s) = req.sharpness {
        let _ = ctrl.set_sharpness(s);
    }
    if let Some(wb_auto) = req.wb_auto {
        let _ = ctrl.set_auto_white_balance(wb_auto);
    }
    if let Some(wb_k) = req.wb_kelvin {
        let _ = ctrl.set_white_balance_temperature(wb_k);
    }
    if let Some(ae) = req.auto_exposure {
        let _ = ctrl.set_auto_exposure(ae);
    }
    if let Some(iso) = req.iso {
        let _ = ctrl.set_iso(iso);
    }
    if let Some(shutter) = req.shutter_speed {
        let _ = ctrl.set_shutter_speed(shutter);
    }
    if let Some(ev) = req.ev_compensation {
        let _ = ctrl.set_exposure_compensation(ev);
    }
    if let Some(hdr) = req.hdr {
        let _ = ctrl.set_hdr(hdr);
    }
    if let Some(fm) = req.filter_mode {
        let _ = ctrl.set_filter(fm);
    }
}

pub fn apply_tracking<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
    req: &TrackingRequest,
) {
    if let Some(speed) = req.track_speed {
        let _ = ctrl.set_track_speed(speed);
    }
    if let Some(style) = req.composition_style {
        let _ = ctrl.set_composition_style(style);
    }
    if let Some(all) = req.all_gestures {
        let _ = ctrl.set_all_gestures_enabled(all);
    }
    if let Some((g, en)) = req.gesture {
        let _ = ctrl.set_gesture_enabled(g, en);
    }
    if let Some((g, func)) = req.gesture_binding {
        let _ = ctrl.set_gesture_binding_func(g, func);
    }
    if let Some(areas) = &req.forbidden_areas {
        let _ = ctrl.set_track_forbidden_areas(areas);
    }
    if let Some(en) = req.forbidden_area_enabled {
        let _ = ctrl.set_track_forbidden_area_enabled(en);
    }
    if let Some(sw) = req.force_smart_whiteboard {
        let _ = ctrl.force_smart_whiteboard(sw);
    }
}

pub fn apply_audio<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
    req: &AudioRequest,
) {
    if let Some(mode) = req.audio_mode {
        let _ = ctrl.set_audio_capture_mode(mode);
    }
    if let Some(denoise) = req.audio_denoise {
        let _ = ctrl.set_audio_noise_reduction(denoise);
    }
}

pub fn apply_system<T: UvcControlTransport>(
    ctrl: &Insta360CameraController<T>,
    req: &SystemRequest,
) {
    if let Some(pm) = req.privacy_mode {
        let _ = ctrl.set_privacy_mode(pm);
    }
    if let Some(hc) = req.horizontal_correction {
        let _ = ctrl.set_horizontal_correction(hc);
    }
    if let Some(plf) = req.power_line_frequency {
        let _ = ctrl.set_power_line_frequency(plf);
    }
    if let Some((func, en)) = req.extend_function {
        let _ = ctrl.set_extend_function(func, en);
    }
    if let Some((bit, en, mask)) = req.iq_switch {
        let _ = ctrl.set_iq_switch(bit, en, mask);
    }
    if let Some(reset) = req.factory_reset {
        if reset {
            let _ = ctrl.load_factory_default();
            let _ = ctrl.restore_factory_settings(false);
        }
    }
}

// ============================================================================
// Axum HTTP & WebSocket Handlers
// ============================================================================

async fn index_handler() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn get_cameras_handler(State(state): State<AppState>) -> Json<CamerasResponse> {
    let active = {
        let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
        ctrl.camera_info().clone()
    };
    let connected = match list_connected_cameras() {
        Ok(cams) if !cams.is_empty() => cams,
        _ => vec![active.clone()],
    };
    Json(CamerasResponse { active, connected })
}

async fn get_state_handler(State(state): State<AppState>) -> Json<CameraStateSnapshot> {
    let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
    Json(capture_snapshot(&ctrl))
}

async fn ptz_handler(
    State(state): State<AppState>,
    Json(req): Json<PtzRequest>,
) -> Json<CameraStateSnapshot> {
    let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
    apply_ptz(&ctrl, &req);
    Json(capture_snapshot(&ctrl))
}

async fn mode_handler(
    State(state): State<AppState>,
    Json(req): Json<ModeRequest>,
) -> Json<CameraStateSnapshot> {
    let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
    apply_mode(&ctrl, &req);
    Json(capture_snapshot(&ctrl))
}

async fn preset_handler(
    State(state): State<AppState>,
    Json(req): Json<PresetRequest>,
) -> Json<CameraStateSnapshot> {
    let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
    apply_preset(&ctrl, &req);
    Json(capture_snapshot(&ctrl))
}

async fn image_handler(
    State(state): State<AppState>,
    Json(req): Json<ImageRequest>,
) -> Json<CameraStateSnapshot> {
    let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
    apply_image(&ctrl, &req);
    Json(capture_snapshot(&ctrl))
}

async fn tracking_handler(
    State(state): State<AppState>,
    Json(req): Json<TrackingRequest>,
) -> Json<CameraStateSnapshot> {
    let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
    apply_tracking(&ctrl, &req);
    Json(capture_snapshot(&ctrl))
}

async fn audio_handler(
    State(state): State<AppState>,
    Json(req): Json<AudioRequest>,
) -> Json<CameraStateSnapshot> {
    let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
    apply_audio(&ctrl, &req);
    Json(capture_snapshot(&ctrl))
}

async fn system_handler(
    State(state): State<AppState>,
    Json(req): Json<SystemRequest>,
) -> Json<CameraStateSnapshot> {
    let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
    apply_system(&ctrl, &req);
    Json(capture_snapshot(&ctrl))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws_socket(socket, state))
}

async fn handle_ws_socket(mut socket: WebSocket, state: AppState) {
    // 1. Send initial state snapshot upon connection
    let initial_snapshot = {
        let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
        capture_snapshot(&ctrl)
    };
    if let Ok(json_str) = serde_json::to_string(&initial_snapshot) {
        if socket.send(Message::Text(json_str.into())).await.is_err() {
            return;
        }
    }

    // 2. Process incoming client commands and reply with updated state snapshot
    while let Some(msg_res) = socket.recv().await {
        let msg = match msg_res {
            Ok(m) => m,
            Err(_) => break,
        };

        let text = match msg {
            Message::Text(t) => t,
            Message::Binary(b) => match String::from_utf8(b.to_vec()) {
                Ok(s) => s.into(),
                Err(_) => continue,
            },
            Message::Close(_) => break,
            Message::Ping(p) => {
                let _ = socket.send(Message::Pong(p)).await;
                continue;
            }
            _ => continue,
        };

        let cmd: WsClientMessage = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to deserialize WsClientMessage: {}", e);
                continue;
            }
        };

        let updated_snapshot = {
            let ctrl = state.lock().unwrap_or_else(|p| p.into_inner());
            match cmd {
                WsClientMessage::GetState => {}
                WsClientMessage::Ptz(req) => apply_ptz(&ctrl, &req),
                WsClientMessage::Mode(req) => apply_mode(&ctrl, &req),
                WsClientMessage::Preset(req) => apply_preset(&ctrl, &req),
                WsClientMessage::Image(req) => apply_image(&ctrl, &req),
                WsClientMessage::Tracking(req) => apply_tracking(&ctrl, &req),
                WsClientMessage::Audio(req) => apply_audio(&ctrl, &req),
                WsClientMessage::System(req) => apply_system(&ctrl, &req),
            }
            capture_snapshot(&ctrl)
        };

        if let Ok(json_str) = serde_json::to_string(&updated_snapshot) {
            if socket.send(Message::Text(json_str.into())).await.is_err() {
                break;
            }
        }
    }
}

/// Builds the complete Axum router configured with state, CORS, REST, WebSocket, and Web UI.
pub fn build_router(state: AppState) -> axum::Router {
    let cors = CorsLayer::permissive();

    Router::new()
        .route("/", get(index_handler))
        .route("/api/cameras", get(get_cameras_handler))
        .route("/api/state", get(get_state_handler))
        .route("/api/ptz", post(ptz_handler))
        .route("/api/mode", post(mode_handler))
        .route("/api/preset", post(preset_handler))
        .route("/api/image", post(image_handler))
        .route("/api/tracking", post(tracking_handler))
        .route("/api/audio", post(audio_handler))
        .route("/api/system", post(system_handler))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .with_state(state)
}

// ============================================================================
// Embedded Dark-Mode Web Remote UI
// ============================================================================

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Insta360 Link Controller</title>
  <style>
    :root {
      --bg: #0d1117;
      --card: #161b22;
      --card-border: #30363d;
      --accent: #00c2ff;
      --accent-hover: #33d0ff;
      --text: #c9d1d9;
      --text-bright: #f0f6fc;
      --muted: #8b949e;
      --success: #238636;
      --danger: #da3633;
      --radius: 8px;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }
    body { background: var(--bg); color: var(--text); padding: 1.5rem; min-height: 100vh; }
    header { display: flex; justify-content: space-between; align-items: center; padding-bottom: 1.5rem; border-bottom: 1px solid var(--card-border); margin-bottom: 1.5rem; }
    .title-box h1 { font-size: 1.4rem; color: var(--text-bright); display: flex; align-items: center; gap: 0.5rem; }
    .badge { font-size: 0.75rem; padding: 0.2rem 0.5rem; border-radius: 999px; background: rgba(0,194,255,0.15); color: var(--accent); border: 1px solid var(--accent); }
    .status-pill { display: inline-flex; align-items: center; gap: 0.4rem; font-size: 0.8rem; padding: 0.3rem 0.7rem; border-radius: 999px; background: #21262d; border: 1px solid var(--card-border); }
    .dot { width: 8px; height: 8px; border-radius: 50%; background: #da3633; transition: background 0.3s; }
    .dot.online { background: #2ea043; box-shadow: 0 0 8px #2ea043; }
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 1.5rem; }
    .card { background: var(--card); border: 1px solid var(--card-border); border-radius: var(--radius); padding: 1.25rem; display: flex; flex-direction: column; gap: 1rem; }
    .card h2 { font-size: 1.05rem; color: var(--text-bright); border-bottom: 1px solid rgba(255,255,255,0.08); padding-bottom: 0.5rem; display: flex; justify-content: space-between; align-items: center; }
    .form-group { display: flex; flex-direction: column; gap: 0.35rem; }
    .form-group label { font-size: 0.8rem; color: var(--muted); display: flex; justify-content: space-between; }
    .form-group label span.val { color: var(--accent); font-weight: 600; }
    input[type=range] { accent-color: var(--accent); width: 100%; cursor: pointer; }
    select, input[type=number] { background: #21262d; border: 1px solid var(--card-border); color: var(--text-bright); padding: 0.45rem; border-radius: 6px; font-size: 0.85rem; width: 100%; }
    .btn-row { display: flex; flex-wrap: wrap; gap: 0.5rem; }
    button { background: #21262d; border: 1px solid var(--card-border); color: var(--text-bright); padding: 0.5rem 0.85rem; border-radius: 6px; font-size: 0.85rem; cursor: pointer; transition: all 0.15s; font-weight: 500; }
    button:hover { background: #30363d; border-color: #8b949e; }
    button.active, button.primary { background: var(--accent); color: #000; border-color: var(--accent); font-weight: 600; }
    button.primary:hover { background: var(--accent-hover); }
    button.danger { background: rgba(218,54,51,0.2); border-color: var(--danger); color: #ff7b72; }
    button.danger:hover { background: var(--danger); color: #fff; }
    .dpad-container { display: flex; flex-direction: column; align-items: center; gap: 0.4rem; margin: 0.5rem 0; }
    .dpad-row { display: flex; gap: 0.4rem; }
    .dpad-btn { width: 44px; height: 44px; display: flex; align-items: center; justify-content: center; font-size: 1.1rem; padding: 0; }
    .toggle-row { display: flex; justify-content: space-between; align-items: center; padding: 0.3rem 0; }
    .toggle-row span { font-size: 0.85rem; }
    .switch { position: relative; display: inline-block; width: 38px; height: 22px; }
    .switch input { opacity: 0; width: 0; height: 0; }
    .slider { position: absolute; cursor: pointer; top: 0; left: 0; right: 0; bottom: 0; background: #30363d; transition: .2s; border-radius: 22px; }
    .slider:before { position: absolute; content: ""; height: 16px; width: 16px; left: 3px; bottom: 3px; background: white; transition: .2s; border-radius: 50%; }
    input:checked + .slider { background: var(--accent); }
    input:checked + .slider:before { transform: translateX(16px); background: #000; }
    .presets-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.5rem; }
    .preset-box { border: 1px solid var(--card-border); background: #21262d; border-radius: 6px; padding: 0.5rem; text-align: center; }
    .preset-box span { font-size: 0.75rem; color: var(--muted); display: block; margin-bottom: 0.3rem; }
    .preset-box .preset-btns { display: flex; gap: 0.25rem; justify-content: center; }
    .preset-box button { font-size: 0.7rem; padding: 0.2rem 0.4rem; }
  </style>
</head>
<body>
  <header>
    <div class="title-box">
      <h1>Insta360 Link Remote <span class="badge" id="cam-model">Loading...</span></h1>
      <div style="font-size: 0.8rem; color: var(--muted); margin-top: 0.25rem;" id="cam-details">Connecting to hardware...</div>
    </div>
    <div class="status-pill">
      <span class="dot" id="ws-dot"></span>
      <span id="ws-status">Connecting WS</span>
    </div>
  </header>

  <div class="grid">
    <!-- PTZ Controls -->
    <div class="card">
      <h2>Gimbal & PTZ <span>🎮</span></h2>
      <div class="dpad-container">
        <button class="dpad-btn" onmousedown="startMove(0,1)" onmouseup="stopMove()" title="Tilt Up">▲</button>
        <div class="dpad-row">
          <button class="dpad-btn" onmousedown="startMove(-1,0)" onmouseup="stopMove()" title="Pan Left">◀</button>
          <button class="dpad-btn" onclick="sendPtz({reset_gimbal: true})" title="Home Gimbal">⌂</button>
          <button class="dpad-btn" onmousedown="startMove(1,0)" onmouseup="stopMove()" title="Pan Right">▶</button>
        </div>
        <button class="dpad-btn" onmousedown="startMove(0,-1)" onmouseup="stopMove()" title="Tilt Down">▼</button>
      </div>

      <div class="form-group">
        <label>Zoom <span class="val" id="val-zoom">100%</span></label>
        <input type="range" id="zoom" min="100" max="400" value="100" oninput="updateLabel('val-zoom', this.value+'%')" onchange="sendPtz({zoom: parseInt(this.value)})">
      </div>

      <div class="form-group">
        <label>Roll Angle <span class="val" id="val-roll">0°</span></label>
        <input type="range" id="roll" min="-1800" max="1800" value="0" oninput="updateLabel('val-roll', (this.value/10)+'°')" onchange="sendPtz({roll: parseInt(this.value)})">
      </div>

      <div class="toggle-row">
        <span>Auto Focus</span>
        <label class="switch">
          <input type="checkbox" id="af-toggle" onchange="sendPtz({auto_focus: this.checked})">
          <span class="slider"></span>
        </label>
      </div>

      <div class="form-group">
        <label>Manual Focus <span class="val" id="val-focus">50</span></label>
        <input type="range" id="focus" min="0" max="100" value="50" oninput="updateLabel('val-focus', this.value)" onchange="sendPtz({focus: parseInt(this.value), auto_focus: false})">
      </div>
    </div>

    <!-- Video Modes & Presets -->
    <div class="card">
      <h2>Video Mode & Presets <span>📐</span></h2>
      <div class="form-group">
        <label>Active Video Mode</label>
        <div class="btn-row" id="mode-buttons">
          <button onclick="sendMode('Normal')">Normal</button>
          <button onclick="sendMode('AutoTracking')">AI Tracking</button>
          <button onclick="sendMode('WhiteBoard')">Whiteboard</button>
          <button onclick="sendMode('OverheadShot')">Overhead</button>
          <button onclick="sendMode('DeskView')">DeskView</button>
          <button onclick="sendMode('AutoFraming')">Auto Framing</button>
          <button onclick="sendMode('SmartWhiteBoard')">Smart Whiteboard</button>
        </div>
      </div>

      <div class="form-group" style="margin-top: 0.5rem;">
        <label>Preset Positions</label>
        <div class="presets-grid">
          <div class="preset-box" id="pbox-0"><span>Slot 0</span><div class="preset-btns"><button onclick="sendPreset('recall', 0)">Recall</button><button onclick="sendPreset('save', 0)">Save</button></div></div>
          <div class="preset-box" id="pbox-1"><span>Slot 1</span><div class="preset-btns"><button onclick="sendPreset('recall', 1)">Recall</button><button onclick="sendPreset('save', 1)">Save</button></div></div>
          <div class="preset-box" id="pbox-2"><span>Slot 2</span><div class="preset-btns"><button onclick="sendPreset('recall', 2)">Recall</button><button onclick="sendPreset('save', 2)">Save</button></div></div>
          <div class="preset-box" id="pbox-3"><span>Slot 3</span><div class="preset-btns"><button onclick="sendPreset('recall', 3)">Recall</button><button onclick="sendPreset('save', 3)">Save</button></div></div>
          <div class="preset-box" id="pbox-4"><span>Slot 4</span><div class="preset-btns"><button onclick="sendPreset('recall', 4)">Recall</button><button onclick="sendPreset('save', 4)">Save</button></div></div>
          <div class="preset-box" id="pbox-5"><span>Slot 5</span><div class="preset-btns"><button onclick="sendPreset('recall', 5)">Recall</button><button onclick="sendPreset('save', 5)">Save</button></div></div>
        </div>
      </div>
    </div>

    <!-- AI Tracking & Gestures -->
    <div class="card">
      <h2>AI Tracking & Gestures <span>🖐️</span></h2>
      <div class="form-group">
        <label>Tracking Speed</label>
        <div class="btn-row">
          <button id="spd-Slow" onclick="sendTracking({track_speed: 'Slow'})">Slow</button>
          <button id="spd-Normal" onclick="sendTracking({track_speed: 'Normal'})">Normal</button>
          <button id="spd-Fast" onclick="sendTracking({track_speed: 'Fast'})">Fast</button>
        </div>
      </div>

      <div class="form-group">
        <label>Framing Composition</label>
        <div class="btn-row">
          <button id="comp-Head" onclick="sendTracking({composition_style: 'Head'})">Head</button>
          <button id="comp-HalfBody" onclick="sendTracking({composition_style: 'HalfBody'})">Half Body</button>
          <button id="comp-WholeBody" onclick="sendTracking({composition_style: 'WholeBody'})">Whole Body</button>
        </div>
      </div>

      <div class="toggle-row">
        <span>Master Gesture Recognition</span>
        <label class="switch">
          <input type="checkbox" id="all-gestures" onchange="sendTracking({all_gestures: this.checked})">
          <span class="slider"></span>
        </label>
      </div>

      <div class="form-group">
        <label>Enabled Gestures</label>
        <div class="btn-row">
          <button id="g-Ok" onclick="toggleGesture('Ok')">👌 OK</button>
          <button id="g-Palm" onclick="toggleGesture('Palm')">✋ Palm</button>
          <button id="g-LShape" onclick="toggleGesture('LShape')">👆 L-Shape</button>
          <button id="g-VShape" onclick="toggleGesture('VShape')">✌️ V-Shape</button>
          <button id="g-Rock" onclick="toggleGesture('Rock')">🤘 Rock</button>
        </div>
      </div>
    </div>

    <!-- Image & Exposure -->
    <div class="card">
      <h2>Image & Color <span>🎨</span></h2>
      <div class="toggle-row">
        <span>Auto Exposure</span>
        <label class="switch">
          <input type="checkbox" id="ae-toggle" onchange="sendImage({auto_exposure: this.checked})">
          <span class="slider"></span>
        </label>
      </div>

      <div class="form-group">
        <label>ISO Sensitivity <span class="val" id="val-iso">100</span></label>
        <input type="range" id="iso" min="100" max="3200" step="50" value="100" oninput="updateLabel('val-iso', this.value)" onchange="sendImage({iso: parseInt(this.value), auto_exposure: false})">
      </div>

      <div class="form-group">
        <label>EV Compensation <span class="val" id="val-ev">0.0</span></label>
        <input type="range" id="ev" min="-30" max="30" value="0" oninput="updateLabel('val-ev', (this.value/10).toFixed(1))" onchange="sendImage({ev_compensation: parseFloat(this.value)/10})">
      </div>

      <div class="toggle-row">
        <span>Auto White Balance</span>
        <label class="switch">
          <input type="checkbox" id="awb-toggle" onchange="sendImage({wb_auto: this.checked})">
          <span class="slider"></span>
        </label>
      </div>

      <div class="form-group">
        <label>Color Temp <span class="val" id="val-wb">5000K</span></label>
        <input type="range" id="wb" min="2000" max="10000" step="100" value="5000" oninput="updateLabel('val-wb', this.value+'K')" onchange="sendImage({wb_kelvin: parseInt(this.value), wb_auto: false})">
      </div>

      <div class="form-group">
        <label>Brightness <span class="val" id="val-bright">50</span></label>
        <input type="range" id="bright" min="0" max="100" value="50" oninput="updateLabel('val-bright', this.value)" onchange="sendImage({brightness: parseInt(this.value)})">
      </div>

      <div class="form-group">
        <label>Contrast <span class="val" id="val-contrast">50</span></label>
        <input type="range" id="contrast" min="0" max="100" value="50" oninput="updateLabel('val-contrast', this.value)" onchange="sendImage({contrast: parseInt(this.value)})">
      </div>

      <div class="form-group">
        <label>Hardware Color Filter</label>
        <select id="filter-mode" onchange="sendImage({filter_mode: this.value})">
          <option value="None">None (Natural)</option>
          <option value="Style1">Style 1</option>
          <option value="Style2">Style 2</option>
          <option value="Fresh">Fresh</option>
          <option value="Amber">Amber</option>
          <option value="Film1">Film 1</option>
          <option value="Film2">Film 2</option>
          <option value="Cool">Cool</option>
          <option value="Warm">Warm</option>
        </select>
      </div>
    </div>

    <!-- Audio & System -->
    <div class="card">
      <h2>Audio & System <span>⚙️</span></h2>
      <div class="form-group">
        <label>Microphone Capture Mode</label>
        <select id="audio-mode" onchange="sendAudio({audio_mode: this.value})">
          <option value="VoiceFocus">Voice Focus (Directional)</option>
          <option value="VoiceSuppression">Noise Suppression (Wide)</option>
          <option value="MusicBalance">Music Balance</option>
          <option value="Original">Original Sound</option>
          <option value="SoundWall">Sound Wall (Audio Fence)</option>
        </select>
      </div>

      <div class="toggle-row">
        <span>AI Microphone Denoise</span>
        <label class="switch">
          <input type="checkbox" id="denoise-toggle" onchange="sendAudio({audio_denoise: this.checked})">
          <span class="slider"></span>
        </label>
      </div>

      <div class="form-group">
        <label>Anti-Flicker Power Line Frequency</label>
        <select id="power-line" onchange="sendSystem({power_line_frequency: this.value})">
          <option value="Disabled">Disabled</option>
          <option value="Hz50">50 Hz</option>
          <option value="Hz60">60 Hz</option>
          <option value="Auto">Auto</option>
        </select>
      </div>

      <div class="toggle-row">
        <span>Privacy Sleep Mode</span>
        <label class="switch">
          <input type="checkbox" id="privacy-toggle" onchange="sendSystem({privacy_mode: this.checked})">
          <span class="slider"></span>
        </label>
      </div>

      <div class="toggle-row">
        <span>Horizontal Level Correction</span>
        <label class="switch">
          <input type="checkbox" id="horiz-toggle" onchange="sendSystem({horizontal_correction: this.checked})">
          <span class="slider"></span>
        </label>
      </div>

      <div style="margin-top: 1rem;">
        <button class="danger" style="width: 100%;" onclick="if(confirm('Restore camera factory defaults?')) sendSystem({factory_reset: true})">Factory Reset Defaults</button>
      </div>
    </div>
  </div>

  <script>
    let ws = null;
    let stateCache = null;

    function connectWs() {
      const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
      const wsUrl = `${proto}//${location.host}/ws`;
      ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        document.getElementById('ws-dot').className = 'dot online';
        document.getElementById('ws-status').textContent = 'Live (WS)';
      };

      ws.onclose = () => {
        document.getElementById('ws-dot').className = 'dot';
        document.getElementById('ws-status').textContent = 'Reconnecting...';
        setTimeout(connectWs, 2000);
      };

      ws.onmessage = (evt) => {
        try {
          const snapshot = JSON.parse(evt.data);
          renderSnapshot(snapshot);
        } catch (e) {
          console.error('Failed to parse snapshot:', e);
        }
      };
    }

    function sendCommand(type, payload) {
      const msg = { type, ...payload };
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(msg));
      } else {
        fetch(`/api/${type}`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        })
        .then(res => res.json())
        .then(snapshot => renderSnapshot(snapshot))
        .catch(err => console.error(err));
      }
    }

    function sendPtz(payload) { sendCommand('ptz', payload); }
    function sendMode(mode) { sendCommand('mode', { mode }); }
    function sendPreset(action, index) { sendCommand('preset', { action, index }); }
    function sendImage(payload) { sendCommand('image', payload); }
    function sendTracking(payload) { sendCommand('tracking', payload); }
    function sendAudio(payload) { sendCommand('audio', payload); }
    function sendSystem(payload) { sendCommand('system', payload); }

    function updateLabel(id, text) {
      document.getElementById(id).textContent = text;
    }

    let moveInterval = null;
    function startMove(dirX, dirY) {
      stopMove();
      sendPtz({ pan_speed: dirX !== 0 ? 12 : 0, pan_dir: dirX, tilt_speed: dirY !== 0 ? 12 : 0, tilt_dir: dirY });
    }
    function stopMove() {
      sendPtz({ stop: true });
    }

    function toggleGesture(name) {
      if (!stateCache) return;
      const gEntry = stateCache.gestures.find(pair => pair[0] === name);
      const cur = gEntry ? gEntry[1] : false;
      sendTracking({ gesture: [name, !cur] });
    }

    function renderSnapshot(s) {
      stateCache = s;
      document.getElementById('cam-model').textContent = s.model;
      document.getElementById('cam-details').textContent = `${s.device_info.device_name || 'Insta360 Camera'} • FW: ${s.device_info.firmware_version || 'N/A'} • SN: ${s.device_info.serial_number || 'N/A'}`;

      // Sliders & Toggles
      document.getElementById('zoom').value = s.zoom;
      updateLabel('val-zoom', s.zoom + '%');

      document.getElementById('roll').value = s.roll;
      updateLabel('val-roll', (s.roll / 10).toFixed(1) + '°');

      document.getElementById('af-toggle').checked = s.auto_focus;
      document.getElementById('focus').value = s.focus;
      updateLabel('val-focus', s.focus);

      document.getElementById('ae-toggle').checked = s.auto_exposure;
      document.getElementById('iso').value = s.iso;
      updateLabel('val-iso', s.iso);

      document.getElementById('ev').value = Math.round(s.ev_compensation * 10);
      updateLabel('val-ev', s.ev_compensation.toFixed(1));

      document.getElementById('awb-toggle').checked = s.wb_auto;
      document.getElementById('wb').value = s.wb_kelvin;
      updateLabel('val-wb', s.wb_kelvin + 'K');

      document.getElementById('bright').value = s.brightness;
      updateLabel('val-bright', s.brightness);

      document.getElementById('contrast').value = s.contrast;
      updateLabel('val-contrast', s.contrast);

      document.getElementById('filter-mode').value = s.filter_mode;
      document.getElementById('audio-mode').value = s.audio_mode;
      document.getElementById('denoise-toggle').checked = s.audio_denoise;
      document.getElementById('power-line').value = s.power_line_frequency;
      document.getElementById('privacy-toggle').checked = s.privacy_mode;
      document.getElementById('horiz-toggle').checked = s.horizontal_correction;

      // Mode buttons
      const modeContainer = document.getElementById('mode-buttons');
      Array.from(modeContainer.children).forEach(btn => {
        btn.className = (btn.textContent.trim().replace(/\s/g,'') === s.video_mode) ? 'active' : '';
      });

      // Tracking speed
      ['Slow', 'Normal', 'Fast'].forEach(spd => {
        const el = document.getElementById('spd-' + spd);
        if (el) el.className = (s.track_speed === spd) ? 'active' : '';
      });

      // Composition
      ['Head', 'HalfBody', 'WholeBody'].forEach(c => {
        const el = document.getElementById('comp-' + c);
        if (el) el.className = (s.composition_style === c) ? 'active' : '';
      });

      // Gestures
      const allG = s.extend_functions.find(f => f[0] === 'AllGestures');
      document.getElementById('all-gestures').checked = allG ? allG[1] : true;

      s.gestures.forEach(([name, en]) => {
        const btn = document.getElementById('g-' + name);
        if (btn) btn.className = en ? 'active' : '';
      });
    }

    // Initialize
    connectWs();
    fetch('/api/state')
      .then(r => r.json())
      .then(renderSnapshot)
      .catch(console.error);
  </script>
</body>
</html>
"#;
