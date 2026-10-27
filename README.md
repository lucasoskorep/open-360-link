# Open 360 Link

A pure-Rust, cross-platform (**Linux**, **Windows**, **macOS**) hardware controller suite for the **Insta360 Link**, **Insta360 Link 2**, **Insta360 Link 2C**, and **Insta360 Link 2 Pro (PUC3)** 4K AI webcams.

Built by reverse-engineering the proprietary USB & UVC Extension Unit (`XU1` and `XU2`) protocol from the official Insta360 Link Controller desktop binary, this workspace provides full hardware control without proprietary drivers, telemetry, or OS lock-in—including first-class **Linux** support.

📄 **Protocol Specification**: See [`docs/PROTOCOL.md`](docs/PROTOCOL.md) for the complete byte-level UVC Extension Unit documentation.

---

## Workspace Crates

| Crate / Binary | Description |
| :--- | :--- |
| **`open-360-link-protocol`** | Core Rust library implementing USB device discovery, UVC VideoControl descriptor parsing, standard UVC `CT_*`/`PU_*` controls, proprietary `XU1`/`XU2` codecs, `NusbTransport` (pure-Rust USB via `nusb`), and a stateful `MockCameraTransport` simulator. |
| **`open-360-link-ctl`** (`open-360-link-cli`) | Command-line tool for scripting PTZ movement, presets, video modes (`Normal`, `AutoTracking`, `WhiteBoard`, `OverheadShot`, `DeskView`, `AutoFraming`, `SmartWhiteBoard`), exposure/ISO, white balance, audio beamforming modes, filters, and extended hardware switches. |
| **`open-360-link-server`** | Asynchronous HTTP REST & WebSocket daemon (`axum` + `tokio`) for remote control, Stream Deck / OBS integration, and real-time state synchronization. |
| **`open-360-link-gui`** | Native cross-platform desktop GUI (`egui` / `eframe`) featuring a virtual gimbal PTZ joystick, AI mode switcher, image adjustment sliders, audio & filter controls, and live camera preview/recording. |

---

## Supported Hardware

All cameras under Arashi Vision USB Vendor ID `0x2e1a`:

| Model | USB `idVendor:idProduct` | Physical Gimbal | Supported Features |
| :--- | :--- | :---: | :--- |
| **Insta360 Link** (1st Gen) | `2e1a:4c01` | 3-Axis | PTZ, Roll, AI Tracking, Whiteboard, Overhead, DeskView, Gestures, HDR, Exposure Curve |
| **Insta360 Link 2** | `2e1a:4c04` | 2-Axis | PTZ, AI Tracking, Auto Framing, Smart Whiteboard, DeskView, Overhead, Audio Modes, Filters, Templates, Horizon Leveling, Forbidden Areas |
| **Insta360 Link 2C** | `2e1a:4c03` | Fixed (Digital Host PTZ) | Digital Host PTZ, Auto Framing, Smart Whiteboard, DeskView, Audio Modes, Filters, Templates, Privacy Mode |
| **Insta360 Link 2 Pro (PUC3)** | `2e1a:4c02` / `4c05` / `4c06` | 2-Axis / Fixed | Full Link 2 / 2C feature set + Sound Wall (`AudioCaptureMode::SoundWall`) |

---

## Quickstart with `mise`

This project includes a [`mise.toml`](mise.toml) configuration that manages the Rust toolchain and exposes convenient task runners:

```bash
# Trust and install toolchain
mise trust
mise install

# Build & launch the desktop GUI
mise run gui

# Run the CLI (e.g., list cameras or inspect camera info)
mise run cli -- list
mise run cli -- info

# Run automated live hardware GUI verification
mise run self-test
```

## Building from Source

Requires Rust `1.78+` (no C/C++ `libusb` system dependency required—uses pure-Rust [`nusb`](https://docs.rs/nusb)):

```bash
cargo build --release
```

The compiled binaries will be placed in `target/release/`:
- `target/release/open-360-link-ctl`
- `target/release/open-360-link-server`
- `target/release/open-360-link-gui`

---

## Platform Notes

### Linux (`V4L2` & `uvcvideo`)

On Linux, `open-360-link-protocol` communicates directly with the primary V4L2 video capture node (`/dev/video*`) using `UVCIOC_CTRL_QUERY` for proprietary Insta360 Extension Unit (`XU1`/`XU2`) commands and standard V4L2 `VIDIOC_G_CTRL`/`VIDIOC_S_CTRL` ioctls for Processing Unit / Camera Terminal controls, while reading USB descriptors from sysfs. Because `systemd-logind` automatically grants the active desktop user session access to `/dev/video*`, **no custom `udev` rules or `sudo` privileges are required**.

#### Arch Linux & Arch Linux ARM (AUR)

```bash
yay -S open-360-link-bin
```

### macOS

On macOS, `nusb` uses `IOKit` to send UVC class-specific control requests directly to Endpoint 0 without detaching the macOS `UVCAssistant` / `AVFoundation` camera stream. No kernel extensions or extra permissions are required.

---

## Usage Examples

### 1. Command-Line Interface (`open-360-link-ctl`)

> **Tip**: Pass `--mock link2` (`link`, `link2`, or `link2c`) to test any command against the simulated camera without physical hardware attached.

```bash
# List connected Insta360 Link cameras and discovered UVC unit IDs
cargo run -p open-360-link-cli -- list

# Show complete camera status, firmware version, PTZ angles, and active features
cargo run -p open-360-link-cli -- info

# Move gimbal to absolute pan/tilt angles and set zoom (100 = 1.0x, 200 = 2.0x)
cargo run -p open-360-link-cli -- ptz set-abs --pan 54000 --tilt -36000
cargo run -p open-360-link-cli -- ptz zoom 150

# Center / home the gimbal
cargo run -p open-360-link-cli -- ptz reset

# Switch AI Video Mode (normal, auto-tracking, whiteboard, overhead, deskview, auto-framing, smart-whiteboard)
cargo run -p open-360-link-cli -- mode set auto-tracking
cargo run -p open-360-link-cli -- tracking speed fast

# Configure image settings (brightness, contrast, saturation, sharpness, white balance, HDR)
cargo run -p open-360-link-cli -- image brightness 60
cargo run -p open-360-link-cli -- image hdr true

# Configure microphone beamforming mode (voice-focus, noise-reduction, music-mode, original, sound-wall)
cargo run -p open-360-link-cli -- audio mode voice-focus
cargo run -p open-360-link-cli -- audio denoise true
```

### 2. HTTP & WebSocket Server (`open-360-link-server`)

Start the daemon on `127.0.0.1:9360` (use `--mock link2` if no physical camera is connected):

```bash
cargo run -p open-360-link-server -- --bind 127.0.0.1:9360 --mock link2
```

Example REST API calls:

```bash
# List discovered cameras
curl http://127.0.0.1:9360/api/cameras

# Fetch full camera state snapshot
curl http://127.0.0.1:9360/api/state

# Set PTZ pan, tilt, or zoom
curl -X POST http://127.0.0.1:9360/api/ptz \
  -H "Content-Type: application/json" \
  -d '{"pan_deg": 20.0, "tilt_deg": -5.0, "zoom": 180}'

# Switch video mode
curl -X POST http://127.0.0.1:9360/api/mode \
  -H "Content-Type: application/json" \
  -d '{"mode": "DeskView"}'
```

Connect to `ws://127.0.0.1:9360/ws` over WebSockets to receive real-time JSON state broadcasts and send low-latency PTZ/control commands.

### 3. Desktop GUI (`open-360-link-gui`)

Launch the native cross-platform GUI (automatically connects to a physical Insta360 Link camera if found):

```bash
cargo run -p open-360-link-gui
```

---

## Roadmap / TODO

- [ ] **PTZ Gimbal Presets**: Save, recall, rename, and delete up to 6 hardware gimbal preset positions (`XU1 0x0e` / `0x11` & `0x1a`) with live thumbnail preview cards in the right sidebar.
- [ ] **Privacy Mode / Sleep Mode**: Hardware privacy sleep toggle (`XU2 0x0f`) and Mute in Privacy Mode (`XU1 0x1b` `UltimatePrivacyMode` bit 11) with gimbal-down standby synchronization.
- [ ] **Smart Composition**: AI Zoom (`XU1 0x1b` bit 0) and Head / Half Body / Whole Body / Custom composition framing (`XU1 0x13` & `0x18`) during AI Tracking.

---

## Protocol Documentation

For full reverse-engineering details—including disassembly addresses from `Webcam-desktop` v2.0.2, mixed-endian UVC Extension Unit GUIDs, `XU1` and `XU2` selector tables, `ExtendFunction` bitmasks, and the USB firmware upgrade state machine—see **[`docs/PROTOCOL.md`](docs/PROTOCOL.md)**.

## License

Licensed under either of **MIT** or **Apache-2.0** at your option.
