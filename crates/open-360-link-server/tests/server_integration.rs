use std::collections::HashMap;
use std::net::SocketAddr;

use open_360_link_protocol::{
    AudioCaptureMode, CameraModel, CompositionStyle, ExtendFunction, FilterMode, Gesture,
    GestureFunction, TrackSpeed, VideoMode,
};
use open_360_link_server::{
    build_router, new_mock_state, AudioRequest, CameraStateSnapshot, ImageRequest, ModeRequest,
    PresetRequest, PtzRequest, SystemRequest, TrackingRequest,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Binds an ephemeral TCP listener on 127.0.0.1:0, attaches the axum router
/// powered by `new_mock_state(model)`, spawns the server onto the tokio runtime,
/// and returns the bound socket address.
async fn start_test_server(model: CameraModel) -> SocketAddr {
    let state = new_mock_state(model);
    let app = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral TCP listener");
    let addr = listener
        .local_addr()
        .expect("Failed to retrieve local socket address");

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    addr
}

/// Sends an HTTP/1.1 request directly over a raw TCP connection (`TcpStream`),
/// eliminating external HTTP client crate requirements.
///
/// Handles chunked reading, `Content-Length` framing, and connection termination.
/// Returns `(status_code, headers, body)`.
async fn send_http_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (u16, HashMap<String, String>, String) {
    let mut stream = TcpStream::connect(addr)
        .await
        .expect("Failed to connect TCP stream to test server");

    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if let Some(b) = body {
        req.push_str("Content-Type: application/json\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    req.push_str("\r\n");
    if let Some(b) = body {
        req.push_str(b);
    }

    stream
        .write_all(req.as_bytes())
        .await
        .expect("Failed to write HTTP request to TCP stream");
    stream.flush().await.expect("Failed to flush TCP stream");

    let mut response_buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream
            .read(&mut chunk)
            .await
            .expect("Failed to read HTTP response chunk from TCP stream");
        if n == 0 {
            break;
        }
        response_buf.extend_from_slice(&chunk[..n]);

        if let Some(pos) = response_buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let header_str = String::from_utf8_lossy(&response_buf[..pos]);
            let mut content_len = None;
            for line in header_str.split("\r\n") {
                if let Some((k, v)) = line.split_once(':') {
                    if k.trim().eq_ignore_ascii_case("content-length") {
                        if let Ok(cl) = v.trim().parse::<usize>() {
                            content_len = Some(cl);
                        }
                    }
                }
            }
            if let Some(cl) = content_len {
                if response_buf.len() >= pos + 4 + cl {
                    break;
                }
            }
        }
    }

    let header_end = response_buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("Malformed HTTP response: missing header delimiter CRLF CRLF");
    let header_bytes = &response_buf[..header_end];
    let body_bytes = &response_buf[header_end + 4..];

    let header_str = std::str::from_utf8(header_bytes).expect("Non-UTF8 HTTP headers");
    let mut lines = header_str.split("\r\n");
    let status_line = lines.next().expect("Missing HTTP status line");
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .expect("Missing status code in HTTP status line")
        .parse()
        .expect("Invalid integer status code in HTTP response");

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    let body_str = String::from_utf8_lossy(body_bytes).to_string();
    (status_code, headers, body_str)
}

/// Helper for GET requests.
async fn send_http_get(addr: SocketAddr, path: &str) -> (u16, HashMap<String, String>, String) {
    send_http_request(addr, "GET", path, None).await
}

/// Helper for JSON POST requests.
async fn send_http_json(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, HashMap<String, String>, String) {
    send_http_request(addr, method, path, Some(body)).await
}

#[test]
fn test_request_structs_exports() {
    // Verify that all documented request structs deserialize properly
    let _ptz: PtzRequest = serde_json::from_str(
        r#"{"pan_arcsec": 36000, "tilt_arcsec": -18000, "zoom": 250}"#,
    )
    .expect("PtzRequest deserialization failed");

    let _mode: ModeRequest = serde_json::from_str(
        r#"{"mode": "DeskView", "pitch": -450, "zoom": 150}"#,
    )
    .expect("ModeRequest deserialization failed");

    let _preset: PresetRequest = serde_json::from_str(
        r#"{"action": "save", "index": 3}"#,
    )
    .expect("PresetRequest deserialization failed");

    let _img: ImageRequest = serde_json::from_str(
        r#"{"brightness": 75, "contrast": 65, "wb_auto": false, "wb_kelvin": 5600, "hdr": true, "filter_mode": "Warm"}"#,
    )
    .expect("ImageRequest deserialization failed");

    let _track: TrackingRequest = serde_json::from_str(
        r#"{"track_speed": "Fast", "composition_style": "WholeBody", "gesture_binding": ["Palm", "AutoFraming"]}"#,
    )
    .expect("TrackingRequest deserialization failed");

    let _audio: AudioRequest = serde_json::from_str(
        r#"{"audio_mode": "VoiceSuppression"}"#,
    )
    .or_else(|_| serde_json::from_str(r#"{"mode": "VoiceSuppression"}"#))
    .expect("AudioRequest deserialization failed");

    let _sys: SystemRequest = serde_json::from_str(
        r#"{"privacy_mode": true, "horizontal_correction": true}"#,
    )
    .expect("SystemRequest deserialization failed");
}

/// 1. GET / returns 200 OK with text/html containing the embedded Web Remote Control UI.
#[tokio::test]
async fn test_web_remote_control_ui() {
    let addr = start_test_server(CameraModel::Link2).await;
    let (status, headers, body) = send_http_get(addr, "/").await;

    assert_eq!(status, 200, "GET / did not return 200 OK");
    let content_type = headers
        .get("content-type")
        .expect("Missing Content-Type header on GET /");
    assert!(
        content_type.contains("text/html"),
        "Content-Type should be text/html, got '{content_type}'"
    );
    assert!(
        body.contains("<!DOCTYPE html>")
            || body.contains("<!doctype html>")
            || body.contains("<html")
            || body.contains("Insta360"),
        "GET / body did not contain expected HTML UI content. Body preview: {}",
        &body[..body.len().min(300)]
    );
}

/// 2. GET /api/state returns 200 OK and deserializes cleanly into CameraStateSnapshot
/// with CameraModel::Link2 and zoom == 100.
#[tokio::test]
async fn test_get_api_state_initial() {
    let addr = start_test_server(CameraModel::Link2).await;
    let (status, headers, body) = send_http_get(addr, "/api/state").await;

    assert_eq!(status, 200, "GET /api/state did not return 200 OK");
    let content_type = headers
        .get("content-type")
        .expect("Missing Content-Type header on GET /api/state");
    assert!(
        content_type.contains("application/json"),
        "Content-Type should be application/json, got '{content_type}'"
    );

    let snapshot: CameraStateSnapshot = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("Failed to deserialize CameraStateSnapshot: {e}\nPayload: {body}"));

    assert_eq!(snapshot.model, CameraModel::Link2);
    assert_eq!(snapshot.zoom, 100);
}

/// 3. POST /api/ptz with {"pan_arcsec": 36000, "tilt_arcsec": -18000, "zoom": 250}
/// updates pan_arcsec, tilt_arcsec, zoom, and real_zoom.
#[tokio::test]
async fn test_post_api_ptz() {
    let addr = start_test_server(CameraModel::Link2).await;

    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/ptz",
        r#"{"pan_arcsec": 36000, "tilt_arcsec": -18000, "zoom": 250}"#,
    )
    .await;
    assert_eq!(status, 200, "POST /api/ptz failed");

    let (status, _, body) = send_http_get(addr, "/api/state").await;
    assert_eq!(status, 200);

    let snapshot: CameraStateSnapshot = serde_json::from_str(&body)
        .expect("Failed to deserialize CameraStateSnapshot after PTZ update");

    assert_eq!(snapshot.pan_arcsec, 36000);
    assert_eq!(snapshot.tilt_arcsec, -18000);
    assert_eq!(snapshot.zoom, 250);
    assert_eq!(snapshot.real_zoom, 250);
}

/// 4. POST /api/mode with {"mode": "DeskView", "pitch": -450, "zoom": 150}
/// switches video_mode to VideoMode::DeskView and updates video_aux.pitch to -450.
#[tokio::test]
async fn test_post_api_mode() {
    let addr = start_test_server(CameraModel::Link2).await;

    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/mode",
        r#"{"mode": "DeskView", "pitch": -450, "zoom": 150}"#,
    )
    .await;
    assert_eq!(status, 200, "POST /api/mode failed");

    let (status, _, body) = send_http_get(addr, "/api/state").await;
    assert_eq!(status, 200);

    let snapshot: CameraStateSnapshot = serde_json::from_str(&body)
        .expect("Failed to deserialize CameraStateSnapshot after mode update");

    assert_eq!(snapshot.video_mode, VideoMode::DeskView);
    assert_eq!(snapshot.video_aux.pitch, -450);
}

/// 5. POST /api/image with {"brightness": 75, "contrast": 65, "wb_auto": false, "wb_kelvin": 5600, "hdr": true, "filter_mode": "Warm"}
/// updates all image parameters and sets ExtendFunction::Hdr to true.
#[tokio::test]
async fn test_post_api_image() {
    let addr = start_test_server(CameraModel::Link2).await;

    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/image",
        r#"{"brightness": 75, "contrast": 65, "wb_auto": false, "wb_kelvin": 5600, "hdr": true, "filter_mode": "Warm"}"#,
    )
    .await;
    assert_eq!(status, 200, "POST /api/image failed");

    let (status, _, body) = send_http_get(addr, "/api/state").await;
    assert_eq!(status, 200);

    let snapshot: CameraStateSnapshot = serde_json::from_str(&body)
        .expect("Failed to deserialize CameraStateSnapshot after image update");

    assert_eq!(snapshot.brightness, 75);
    assert_eq!(snapshot.contrast, 65);
    assert_eq!(snapshot.wb_auto, false);
    assert_eq!(snapshot.wb_kelvin, 5600);
    assert_eq!(snapshot.filter_mode, FilterMode::Warm);

    let hdr_enabled = snapshot
        .extend_functions
        .iter()
        .any(|(f, en)| *f == ExtendFunction::Hdr && *en);
    assert!(hdr_enabled, "ExtendFunction::Hdr was not set to true in snapshot.extend_functions");
}

/// 6. POST /api/tracking with {"track_speed": "Fast", "composition_style": "WholeBody", "gesture_binding": ["Palm", "AutoFraming"]}
/// updates tracking speed, composition style, and gesture bindings.
#[tokio::test]
async fn test_post_api_tracking() {
    let addr = start_test_server(CameraModel::Link2).await;

    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/tracking",
        r#"{"track_speed": "Fast", "composition_style": "WholeBody", "gesture_binding": ["Palm", "AutoFraming"]}"#,
    )
    .await;
    assert_eq!(status, 200, "POST /api/tracking failed");

    let (status, _, body) = send_http_get(addr, "/api/state").await;
    assert_eq!(status, 200);

    let snapshot: CameraStateSnapshot = serde_json::from_str(&body)
        .expect("Failed to deserialize CameraStateSnapshot after tracking update");

    assert_eq!(snapshot.track_speed, TrackSpeed::Fast);
    assert_eq!(snapshot.composition_style, CompositionStyle::WholeBody);

    let palm_binding = snapshot
        .gesture_bindings
        .iter()
        .find(|(g, _)| *g == Gesture::Palm);
    assert_eq!(
        palm_binding.map(|(_, f)| *f),
        Some(GestureFunction::AutoFraming),
        "Expected Palm gesture binding to be AutoFraming"
    );
}

/// 7. POST /api/preset with {"action": "save", "index": 3} sets bit 3 in preset_flags.slot_mask.
#[tokio::test]
async fn test_post_api_preset() {
    let addr = start_test_server(CameraModel::Link2).await;

    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/preset",
        r#"{"action": "save", "index": 3}"#,
    )
    .await;
    assert_eq!(status, 200, "POST /api/preset failed");

    let (status, _, body) = send_http_get(addr, "/api/state").await;
    assert_eq!(status, 200);

    let snapshot: CameraStateSnapshot = serde_json::from_str(&body)
        .expect("Failed to deserialize CameraStateSnapshot after preset update");

    assert_ne!(
        snapshot.preset_flags.slot_mask & (1 << 3),
        0,
        "Bit 3 in preset_flags.slot_mask should be set after saving preset slot 3"
    );
}

/// 8. POST /api/audio and POST /api/system update audio_mode, privacy_mode, and horizontal_correction.
#[tokio::test]
async fn test_post_api_audio_and_system() {
    let addr = start_test_server(CameraModel::Link2).await;

    // Audio update
    let mut audio_res = send_http_json(
        addr,
        "POST",
        "/api/audio",
        r#"{"audio_mode": "VoiceSuppression"}"#,
    )
    .await;
    if audio_res.0 != 200 {
        audio_res = send_http_json(
            addr,
            "POST",
            "/api/audio",
            r#"{"mode": "VoiceSuppression"}"#,
        )
        .await;
    }
    assert_eq!(audio_res.0, 200, "POST /api/audio failed");

    let (status, _, body) = send_http_get(addr, "/api/state").await;
    assert_eq!(status, 200);
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body)
        .expect("Failed to deserialize CameraStateSnapshot after audio update");
    assert_eq!(snapshot.audio_mode, AudioCaptureMode::VoiceSuppression);

    // System update
    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/system",
        r#"{"privacy_mode": true, "horizontal_correction": true}"#,
    )
    .await;
    assert_eq!(status, 200, "POST /api/system failed");

    let (status, _, body) = send_http_get(addr, "/api/state").await;
    assert_eq!(status, 200);
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body)
        .expect("Failed to deserialize CameraStateSnapshot after system update");
    assert!(snapshot.privacy_mode, "privacy_mode should be true");
    assert!(
        snapshot.horizontal_correction,
        "horizontal_correction should be true"
    );
}

/// Complete end-to-end integration flow testing all 8 steps sequentially on a single
/// server instance over TCP sockets.
#[tokio::test]
async fn test_server_full_integration_flow() {
    let addr = start_test_server(CameraModel::Link2).await;

    // 1. GET / returns 200 OK with text/html UI
    let (status, headers, body) = send_http_get(addr, "/").await;
    assert_eq!(status, 200);
    assert!(headers.get("content-type").unwrap().contains("text/html"));
    assert!(
        body.contains("<!DOCTYPE html>")
            || body.contains("<!doctype html>")
            || body.contains("<html")
            || body.contains("Insta360")
    );

    // 2. GET /api/state initial state
    let (status, _, body) = send_http_get(addr, "/api/state").await;
    assert_eq!(status, 200);
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body).unwrap();
    assert_eq!(snapshot.model, CameraModel::Link2);
    assert_eq!(snapshot.zoom, 100);

    // 3. POST /api/ptz
    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/ptz",
        r#"{"pan_arcsec": 36000, "tilt_arcsec": -18000, "zoom": 250}"#,
    )
    .await;
    assert_eq!(status, 200);

    let (_, _, body) = send_http_get(addr, "/api/state").await;
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body).unwrap();
    assert_eq!(snapshot.pan_arcsec, 36000);
    assert_eq!(snapshot.tilt_arcsec, -18000);
    assert_eq!(snapshot.zoom, 250);
    assert_eq!(snapshot.real_zoom, 250);

    // 4. POST /api/mode
    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/mode",
        r#"{"mode": "DeskView", "pitch": -450, "zoom": 150}"#,
    )
    .await;
    assert_eq!(status, 200);

    let (_, _, body) = send_http_get(addr, "/api/state").await;
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body).unwrap();
    assert_eq!(snapshot.video_mode, VideoMode::DeskView);
    assert_eq!(snapshot.video_aux.pitch, -450);

    // 5. POST /api/image
    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/image",
        r#"{"brightness": 75, "contrast": 65, "wb_auto": false, "wb_kelvin": 5600, "hdr": true, "filter_mode": "Warm"}"#,
    )
    .await;
    assert_eq!(status, 200);

    let (_, _, body) = send_http_get(addr, "/api/state").await;
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body).unwrap();
    assert_eq!(snapshot.brightness, 75);
    assert_eq!(snapshot.contrast, 65);
    assert_eq!(snapshot.wb_auto, false);
    assert_eq!(snapshot.wb_kelvin, 5600);
    assert_eq!(snapshot.filter_mode, FilterMode::Warm);
    assert!(snapshot
        .extend_functions
        .iter()
        .any(|(f, en)| *f == ExtendFunction::Hdr && *en));

    // 6. POST /api/tracking
    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/tracking",
        r#"{"track_speed": "Fast", "composition_style": "WholeBody", "gesture_binding": ["Palm", "AutoFraming"]}"#,
    )
    .await;
    assert_eq!(status, 200);

    let (_, _, body) = send_http_get(addr, "/api/state").await;
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body).unwrap();
    assert_eq!(snapshot.track_speed, TrackSpeed::Fast);
    assert_eq!(snapshot.composition_style, CompositionStyle::WholeBody);
    let palm_binding = snapshot
        .gesture_bindings
        .iter()
        .find(|(g, _)| *g == Gesture::Palm);
    assert_eq!(
        palm_binding.map(|(_, f)| *f),
        Some(GestureFunction::AutoFraming)
    );

    // 7. POST /api/preset
    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/preset",
        r#"{"action": "save", "index": 3}"#,
    )
    .await;
    assert_eq!(status, 200);

    let (_, _, body) = send_http_get(addr, "/api/state").await;
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body).unwrap();
    assert_ne!(snapshot.preset_flags.slot_mask & (1 << 3), 0);

    // 8. POST /api/audio and POST /api/system
    let mut audio_res = send_http_json(
        addr,
        "POST",
        "/api/audio",
        r#"{"audio_mode": "VoiceSuppression"}"#,
    )
    .await;
    if audio_res.0 != 200 {
        audio_res = send_http_json(
            addr,
            "POST",
            "/api/audio",
            r#"{"mode": "VoiceSuppression"}"#,
        )
        .await;
    }
    assert_eq!(audio_res.0, 200);

    let (status, _, _) = send_http_json(
        addr,
        "POST",
        "/api/system",
        r#"{"privacy_mode": true, "horizontal_correction": true}"#,
    )
    .await;
    assert_eq!(status, 200);

    let (_, _, body) = send_http_get(addr, "/api/state").await;
    let snapshot: CameraStateSnapshot = serde_json::from_str(&body).unwrap();
    assert_eq!(snapshot.audio_mode, AudioCaptureMode::VoiceSuppression);
    assert!(snapshot.privacy_mode);
    assert!(snapshot.horizontal_correction);
}
