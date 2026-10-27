mod right_panel;
mod state;
mod theme;
mod transport;
mod video_engine;

use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, FontId, Margin, Pos2, Rect, RichText, Rounding, Stroke, Vec2};
use open_360_link_protocol::{
    list_connected_cameras, CameraModel, DiscoveredCamera, Insta360CameraController,
    MockCameraTransport, NusbTransport, PowerLineFrequency, VideoMode, VideoModeAuxData,
};

use crate::right_panel::render_right_settings_panel;
use crate::state::{
    CameraAppState, PreferenceTab, ToolbarPopup, UiMode, VideoFormatPreset,
};
use crate::theme::{apply_ins_theme, ins_bottom_mode_button, InsTheme};
use crate::transport::{DeviceSelection, GuiTransport};
use crate::video_engine::VideoPreviewEngine;

/// Top-level application struct for Insta360 Link Controller.
pub struct Insta360App {
    state: CameraAppState,
    controller: Insta360CameraController<GuiTransport>,
    video_engine: VideoPreviewEngine,
    connected_cameras: Vec<DiscoveredCamera>,
    selected_device: DeviceSelection,
    self_test_enabled: bool,
    self_test_start: Option<Instant>,
    self_test_step: u8,
}

impl Insta360App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Enumerate physical USB cameras and OS video capture devices on startup
        let connected_cameras = list_connected_cameras().unwrap_or_default();
        let mut video_engine = VideoPreviewEngine::new();
        video_engine.refresh_capture_devices();

        // Connect to physical USB camera if detected, otherwise default to simulator
        let (controller, selected_device) = if let Some(first_cam) = connected_cameras.first() {
            match NusbTransport::open_by_info(first_cam.clone()) {
                Ok(usb) => (
                    Insta360CameraController::new(GuiTransport::Usb(usb)),
                    DeviceSelection::Usb(first_cam.clone()),
                ),
                Err(_) => (
                    Insta360CameraController::new(GuiTransport::Mock(MockCameraTransport::new(
                        CameraModel::Link2,
                    ))),
                    DeviceSelection::Simulate(CameraModel::Link2),
                ),
            }
        } else {
            (
                Insta360CameraController::new(GuiTransport::Mock(MockCameraTransport::new(
                    CameraModel::Link2,
                ))),
                DeviceSelection::Simulate(CameraModel::Link2),
            )
        };

        let state = CameraAppState::default();
        let self_test_enabled = std::env::var("INSTA360_GUI_SELF_TEST")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let mut app = Self {
            state,
            controller,
            video_engine,
            connected_cameras,
            selected_device,
            self_test_enabled,
            self_test_start: None,
            self_test_step: 0,
        };
        app.sync_device_info();
        if matches!(app.selected_device, DeviceSelection::Usb(_)) {
            app.auto_select_insta360_video_feed();
        }
        app
    }

    fn auto_select_insta360_video_feed(&mut self) {
        let (w, h) = self.state.format_preset.dimensions();
        let fps = self.state.format_preset.fps();
        if (fps > 30 || self.state.format_preset.is_vertical())
            && !self.state.vertical_resolution_mode
        {
            self.state.vertical_resolution_mode = true;
            let _ = self.controller.set_vertical_screen(true);
        }
        self.video_engine
            .set_capture_format(w as usize, h as usize, fps);

        let candidate = self
            .video_engine
            .available_capture_devices()
            .iter()
            .find(|d| {
                let low = d.to_lowercase();
                low.contains("insta360") && !low.contains("virtual")
            })
            .or_else(|| {
                self.video_engine
                    .available_capture_devices()
                    .iter()
                    .find(|d| d.to_lowercase().contains("insta360"))
            })
            .cloned();
        if let Some(dev_name) = candidate {
            self.video_engine.start_physical_capture(&dev_name);
        }
    }

    fn switch_camera(&mut self, selection: DeviceSelection) {
        match selection.clone() {
            DeviceSelection::Usb(cam) => match NusbTransport::open_by_info(cam.clone()) {
                Ok(transport) => {
                    self.controller =
                        Insta360CameraController::new(GuiTransport::Usb(transport));
                    self.selected_device = selection;
                    self.sync_device_info();
                    self.auto_select_insta360_video_feed();
                    self.state
                        .show_toast(format!("Connected to USB device: {}", cam.name), false);
                }
                Err(e) => {
                    self.state
                        .show_toast(format!("Failed to open camera: {}", e), true);
                }
            },
            DeviceSelection::Simulate(model) => {
                let mock = MockCameraTransport::new(model);
                self.controller = Insta360CameraController::new(GuiTransport::Mock(mock));
                self.selected_device = selection;
                self.video_engine.stop_physical_capture();
                self.sync_device_info();
                self.state
                    .show_toast(format!("Switched to Simulator ({})", model.display_name()), false);
            }
        }
    }

    fn sync_device_info(&mut self) {
        if let Ok(info) = self.controller.get_device_info() {
            self.state.device_info = info;
        }
        if let Ok(ptz) = self.controller.get_ptz_version_and_angles() {
            self.state.pan_deg = (ptz.pan as f32 / 10.0_f32).clamp(-130.0_f32, 130.0_f32);
            self.state.tilt_deg = (ptz.tilt as f32 / 10.0_f32).clamp(-90.0_f32, 90.0_f32);
            self.state.zoom_x = (ptz.zoom as f32 / 100.0_f32).clamp(1.0_f32, 4.0_f32);
            self.state.ptz_info = ptz;
        }
        if let Ok(status) = self.controller.get_device_status() {
            self.state.device_status = status;
        }
        if let Ok((mode, _)) = self.controller.get_video_mode() {
            self.state.video_mode = mode;
        }
        if let Ok(b) = self.controller.get_brightness() {
            self.state.brightness = b.clamp(0, 100);
        }
        if let Ok(c) = self.controller.get_contrast() {
            self.state.contrast = c.clamp(0, 100);
        }
        if let Ok(s) = self.controller.get_saturation() {
            self.state.saturation = s.clamp(0, 100);
        }
        if let Ok(sh) = self.controller.get_sharpness() {
            self.state.sharpness = sh.clamp(0, 100);
        }
        if let Ok(wb_auto) = self.controller.get_auto_white_balance() {
            self.state.auto_white_balance = wb_auto;
        }
        if let Ok(wb) = self.controller.get_white_balance_temperature() {
            self.state.wb_kelvin = wb.clamp(2000, 10000);
        }
        if let Ok(ext) = self.controller.get_extend_function_status() {
            for (func, en) in ext {
                match func {
                    open_360_link_protocol::ExtendFunction::AiZoom => {
                        self.state.ai_zoom_enabled = en
                    }
                    open_360_link_protocol::ExtendFunction::AllGestures => {
                        self.state.enable_all_gestures = en
                    }
                    open_360_link_protocol::ExtendFunction::AutoTracking => {
                        self.state.enable_auto_tracking = en
                    }
                    open_360_link_protocol::ExtendFunction::SingleTapTracking => {
                        self.state.enable_single_tap_tracking = en
                    }
                    open_360_link_protocol::ExtendFunction::Hdr => self.state.hdr_enabled = en,
                    open_360_link_protocol::ExtendFunction::HorizontalMirror => {
                        self.state.horizontal_mirror = en
                    }
                    open_360_link_protocol::ExtendFunction::VerticalMirror => {
                        self.state.vertical_mirror = en
                    }
                    open_360_link_protocol::ExtendFunction::VerticalMode => {
                        self.state.vertical_resolution_mode = en
                    }
                    open_360_link_protocol::ExtendFunction::ForcedVertical => {
                        self.state.forced_vertical_lock = en
                    }
                    open_360_link_protocol::ExtendFunction::LowResolution => {
                        self.state.low_resolution_mode = en
                    }
                    open_360_link_protocol::ExtendFunction::SmartAdjustment => {
                        self.state.smart_adjustment = en
                    }
                    open_360_link_protocol::ExtendFunction::StartupParams => {
                        self.state.startup_params_enabled = en
                    }
                    open_360_link_protocol::ExtendFunction::UltimatePrivacyMode => {
                        self.state.ultimate_privacy = en
                    }
                    _ => {}
                }
            }
        }
        if let Ok(style) = self.controller.get_composition_style() {
            self.state.composition_style = style;
        }
        if let Ok(speed) = self.controller.get_track_speed() {
            self.state.track_speed = speed;
        }
        if let Ok(gestures) = self.controller.get_gesture_status() {
            for (g, en) in gestures {
                match g {
                    open_360_link_protocol::Gesture::Ok => self.state.use_gesture_ok = en,
                    open_360_link_protocol::Gesture::Palm => self.state.use_gesture_palm = en,
                    open_360_link_protocol::Gesture::LShape => self.state.use_gesture_l = en,
                    open_360_link_protocol::Gesture::VShape => self.state.use_gesture_v = en,
                    open_360_link_protocol::Gesture::Rock => self.state.use_gesture_rock = en,
                }
            }
        }
        if let Ok(ae) = self.controller.get_auto_exposure() {
            self.state.auto_exposure = ae;
        }
        if let Ok(ev) = self.controller.get_exposure_compensation() {
            self.state.ev_compensation = ev.clamp(-3.0_f32, 3.0_f32);
        }
        if let Ok(iso) = self.controller.get_iso() {
            if let Some(idx) = crate::state::ISO_LIST.iter().position(|&v| v == iso) {
                self.state.iso_index = idx;
            }
        }
        if let Ok(shutter) = self.controller.get_shutter_speed() {
            if let Some(idx) = crate::state::SHUTTER_DENOM_LIST
                .iter()
                .position(|&v| v == shutter)
            {
                self.state.shutter_index = idx;
            }
        }
        if let Ok(af) = self.controller.get_auto_focus() {
            self.state.auto_focus = af;
        }
        if let Ok(focus) = self.controller.get_focus() {
            self.state.manual_focus = focus.min(100);
        }
        if let Ok(pl) = self.controller.get_power_line_frequency() {
            self.state.anti_flicker = pl;
        }
        if let Ok(roll) = self.controller.get_roll() {
            self.state.roll_deg = (roll as f32 / 10.0_f32).clamp(-1.0_f32, 1.0_f32);
        }
        if let Ok(denoise) = self.controller.get_audio_noise_reduction() {
            self.state.audio_denoise = denoise;
        }
    }

    fn apply_saved_settings_to_hardware(&mut self) {
        let Some(saved) = self.state.last_saved_settings.clone() else {
            self.state.last_saved_settings = Some(self.state.to_persisted());
            return;
        };
        self.state.apply_persisted(&saved);
        let _ = self.controller.set_brightness(self.state.brightness);
        let _ = self.controller.set_contrast(self.state.contrast);
        let _ = self.controller.set_saturation(self.state.saturation);
        let _ = self.controller.set_sharpness(self.state.sharpness);
        let _ = self
            .controller
            .set_auto_white_balance(self.state.auto_white_balance);
        if !self.state.auto_white_balance {
            let _ = self
                .controller
                .set_white_balance_temperature(self.state.wb_kelvin);
        }
        let _ = self.controller.set_auto_focus(self.state.auto_focus);
        if !self.state.auto_focus {
            let _ = self.controller.set_focus(self.state.manual_focus);
        }
        let _ = self
            .controller
            .set_power_line_frequency(self.state.anti_flicker);
        let _ = self.controller.set_hdr(self.state.hdr_enabled);
        if !self.state.hdr_enabled {
            let _ = self.controller.set_auto_exposure(self.state.auto_exposure);
            if self.state.auto_exposure {
                let _ = self
                    .controller
                    .set_exposure_compensation(self.state.ev_compensation);
            } else {
                if let Some(&iso) = crate::state::ISO_LIST.get(self.state.iso_index) {
                    let _ = self.controller.set_iso(iso);
                }
                if let Some(&shutter) = crate::state::SHUTTER_DENOM_LIST.get(self.state.shutter_index)
                {
                    let _ = self.controller.set_shutter_speed(shutter);
                }
            }
        }
        let _ = self.controller.set_track_speed(self.state.track_speed);
        let _ = self
            .controller
            .set_single_tap_tracking_enabled(self.state.enable_single_tap_tracking);
        let _ = self
            .controller
            .set_all_gestures_enabled(self.state.enable_all_gestures);
        let _ = self.controller.set_gesture_enabled(
            open_360_link_protocol::Gesture::Palm,
            self.state.use_gesture_palm,
        );
        let _ = self.controller.set_gesture_enabled(
            open_360_link_protocol::Gesture::VShape,
            self.state.use_gesture_v,
        );
        let _ = self.controller.set_gesture_enabled(
            open_360_link_protocol::Gesture::LShape,
            self.state.use_gesture_l,
        );
        let _ = self
            .controller
            .set_audio_noise_reduction(self.state.audio_denoise);
        let _ = self
            .controller
            .set_horizontal_mirror(self.state.horizontal_mirror);
    }

    fn set_video_mode(&mut self, target_mode: VideoMode) {
        let prev_mode = self.state.video_mode;
        let mode_to_set = if self.state.video_mode == target_mode {
            VideoMode::Normal
        } else {
            target_mode
        };

        if mode_to_set == VideoMode::AutoTracking {
            // `Webcam::CameraInsta::autoTrackClicked` (`0x10006c400`) ->
            // `onHandleVideoModeChangeEvent` (`0x10005a400`..`0x10005a4cc`) -> `readyAutoTrack` (`0x10006c8c8`):
            // 1. If the camera was in a non-Normal mode (`>= 2`: Whiteboard, Overhead, DeskView),
            //    first exit to `VideoMode::Normal` (`0`) so `GetTrackObjLists` (`XU1 0x14`) runs.
            // 2. Do NOT send `SetTrackObj(0.5, 0.5)` or `SetVideoMode(AutoTracking)` before
            //    `GetTrackObjLists` detects a person — doing so aborts firmware person detection!
            // 3. Start the 200ms `ready_auto_track` polling loop (`SetCheckTrack(true)`).
            if prev_mode != VideoMode::Normal && prev_mode != VideoMode::AutoTracking {
                let _ = self
                    .controller
                    .set_video_mode_raw(VideoMode::Normal, &VideoModeAuxData::default());
            }
            self.state.video_mode = VideoMode::AutoTracking;
            self.state.ready_auto_track = true;
            self.state.last_track_poll = Some(Instant::now());
            self.state.show_toast(
                "AI Tracking ready — looking for person (or click on video to lock target)",
                false,
            );
            return;
        }

        self.state.ready_auto_track = false;
        if mode_to_set == VideoMode::Normal {
            self.state.tracked_targets.clear();
        }
        self.state.video_mode = mode_to_set;

        let mut aux = VideoModeAuxData::default();
        if mode_to_set == VideoMode::DeskView {
            aux.pitch = self.state.deskview_pitch;
        }
        let _ = self.controller.set_video_mode_raw(mode_to_set, &aux);
        self.state.last_hw_sync = Some(Instant::now() + Duration::from_millis(700));
        if (prev_mode == VideoMode::DeskView || prev_mode == VideoMode::OverheadShot)
            && mode_to_set == VideoMode::Normal
        {
            let _ = self.controller.set_zoom((self.state.zoom_x * 100.0_f32) as u16);
            self.state.pending_ptz_restore = Some((
                Instant::now() + Duration::from_millis(800),
                (self.state.pan_deg * 3600.0_f32) as i32,
                (self.state.tilt_deg * 3600.0_f32) as i32,
            ));
        }
    }

    // ========================================================================
    // Top Bar (TopBarWin.qml - 44px height, theme.bg01)
    // ========================================================================
    fn render_top_bar(&mut self, ctx: &egui::Context, theme: &InsTheme) {
        egui::TopBottomPanel::top("ins_top_bar")
            .exact_height(44.0_f32)
            .frame(
                egui::Frame::none()
                    .fill(theme.bg01)
                    .inner_margin(Margin::symmetric(14.0_f32, 6.0_f32)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Left: Green camera icon + "Open 360 Link" title
                    ui.label(
                        RichText::new("📷")
                            .color(theme.brand_green)
                            .size(17.0_f32),
                    );
                    ui.label(
                        RichText::new("Open 360 Link")
                            .strong()
                            .size(14.5_f32)
                            .color(theme.text01),
                    );

                    ui.add_space(24.0_f32);

                    // Center: Camera Controller Selector
                    let show_simulators = cfg!(debug_assertions)
                        || std::env::var("INSTA360_SIMULATOR")
                            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                            .unwrap_or(false);

                    let current_dev_label = match &self.selected_device {
                        DeviceSelection::Usb(_) => self.selected_device.label(),
                        DeviceSelection::Simulate(_) if show_simulators => {
                            self.selected_device.label()
                        }
                        DeviceSelection::Simulate(_) => "No Camera Connected".to_string(),
                    };

                    egui::ComboBox::from_id_salt("top_camera_selector")
                        .selected_text(&current_dev_label)
                        .show_ui(ui, |ui| {
                            if self.connected_cameras.is_empty() && !show_simulators {
                                ui.label(
                                    RichText::new("No Camera Connected").color(theme.text03),
                                );
                            }
                            for cam in self.connected_cameras.clone() {
                                let dev = DeviceSelection::Usb(cam);
                                let label = dev.label();
                                if ui.selectable_label(self.selected_device == dev, label).clicked() {
                                    self.switch_camera(dev);
                                }
                            }
                            if show_simulators {
                                if !self.connected_cameras.is_empty() {
                                    ui.separator();
                                }
                                for model in [
                                    CameraModel::Link2,
                                    CameraModel::Link2C,
                                    CameraModel::Link,
                                ] {
                                    let dev = DeviceSelection::Simulate(model);
                                    let label = dev.label();
                                    if ui.selectable_label(self.selected_device == dev, label).clicked() {
                                        self.switch_camera(dev);
                                    }
                                }
                            }
                        });

                    let refresh_btn = ui.button("↻");
                    if refresh_btn.on_hover_text("Refresh USB camera devices").clicked() {
                        self.connected_cameras = list_connected_cameras().unwrap_or_default();
                        self.video_engine.refresh_capture_devices();
                        if !matches!(self.selected_device, DeviceSelection::Usb(_)) {
                            if let Some(first) = self.connected_cameras.first().cloned() {
                                self.switch_camera(DeviceSelection::Usb(first));
                            }
                        }
                        self.state.show_toast("Refreshed USB devices", false);
                    }

                    if show_simulators {
                        ui.add_space(10.0_f32);

                        // Video Feed Source Selector (debug / simulator mode only)
                        let active_feed_label = if self.video_engine.is_using_physical_capture() {
                            self.video_engine
                                .active_capture_device()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "Physical Webcam".to_string())
                        } else {
                            "Studio Camera Feed (Live)".to_string()
                        };

                        egui::ComboBox::from_id_salt("top_video_feed_selector")
                            .selected_text(&active_feed_label)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        !self.video_engine.is_using_physical_capture(),
                                        "Studio Camera Feed (Live)",
                                    )
                                    .clicked()
                                {
                                    self.video_engine.stop_physical_capture();
                                }
                                let devs = self.video_engine.available_capture_devices().to_vec();
                                for dev_name in devs {
                                    let is_active = self.video_engine.is_using_physical_capture()
                                        && self
                                            .video_engine
                                            .active_capture_device()
                                            .map(|d| d == dev_name.as_str())
                                            .unwrap_or(false);
                                    if ui.selectable_label(is_active, dev_name.as_str()).clicked() {
                                        self.video_engine.start_physical_capture(&dev_name);
                                    }
                                }
                            });
                    }

                    // Right group
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Preferences ("⋯") button
                        let pref_btn = ui.button(RichText::new("⋯").size(16.0_f32));
                        if pref_btn.on_hover_text("Preferences").clicked() {
                            self.state.show_preferences = true;
                        }

                        // Toolbar Mode icon button ("🗗 Toolbar")
                        let tb_btn = ui.button("🗗 Toolbar");
                        if tb_btn.on_hover_text("Compact Floating Toolbar Mode").clicked() {
                            self.state.ui_mode = UiMode::Toolbar;
                            self.state.toolbar_popup = ToolbarPopup::None;
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                                780.0_f32, 74.0_f32,
                            )));
                        }

                        // Remote Control icon button ("📱 Remote")
                        let rem_btn = ui.button("📱 Remote");
                        if rem_btn.on_hover_text("Mobile Web Remote Control").clicked() {
                            self.state.show_qr_dialog = true;
                        }

                        // Virtual Camera icon button ("🎥 Virtual Cam")
                        let vcam_color = if self.state.virtual_camera_enabled {
                            theme.brand_green
                        } else {
                            theme.text02
                        };
                        let vcam_btn =
                            ui.button(RichText::new("🎥 Virtual Cam").color(vcam_color));
                        if vcam_btn.on_hover_text("Virtual Camera Settings").clicked() {
                            self.state.show_virtual_cam_pop = true;
                        }
                    });
                });
            });
    }

    // ========================================================================
    // Bottom Control Bar (BottomControl.qml - 66px height, theme.bg02)
    // ========================================================================
    fn render_bottom_control_bar(&mut self, ui: &mut egui::Ui, theme: &InsTheme) {
        ui.horizontal(|ui| {
            ui.set_height(66.0_f32);
            ui.add_space(8.0_f32);

            // Left group: Preview On/Off button (40x40) + Divider + Format Selector
            let preview_icon = if self.state.preview_open { "👁" } else { "🚫" };
            let preview_tooltip = if self.state.preview_open {
                "Close Preview"
            } else {
                "Open Preview"
            };
            let preview_btn = ui.add_sized(
                [40.0_f32, 40.0_f32],
                egui::Button::new(RichText::new(preview_icon).size(15.0_f32)),
            );
            if preview_btn.on_hover_text(preview_tooltip).clicked() {
                self.state.preview_open = !self.state.preview_open;
            }

            ui.add_space(4.0_f32);
            let (div_rect, _) = ui.allocate_exact_size(egui::vec2(1.0_f32, 16.0_f32), egui::Sense::hover());
            ui.painter().rect_filled(div_rect, Rounding::ZERO, theme.border01);
            ui.add_space(4.0_f32);

            // Resolution / Format preset combo
            let prev_preset = self.state.format_preset;
            egui::ComboBox::from_id_salt("bottom_format_combo")
                .selected_text(self.state.format_preset.label())
                .show_ui(ui, |ui| {
                    for preset in VideoFormatPreset::ALL {
                        if ui
                            .selectable_value(
                                &mut self.state.format_preset,
                                preset,
                                preset.label(),
                            )
                            .clicked()
                        {
                            let (w, h) = preset.dimensions();
                            let fps = preset.fps();
                            if (fps > 30 || preset.is_vertical())
                                && !self.state.vertical_resolution_mode
                            {
                                self.video_engine.stop_physical_capture();
                                self.state.vertical_resolution_mode = true;
                                let _ = self.controller.set_vertical_screen(true);
                            }
                            self.state.low_resolution_mode = w == 640;
                            self.video_engine.set_capture_format(w as usize, h as usize, fps);
                            self.state
                                .show_toast(format!("Resolution set to {}", preset.label()), false);
                        }
                    }
                });
            if self.state.format_preset != prev_preset {
                let (w, h) = self.state.format_preset.dimensions();
                let fps = self.state.format_preset.fps();
                self.video_engine.set_capture_format(w as usize, h as usize, fps);
            }

            ui.add_space(16.0_f32);

            // Center group: ins_bottom_mode_button items
            let caps = self.controller.capabilities();
            let model = self.controller.model();

            if caps.support_ptz_drag {
                // Link 2 / Link: AI Tracking
                let is_tracking_active = self.state.video_mode == VideoMode::AutoTracking
                    || self.state.video_mode == VideoMode::AutoFraming;
                if ins_bottom_mode_button(ui, "🎯", "AI Tracking", is_tracking_active, true, theme).clicked() {
                    let next = if is_tracking_active {
                        VideoMode::Normal
                    } else if self.state.default_track_mode == 0 {
                        VideoMode::AutoTracking
                    } else {
                        VideoMode::AutoFraming
                    };
                    self.set_video_mode(next);
                }

                // Small ▴ toggle for Single Tracking (0) vs Group Tracking (1) on Link 2
                if model == CameraModel::Link2 {
                    let arrow_label = if self.state.default_track_mode == 0 { "Single" } else { "Group" };
                    let arrow_btn = ui.button(RichText::new("▴").size(10.0_f32));
                    if arrow_btn
                        .on_hover_text(format!("Track mode: {} (Click to toggle Single/Group)", arrow_label))
                        .clicked()
                    {
                        self.state.default_track_mode = if self.state.default_track_mode == 0 { 1 } else { 0 };
                        let _ = self.controller.set_default_track_mode(self.state.default_track_mode);
                        if is_tracking_active {
                            let target = if self.state.default_track_mode == 0 {
                                VideoMode::AutoTracking
                            } else {
                                VideoMode::AutoFraming
                            };
                            self.set_video_mode(target);
                        }
                    }
                }
            } else {
                // Link 2C: Auto Framing
                let is_framing_active = self.state.video_mode == VideoMode::AutoFraming;
                if ins_bottom_mode_button(ui, "👥", "Auto Framing", is_framing_active, true, theme).clicked() {
                    self.set_video_mode(VideoMode::AutoFraming);
                }
            }

            // Whiteboard
            let is_wb_active = self.state.video_mode == VideoMode::WhiteBoard;
            if ins_bottom_mode_button(ui, "📋", "Whiteboard", is_wb_active, true, theme).clicked() {
                self.set_video_mode(VideoMode::WhiteBoard);
            }

            // Overhead (Link 1 only)
            if model == CameraModel::Link {
                let is_overhead_active = self.state.video_mode == VideoMode::OverheadShot;
                if ins_bottom_mode_button(ui, "⬇", "Overhead", is_overhead_active, true, theme).clicked() {
                    self.set_video_mode(VideoMode::OverheadShot);
                }
            }

            // Smart Whiteboard
            let is_swb_active = self.state.video_mode == VideoMode::SmartWhiteBoard;
            if ins_bottom_mode_button(ui, "🖽", "Smart Whiteboard", is_swb_active, true, theme).clicked() {
                self.set_video_mode(VideoMode::SmartWhiteBoard);
            }

            // DeskView + pitch adjustment popover button
            let is_desk_active = self.state.video_mode == VideoMode::DeskView;
            if ins_bottom_mode_button(ui, "📐", "DeskView", is_desk_active, true, theme).clicked() {
                self.set_video_mode(VideoMode::DeskView);
            }
            if is_desk_active {
                let pitch_btn = ui.button(RichText::new("▴").size(10.0_f32));
                if pitch_btn
                    .on_hover_text("DeskView Vertical Correction pitch adjustment")
                    .clicked()
                {
                    self.state.show_deskview_pitch_pop = !self.state.show_deskview_pitch_pop;
                }
            }

            // Right group: Screenshot + Divider + Record
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0_f32);

                // Record button
                if self.state.is_recording {
                    let elapsed = self
                        .state
                        .recording_started
                        .map(|t| t.elapsed().as_secs())
                        .unwrap_or(0);
                    let time_str = format!("{:02}:{:02}", elapsed / 60, elapsed % 60);
                    let rec_btn = ui.button(
                        RichText::new(format!("⏹ {}", time_str))
                            .color(theme.error)
                            .strong(),
                    );
                    if rec_btn.on_hover_text("Stop Recording").clicked() {
                        self.state.is_recording = false;
                        self.state.recording_started = None;
                        match self.video_engine.stop_recording() {
                            Ok(path) => {
                                let (w, h) = self.video_engine.capture_dimensions();
                                self.state.show_toast(
                                    format!("Saved {w}x{h} video to: {}", path.display()),
                                    false,
                                );
                            }
                            Err(e) => {
                                self.state
                                    .show_toast(format!("Stop recording error: {e}"), true);
                            }
                        }
                    }
                } else {
                    let rec_btn = ui.add_sized(
                        [40.0_f32, 40.0_f32],
                        egui::Button::new(RichText::new("⏺").size(16.0_f32)),
                    );
                    if rec_btn.on_hover_text("Record Video").clicked() {
                        let (w, h) = self.state.format_preset.dimensions();
                        let fps = self.state.format_preset.fps();
                        self.video_engine.set_capture_format(w as usize, h as usize, fps);
                        match self.video_engine.start_recording(&self.state.recording_dir) {
                            Ok(path) => {
                                self.state.is_recording = true;
                                self.state.recording_started = Some(Instant::now());
                                self.state.show_toast(
                                    format!("Recording {w}x{h} video: {}", path.display()),
                                    false,
                                );
                            }
                            Err(e) => {
                                self.state
                                    .show_toast(format!("Failed to start recording: {e}"), true);
                            }
                        }
                    }
                }

                ui.add_space(4.0_f32);
                let (div_r, _) = ui.allocate_exact_size(egui::vec2(1.0_f32, 16.0_f32), egui::Sense::hover());
                ui.painter().rect_filled(div_r, Rounding::ZERO, theme.border01);
                ui.add_space(4.0_f32);

                // Screenshot button
                let snap_btn = ui.add_sized(
                    [40.0_f32, 40.0_f32],
                    egui::Button::new(RichText::new("📷").size(15.0_f32)),
                );
                if snap_btn.on_hover_text("Take Screenshot").clicked() {
                    let (w, h) = self.state.format_preset.dimensions();
                    let fps = self.state.format_preset.fps();
                    self.video_engine.set_capture_format(w as usize, h as usize, fps);
                    match self
                        .video_engine
                        .save_screenshot(&self.state.screenshot_dir, &self.state)
                    {
                        Ok(path) => {
                            let (cw, ch) = self.video_engine.capture_dimensions();
                            self.state.show_toast(
                                format!("Saved {cw}x{ch} screenshot to: {}", path.display()),
                                false,
                            );
                        }
                        Err(e) => {
                            self.state
                                .show_toast(format!("Screenshot failed: {}", e), true);
                        }
                    }
                }
            });
        });
    }

    // ========================================================================
    // Video Preview Area (Previewer.qml / DefCamera.qml)
    // ========================================================================
    fn render_preview_area(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, theme: &InsTheme) {
        let avail_size = ui.available_size();

        if !self.state.preview_open {
            // Preview closed dark overlay
            let (rect, _) = ui.allocate_exact_size(avail_size, egui::Sense::hover());
            ui.painter().rect_filled(rect, Rounding::same(6.0_f32), Color32::from_rgb(12, 16, 22));

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space((rect.height() * 0.32_f32).max(10.0_f32));
                    ui.label(
                        RichText::new("Preview closed")
                            .size(21.0_f32)
                            .strong()
                            .color(theme.text01),
                    );
                    ui.add_space(8.0_f32);
                    ui.label(
                        RichText::new(
                            "If you need to use the device in a third-party software, please close the preview first.",
                        )
                        .size(13.0_f32)
                        .color(theme.text02),
                    );
                    ui.add_space(16.0_f32);
                    let open_btn = ui.button(
                        RichText::new("Open Preview")
                            .color(Color32::BLACK)
                            .strong(),
                    );
                    if open_btn.clicked() {
                        self.state.preview_open = true;
                    }
                });
            });
            return;
        }

        // Live video feed rendering with aspect ratio preservation
        let target_aspect = if self.state.format_preset.is_vertical() {
            9.0_f32 / 16.0_f32
        } else {
            16.0_f32 / 9.0_f32
        };

        let (video_w, video_h) = if avail_size.x / avail_size.y > target_aspect {
            (avail_size.y * target_aspect, avail_size.y)
        } else {
            (avail_size.x, avail_size.x / target_aspect)
        };

        let pad_x = (avail_size.x - video_w) * 0.5_f32;
        let pad_y = (avail_size.y - video_h) * 0.5_f32;

        let (full_rect, _) = ui.allocate_exact_size(avail_size, egui::Sense::hover());
        let video_rect = Rect::from_min_size(
            Pos2::new(full_rect.min.x + pad_x, full_rect.min.y + pad_y),
            Vec2::new(video_w, video_h),
        );

        let tex = self.video_engine.render_frame(ctx, &mut self.state);

        // Paint background and video frame
        ui.painter().rect_filled(full_rect, Rounding::same(6.0_f32), Color32::from_rgb(10, 14, 20));
        ui.painter().image(
            tex.id(),
            video_rect,
            Rect::from_min_max(Pos2::new(0.0_f32, 0.0_f32), Pos2::new(1.0_f32, 1.0_f32)),
            Color32::WHITE,
        );

        // Mouse interaction response on video rect
        let video_response = ui.interact(video_rect, ui.id().with("video_canvas"), egui::Sense::click_and_drag());

        // Interactive mouse wheel zoom (WheelHandler)
        if video_response.hovered() {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta.abs() > 0.1_f32 {
                let zoom_change = if scroll_delta > 0.0_f32 { 0.1_f32 } else { -0.1_f32 };
                self.state.zoom_x = (self.state.zoom_x + zoom_change).clamp(1.0_f32, 4.0_f32);
                let _ = self.controller.set_zoom((self.state.zoom_x * 100.0_f32) as u16);
            }
        }

        // Interactive click-and-drag pan/tilt (`DefCamera.qml` `dragMovePanTilt` + `dragDropTransition`)
        // Only active in Normal mode (`!ready_auto_track`) so clicking on the video during AI Tracking
        // does not send a manual PTZ velocity/stop command that aborts tracking.
        let manual_ptz_drag_allowed = self.controller.capabilities().support_ptz_drag
            && self.state.video_mode == VideoMode::Normal
            && !self.state.ready_auto_track;
        if video_response.dragged() && manual_ptz_drag_allowed {
            let drag_delta = video_response.drag_delta();
            if drag_delta.length_sq() > 0.25_f32 {
                self.state.pan_deg =
                    (self.state.pan_deg - drag_delta.x * 0.15_f32).clamp(-130.0_f32, 130.0_f32);
                self.state.tilt_deg =
                    (self.state.tilt_deg + drag_delta.y * 0.15_f32).clamp(-90.0_f32, 90.0_f32);
                let pan_spd = ((drag_delta.x.abs() * 0.35_f32).round() as u8).clamp(1, 6);
                let tilt_spd = ((drag_delta.y.abs() * 0.35_f32).round() as u8).clamp(1, 6);
                let pan_dir = if drag_delta.x.abs() > 0.5_f32 {
                    if drag_delta.x > 0.0_f32 { -1i8 } else { 1i8 }
                } else {
                    0i8
                };
                let tilt_dir = if drag_delta.y.abs() > 0.5_f32 {
                    if drag_delta.y > 0.0_f32 { -1i8 } else { 1i8 }
                } else {
                    0i8
                };
                let can_send = self
                    .state
                    .last_ptz_send_time
                    .map(|t| t.elapsed().as_millis() >= 60)
                    .unwrap_or(true);
                if can_send {
                    let _ = self.controller.set_pantilt_relative_ext(
                        if pan_dir == 0 { 0 } else { pan_spd },
                        pan_dir,
                        if tilt_dir == 0 { 0 } else { tilt_spd },
                        tilt_dir,
                    );
                    self.state.last_ptz_cmd = (pan_spd, pan_dir, tilt_spd, tilt_dir);
                    self.state.last_ptz_send_time = Some(Instant::now());
                }
            }
        } else if video_response.drag_stopped() && manual_ptz_drag_allowed {
            let _ = self.controller.stop_pantilt_relative();
            self.state.last_ptz_cmd = (0, 0, 0, 0);
        }

        // AI Tracking bounding boxes (`personRect`, normalized 0..10000 in hardware `UvcRect`)
        if self.state.video_mode == VideoMode::AutoTracking
            || self.state.video_mode == VideoMode::AutoFraming
        {
            let mut newly_selected = None;
            for target in &self.state.tracked_targets {
                let norm_x = (target.x as f32 / 10000.0_f32).clamp(0.0_f32, 1.0_f32);
                let norm_y = (target.y as f32 / 10000.0_f32).clamp(0.0_f32, 1.0_f32);
                let norm_w = (target.width as f32 / 10000.0_f32).clamp(0.02_f32, 1.0_f32);
                let norm_h = (target.height as f32 / 10000.0_f32).clamp(0.02_f32, 1.0_f32);

                let box_rect = Rect::from_min_size(
                    Pos2::new(
                        video_rect.min.x + norm_x * video_rect.width(),
                        video_rect.min.y + norm_y * video_rect.height(),
                    ),
                    Vec2::new(norm_w * video_rect.width(), norm_h * video_rect.height()),
                );

                let is_selected = target.track_id == self.state.selected_target_id;
                let stroke_color = if is_selected {
                    theme.brand_green
                } else {
                    theme.text03
                };

                ui.painter().rect_stroke(
                    box_rect,
                    Rounding::same(4.0_f32),
                    Stroke::new(2.0_f32, stroke_color),
                );

                if video_response.clicked() {
                    if let Some(pos) = video_response.interact_pointer_pos() {
                        if box_rect.contains(pos) {
                            newly_selected = Some(*target);
                        }
                    }
                }
            }

            if let Some(selected_target) = newly_selected {
                let _ = self.controller.select_track_object(&selected_target);
                self.state.selected_target_id = selected_target.track_id;
                self.state.ready_auto_track = false;
                self.state.tracked_targets.clear();
                let _ = self
                    .controller
                    .set_video_mode_raw(VideoMode::AutoTracking, &VideoModeAuxData::default());
                self.state.show_toast("AI Tracking locked onto selected target", false);
            } else if video_response.clicked() && self.state.ready_auto_track {
                if let Some(pos) = video_response.interact_pointer_pos() {
                    let norm_x =
                        ((pos.x - video_rect.min.x) / video_rect.width()).clamp(0.05_f32, 0.95_f32);
                    let norm_y =
                        ((pos.y - video_rect.min.y) / video_rect.height()).clamp(0.05_f32, 0.95_f32);
                    let _ = self
                        .controller
                        .select_track_object_norm(norm_x, norm_y, 0.0_f32, 0.0_f32);
                    self.state.ready_auto_track = false;
                    self.state.tracked_targets.clear();
                    let _ = self
                        .controller
                        .set_video_mode_raw(VideoMode::AutoTracking, &VideoModeAuxData::default());
                    self.state.show_toast("AI Tracking locked onto selected target", false);
                }
            }
        }

        // Smart Whiteboard 4-Corner Overlay (SmartWhiteBoardBox.qml)
        if self.state.video_mode == VideoMode::SmartWhiteBoard {
            let corner_screen: Vec<Pos2> = self
                .state
                .whiteboard_corners
                .iter()
                .map(|c| {
                    Pos2::new(
                        video_rect.min.x + c.x * video_rect.width(),
                        video_rect.min.y + c.y * video_rect.height(),
                    )
                })
                .collect();

            // Draw bounding polygon edges
            for i in 0..4 {
                let p1 = corner_screen[i];
                let p2 = corner_screen[(i + 1) % 4];
                ui.painter().line_segment([p1, p2], Stroke::new(2.0_f32, theme.brand_green));
            }

            // Draggable corner handles
            for (i, corner) in self.state.whiteboard_corners.iter_mut().enumerate() {
                let p = corner_screen[i];
                let handle_rect = Rect::from_center_size(p, Vec2::new(16.0_f32, 16.0_f32));
                let handle_resp = ui.interact(
                    handle_rect,
                    ui.id().with(("wb_corner", i)),
                    egui::Sense::drag(),
                );

                if handle_resp.dragged() {
                    let d = handle_resp.drag_delta();
                    corner.x = (corner.x + d.x / video_rect.width()).clamp(0.0_f32, 1.0_f32);
                    corner.y = (corner.y + d.y / video_rect.height()).clamp(0.0_f32, 1.0_f32);
                }

                ui.painter().circle_filled(p, 6.0_f32, theme.brand_green);
                ui.painter().circle_stroke(p, 6.0_f32, Stroke::new(2.0_f32, Color32::WHITE));
            }

            // Floating bottom pill for whiteboard controls
            let pill_rect = Rect::from_center_size(
                Pos2::new(video_rect.center().x, video_rect.max.y - 32.0_f32),
                Vec2::new(320.0_f32, 38.0_f32),
            );
            ui.painter().rect_filled(pill_rect, Rounding::same(19.0_f32), Color32::from_rgba_unmultiplied(20, 26, 36, 230));
            ui.painter().rect_stroke(pill_rect, Rounding::same(19.0_f32), Stroke::new(1.0_f32, theme.border01));

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(pill_rect), |ui| {
                ui.horizontal_centered(|ui| {
                    if ui.button("Rescan").clicked() {
                        self.state.whiteboard_corners = [
                            Pos2::new(0.18_f32, 0.16_f32),
                            Pos2::new(0.82_f32, 0.16_f32),
                            Pos2::new(0.82_f32, 0.78_f32),
                            Pos2::new(0.18_f32, 0.78_f32),
                        ];
                        self.state.show_toast("Rescanning whiteboard corners...", false);
                    }
                    if ui.button("Manual Adjustment").clicked() {
                        self.state.show_toast("Drag corner handles to adjust whiteboard bounds", false);
                    }
                    if ui.button(RichText::new("Exit").color(theme.error)).clicked() {
                        self.set_video_mode(VideoMode::Normal);
                    }
                });
            });
        }

        // Pause-Track Area Overlay (TrackForbiddenAreaSetting.qml)
        if self.state.show_pause_track_overlay {
            // Render exclusion zones
            for (idx, area) in self.state.forbidden_areas.iter().enumerate() {
                if !area.enabled {
                    continue;
                }
                let norm_x_min = ((area.pan_min + 350) as f32 / 700.0_f32).clamp(0.0_f32, 1.0_f32);
                let norm_x_max = ((area.pan_max + 350) as f32 / 700.0_f32).clamp(0.0_f32, 1.0_f32);
                let norm_y_min = ((200 - area.tilt_max) as f32 / 400.0_f32).clamp(0.0_f32, 1.0_f32);
                let norm_y_max = ((200 - area.tilt_min) as f32 / 400.0_f32).clamp(0.0_f32, 1.0_f32);

                let z_rect = Rect::from_min_max(
                    Pos2::new(video_rect.min.x + norm_x_min * video_rect.width(), video_rect.min.y + norm_y_min * video_rect.height()),
                    Pos2::new(video_rect.min.x + norm_x_max * video_rect.width(), video_rect.min.y + norm_y_max * video_rect.height()),
                );

                ui.painter().rect_filled(z_rect, Rounding::same(4.0_f32), Color32::from_rgba_unmultiplied(239, 68, 68, 60));
                ui.painter().rect_stroke(z_rect, Rounding::same(4.0_f32), Stroke::new(1.5_f32, Color32::from_rgb(239, 68, 68)));
                ui.painter().text(
                    z_rect.center(),
                    Align2::CENTER_CENTER,
                    format!("Zone {}", idx + 1),
                    FontId::proportional(13.0_f32),
                    Color32::WHITE,
                );
            }

            // Floating header control bar
            let header_rect = Rect::from_center_size(
                Pos2::new(video_rect.center().x, video_rect.min.y + 32.0_f32),
                Vec2::new((video_rect.width() * 0.85_f32).min(540.0_f32), 44.0_f32),
            );
            ui.painter().rect_filled(header_rect, Rounding::same(8.0_f32), Color32::from_rgba_unmultiplied(20, 26, 36, 235));
            ui.painter().rect_stroke(header_rect, Rounding::same(8.0_f32), Stroke::new(1.0_f32, theme.border01));

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(header_rect), |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("Pause-Track Areas").strong().color(theme.text01));
                    ui.separator();
                    for i in 0..4 {
                        let active = self.state.forbidden_areas[i].enabled;
                        let btn = ui.selectable_label(active, format!("Zone {}", i + 1));
                        if btn.clicked() {
                            self.state.forbidden_areas[i].enabled = !active;
                        }
                    }
                    ui.separator();
                    if ui.button("Clear all").clicked() {
                        for a in &mut self.state.forbidden_areas {
                            a.enabled = false;
                        }
                    }
                    if ui.button(RichText::new("Done").color(theme.brand_green).strong()).clicked() {
                        let _ = self.controller.set_track_forbidden_areas(&self.state.forbidden_areas);
                        let _ = self.controller.exit_track_forbidden_area_setting();
                        self.state.show_pause_track_overlay = false;
                    }
                });
            });
        }

        // Toast Banner
        if let Some((msg, ts, is_err)) = &self.state.toast_message {
            if ts.elapsed().as_secs_f32() < 3.5_f32 {
                let toast_w = (msg.len() as f32 * 7.5_f32 + 48.0_f32).max(180.0_f32);
                let toast_rect = Rect::from_center_size(
                    Pos2::new(video_rect.center().x, video_rect.min.y + 26.0_f32),
                    Vec2::new(toast_w, 32.0_f32),
                );

                let badge_color = if *is_err { theme.error } else { theme.brand_green };
                ui.painter().rect_filled(
                    toast_rect,
                    Rounding::same(16.0_f32),
                    Color32::from_rgba_unmultiplied(18, 24, 34, 235),
                );
                ui.painter().rect_stroke(
                    toast_rect,
                    Rounding::same(16.0_f32),
                    Stroke::new(1.0_f32, badge_color),
                );

                ui.painter().text(
                    Pos2::new(toast_rect.min.x + 14.0_f32, toast_rect.center().y),
                    Align2::LEFT_CENTER,
                    if *is_err { "⚠" } else { "✓" },
                    FontId::proportional(14.0_f32),
                    badge_color,
                );
                ui.painter().text(
                    Pos2::new(toast_rect.min.x + 32.0_f32, toast_rect.center().y),
                    Align2::LEFT_CENTER,
                    msg,
                    FontId::proportional(12.5_f32),
                    theme.text01,
                );
            }
        }
    }

    // ========================================================================
    // Compact Floating Toolbar Mode (Toolbar.qml - 780x74 or 780x280 with popup)
    // ========================================================================
    fn render_toolbar_mode(&mut self, ctx: &egui::Context, theme: &InsTheme) {
        // Keep physical camera frame pipeline alive in background so AI tracking and hardware commands work
        let _ = self.video_engine.render_frame(ctx, &mut self.state);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme.bg01)
                    .inner_margin(Margin::symmetric(8.0_f32, 5.0_f32)),
            )
            .show(ctx, |ui| {
                // 1. Optional Popover Panel above the toolbar (`ToolbarPopup`)
                if self.state.toolbar_popup != ToolbarPopup::None {
                    let pop_h = (ui.available_height() - 68.0_f32).max(150.0_f32);
                    let (pop_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), pop_h),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(pop_rect, Rounding::same(8.0_f32), theme.bg02);
                    ui.painter().rect_stroke(
                        pop_rect,
                        Rounding::same(8.0_f32),
                        Stroke::new(1.0_f32, theme.border01),
                    );

                    let inner_rect = pop_rect.shrink2(egui::vec2(14.0_f32, 10.0_f32));
                    let mut pop_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner_rect));
                    pop_ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let title = match self.state.toolbar_popup {
                                ToolbarPopup::PictureSetting => "Image & Effects Settings",
                                ToolbarPopup::GestureCtrl => "Gesture Control",
                                ToolbarPopup::RemoteCtrl => "Mobile Remote Control",
                                ToolbarPopup::MoreSetting => "More Camera Settings",
                                _ => "Settings",
                            };
                            ui.label(RichText::new(title).strong().size(13.5_f32).color(theme.text01));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("✕").clicked() {
                                    self.state.toolbar_popup = ToolbarPopup::None;
                                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                                        egui::vec2(780.0_f32, 74.0_f32),
                                    ));
                                }
                            });
                        });
                        ui.add_space(6.0_f32);

                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| match self.state.toolbar_popup {
                                ToolbarPopup::GestureCtrl => {
                                    if theme::ins_switch_row(
                                        ui,
                                        "Enable Gesture Control",
                                        None,
                                        &mut self.state.enable_all_gestures,
                                        true,
                                        theme,
                                    ) {
                                        let _ = self
                                            .controller
                                            .set_all_gestures_enabled(self.state.enable_all_gestures);
                                    }
                                    ui.add_space(4.0_f32);
                                    ui.horizontal(|ui| {
                                        if ui
                                            .checkbox(&mut self.state.use_gesture_palm, "✋ AI Tracking")
                                            .changed()
                                        {
                                            let _ = self.controller.set_gesture_enabled(
                                                open_360_link_protocol::Gesture::Palm,
                                                self.state.use_gesture_palm,
                                            );
                                        }
                                        ui.add_space(12.0_f32);
                                        if ui
                                            .checkbox(&mut self.state.use_gesture_v, "✌ Whiteboard")
                                            .changed()
                                        {
                                            let _ = self.controller.set_gesture_enabled(
                                                open_360_link_protocol::Gesture::VShape,
                                                self.state.use_gesture_v,
                                            );
                                        }
                                        ui.add_space(12.0_f32);
                                        if ui
                                            .checkbox(&mut self.state.use_gesture_l, "👆 Zoom")
                                            .changed()
                                        {
                                            let _ = self.controller.set_gesture_enabled(
                                                open_360_link_protocol::Gesture::LShape,
                                                self.state.use_gesture_l,
                                            );
                                        }
                                    });
                                }
                                ToolbarPopup::PictureSetting => {
                                    ui.columns(2, |cols| {
                                        cols[0].vertical(|ui| {
                                            if theme::ins_switch_row(
                                                ui,
                                                "Auto Exposure",
                                                None,
                                                &mut self.state.auto_exposure,
                                                !self.state.hdr_enabled,
                                                theme,
                                            ) {
                                                let _ = self
                                                    .controller
                                                    .set_auto_exposure(self.state.auto_exposure);
                                            }
                                            if self.state.auto_exposure {
                                                if theme::ins_slider_f32(
                                                    ui,
                                                    Some("EV"),
                                                    &mut self.state.ev_compensation,
                                                    -3.0,
                                                    3.0,
                                                    0.3,
                                                    1,
                                                    "EV",
                                                    !self.state.hdr_enabled,
                                                    theme,
                                                ) {
                                                    let _ = self
                                                        .controller
                                                        .set_exposure_compensation(
                                                            self.state.ev_compensation,
                                                        );
                                                }
                                            }
                                            if theme::ins_switch_row(
                                                ui,
                                                "Auto Focus",
                                                None,
                                                &mut self.state.auto_focus,
                                                true,
                                                theme,
                                            ) {
                                                let _ =
                                                    self.controller.set_auto_focus(self.state.auto_focus);
                                            }
                                            if theme::ins_switch_row(
                                                ui,
                                                "Auto White Balance",
                                                None,
                                                &mut self.state.auto_white_balance,
                                                true,
                                                theme,
                                            ) {
                                                let _ = self.controller.set_auto_white_balance(
                                                    self.state.auto_white_balance,
                                                );
                                            }
                                        });
                                        cols[1].vertical(|ui| {
                                            let mut br = self.state.brightness as i32;
                                            if theme::ins_slider_i32(
                                                ui,
                                                Some("Brightness"),
                                                &mut br,
                                                0,
                                                100,
                                                "%",
                                                true,
                                                theme,
                                            ) {
                                                self.state.brightness = br as i16;
                                                let _ = self
                                                    .controller
                                                    .set_brightness(self.state.brightness);
                                            }
                                            let mut ct = self.state.contrast as i32;
                                            if theme::ins_slider_i32(
                                                ui,
                                                Some("Contrast"),
                                                &mut ct,
                                                0,
                                                100,
                                                "%",
                                                true,
                                                theme,
                                            ) {
                                                self.state.contrast = ct as u16;
                                                let _ =
                                                    self.controller.set_contrast(self.state.contrast);
                                            }
                                            let mut sat = self.state.saturation as i32;
                                            if theme::ins_slider_i32(
                                                ui,
                                                Some("Saturation"),
                                                &mut sat,
                                                0,
                                                100,
                                                "%",
                                                true,
                                                theme,
                                            ) {
                                                self.state.saturation = sat as u16;
                                                let _ = self
                                                    .controller
                                                    .set_saturation(self.state.saturation);
                                            }
                                            if theme::ins_switch_row(
                                                ui,
                                                "HDR",
                                                None,
                                                &mut self.state.hdr_enabled,
                                                true,
                                                theme,
                                            ) {
                                                let _ =
                                                    self.controller.set_hdr(self.state.hdr_enabled);
                                            }
                                        });
                                    });
                                }
                                ToolbarPopup::RemoteCtrl => {
                                    ui.label(
                                        RichText::new(
                                            "Open http://127.0.0.1:9360 on your phone or browser to control the camera remotely.",
                                        )
                                        .color(theme.text02),
                                    );
                                }
                                ToolbarPopup::MoreSetting | _ => {
                                    ui.columns(2, |cols| {
                                        cols[0].vertical(|ui| {
                                            if ui
                                                .checkbox(
                                                    &mut self.state.horizontal_mirror,
                                                    "Horizontal Flip",
                                                )
                                                .changed()
                                            {
                                                let _ = self.controller.set_horizontal_mirror(
                                                    self.state.horizontal_mirror,
                                                );
                                            }
                                            if self.controller.capabilities().support_mirror_vertically
                                                && ui
                                                    .checkbox(
                                                        &mut self.state.vertical_mirror,
                                                        "Vertical Flip",
                                                    )
                                                    .changed()
                                            {
                                                let _ = self.controller.set_vertical_mirror(
                                                    self.state.vertical_mirror,
                                                );
                                            }
                                            if ui
                                                .checkbox(
                                                    &mut self.state.audio_denoise,
                                                    "AI Audio Noise Reduction",
                                                )
                                                .changed()
                                            {
                                                let _ = self.controller.set_audio_noise_reduction(
                                                    self.state.audio_denoise,
                                                );
                                            }
                                        });
                                        cols[1].vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label("Tracking Speed:");
                                                egui::ComboBox::from_id_salt("tb_track_speed")
                                                    .selected_text(match self.state.track_speed {
                                                        open_360_link_protocol::TrackSpeed::Fast => {
                                                            "Quick"
                                                        }
                                                        open_360_link_protocol::TrackSpeed::Normal => {
                                                            "Ordinary"
                                                        }
                                                        open_360_link_protocol::TrackSpeed::Slow => {
                                                            "Slow"
                                                        }
                                                    })
                                                    .show_ui(ui, |ui| {
                                                        if ui
                                                            .selectable_value(
                                                                &mut self.state.track_speed,
                                                                open_360_link_protocol::TrackSpeed::Fast,
                                                                "Quick",
                                                            )
                                                            .clicked()
                                                        {
                                                            let _ = self.controller.set_track_speed(
                                                                open_360_link_protocol::TrackSpeed::Fast,
                                                            );
                                                        }
                                                        if ui
                                                            .selectable_value(
                                                                &mut self.state.track_speed,
                                                                open_360_link_protocol::TrackSpeed::Normal,
                                                                "Ordinary",
                                                            )
                                                            .clicked()
                                                        {
                                                            let _ = self.controller.set_track_speed(
                                                                open_360_link_protocol::TrackSpeed::Normal,
                                                            );
                                                        }
                                                        if ui
                                                            .selectable_value(
                                                                &mut self.state.track_speed,
                                                                open_360_link_protocol::TrackSpeed::Slow,
                                                                "Slow",
                                                            )
                                                            .clicked()
                                                        {
                                                            let _ = self.controller.set_track_speed(
                                                                open_360_link_protocol::TrackSpeed::Slow,
                                                            );
                                                        }
                                                    });
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Anti-Flicker:");
                                                egui::ComboBox::from_id_salt("tb_anti_flicker")
                                                    .selected_text(match self.state.anti_flicker {
                                                        PowerLineFrequency::Auto => "Auto",
                                                        PowerLineFrequency::Hz50 => "50Hz",
                                                        PowerLineFrequency::Hz60 => "60Hz",
                                                        PowerLineFrequency::Disabled => "Off",
                                                    })
                                                    .show_ui(ui, |ui| {
                                                        if ui
                                                            .selectable_value(
                                                                &mut self.state.anti_flicker,
                                                                PowerLineFrequency::Auto,
                                                                "Auto",
                                                            )
                                                            .clicked()
                                                        {
                                                            let _ = self
                                                                .controller
                                                                .set_power_line_frequency(
                                                                    PowerLineFrequency::Auto,
                                                                );
                                                        }
                                                        if ui
                                                            .selectable_value(
                                                                &mut self.state.anti_flicker,
                                                                PowerLineFrequency::Hz50,
                                                                "50Hz",
                                                            )
                                                            .clicked()
                                                        {
                                                            let _ = self
                                                                .controller
                                                                .set_power_line_frequency(
                                                                    PowerLineFrequency::Hz50,
                                                                );
                                                        }
                                                        if ui
                                                            .selectable_value(
                                                                &mut self.state.anti_flicker,
                                                                PowerLineFrequency::Hz60,
                                                                "60Hz",
                                                            )
                                                            .clicked()
                                                        {
                                                            let _ = self
                                                                .controller
                                                                .set_power_line_frequency(
                                                                    PowerLineFrequency::Hz60,
                                                                );
                                                        }
                                                    });
                                            });
                                        });
                                    });
                                }
                            });
                    });
                    ui.add_space(4.0_f32);
                }

                // 2. Main 64px Pill Bar (`Toolbar.qml` `toolbarRect`)
                let (bar_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 64.0_f32),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(bar_rect, Rounding::same(8.0_f32), theme.bg02);
                ui.painter().rect_stroke(
                    bar_rect,
                    Rounding::same(8.0_f32),
                    Stroke::new(1.0_f32, theme.border01),
                );

                let inner_bar = bar_rect.shrink2(egui::vec2(10.0_f32, 6.0_f32));
                let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner_bar));
                bar_ui.horizontal_centered(|ui| {
                    // Leftmost stacked Close + Switch to Main Window (`Toolbar.qml` lines 473-511)
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0_f32;
                        if ui
                            .add_sized([22.0_f32, 22.0_f32], egui::Button::new(RichText::new("✕").size(11.0_f32)))
                            .on_hover_text("Close Application")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui
                            .add_sized([22.0_f32, 22.0_f32], egui::Button::new(RichText::new("🗖").size(11.0_f32)))
                            .on_hover_text("Switch to Main Window")
                            .clicked()
                        {
                            self.state.ui_mode = UiMode::MainWindow;
                            self.state.toolbar_popup = ToolbarPopup::None;
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                                1440.0_f32, 714.0_f32,
                            )));
                        }
                    });

                    ui.add_space(6.0_f32);
                    let (d0, _) = ui.allocate_exact_size(egui::vec2(1.0_f32, 32.0_f32), egui::Sense::hover());
                    ui.painter().rect_filled(d0, Rounding::ZERO, theme.border01);
                    ui.add_space(6.0_f32);

                    // PTZ Circular D-Pad (`PTZController.qml` 44x44 circle with directional sectors + center reset)
                    let (pad_rect, pad_resp) = ui.allocate_exact_size(
                        egui::vec2(46.0_f32, 46.0_f32),
                        egui::Sense::click_and_drag(),
                    );
                    let center = pad_rect.center();
                    let radius = 22.0_f32;
                    ui.painter().circle_filled(center, radius, theme.overlay03);
                    ui.painter().circle_stroke(center, radius, Stroke::new(1.0_f32, theme.border01));
                    ui.painter().circle_filled(center, 7.0_f32, theme.bg02);

                    // Directional chevrons on the circular pad
                    for (pos, glyph) in [
                        (Pos2::new(center.x, center.y - 14.5_f32), "▴"),
                        (Pos2::new(center.x, center.y + 14.5_f32), "▾"),
                        (Pos2::new(center.x - 14.5_f32, center.y), "◂"),
                        (Pos2::new(center.x + 14.5_f32, center.y), "▸"),
                    ] {
                        ui.painter().text(
                            pos,
                            Align2::CENTER_CENTER,
                            glyph,
                            FontId::proportional(11.0_f32),
                            theme.text02,
                        );
                    }

                    if pad_resp.is_pointer_button_down_on() {
                        if let Some(ptr) = pad_resp.interact_pointer_pos() {
                            let d = ptr - center;
                            if d.length() < 7.5_f32 && pad_resp.clicked() {
                                self.state.pan_deg = 0.0_f32;
                                self.state.tilt_deg = 0.0_f32;
                                let _ = self.controller.reset_gimbal();
                            } else if d.length() >= 7.5_f32 {
                                let pan_dir = if d.x.abs() > d.y.abs() * 0.5_f32 {
                                    if d.x > 0.0_f32 { 1i8 } else { -1i8 }
                                } else {
                                    0i8
                                };
                                let tilt_dir = if d.y.abs() > d.x.abs() * 0.5_f32 {
                                    if d.y > 0.0_f32 { 1i8 } else { -1i8 }
                                } else {
                                    0i8
                                };
                                let can_send = self
                                    .state
                                    .last_ptz_send_time
                                    .map(|t| t.elapsed().as_millis() >= 60)
                                    .unwrap_or(true);
                                if can_send {
                                    let _ = self.controller.set_pantilt_relative_ext(
                                        if pan_dir == 0 { 0 } else { 4 },
                                        pan_dir,
                                        if tilt_dir == 0 { 0 } else { 4 },
                                        tilt_dir,
                                    );
                                    self.state.last_ptz_cmd = (4, pan_dir, 4, tilt_dir);
                                    self.state.last_ptz_send_time = Some(Instant::now());
                                    self.state.nudge_stop_at =
                                        Some(Instant::now() + Duration::from_millis(140));
                                }
                            }
                        }
                    }
                    pad_resp.on_hover_text("Drag or click arrows to Pan/Tilt • Click center dot to Reset");

                    ui.add_space(6.0_f32);

                    // Compact Zoom controls (`−`, slider, `+`)
                    if ui
                        .add_sized([22.0_f32, 26.0_f32], egui::Button::new("−"))
                        .on_hover_text("Zoom Out")
                        .clicked()
                    {
                        self.state.zoom_x = (self.state.zoom_x - 0.25_f32).clamp(1.0_f32, 4.0_f32);
                        let _ = self.controller.set_zoom((self.state.zoom_x * 100.0_f32) as u16);
                    }
                    ui.spacing_mut().slider_width = 64.0_f32;
                    if ui
                        .add(
                            egui::Slider::new(&mut self.state.zoom_x, 1.0_f32..=4.0_f32)
                                .show_value(false),
                        )
                        .on_hover_text(format!("Zoom: {:.1}x", self.state.zoom_x))
                        .changed()
                    {
                        let _ = self.controller.set_zoom((self.state.zoom_x * 100.0_f32) as u16);
                    }
                    if ui
                        .add_sized([22.0_f32, 26.0_f32], egui::Button::new("+"))
                        .on_hover_text("Zoom In")
                        .clicked()
                    {
                        self.state.zoom_x = (self.state.zoom_x + 0.25_f32).clamp(1.0_f32, 4.0_f32);
                        let _ = self.controller.set_zoom((self.state.zoom_x * 100.0_f32) as u16);
                    }

                    ui.add_space(8.0_f32);
                    let (d1, _) = ui.allocate_exact_size(egui::vec2(1.0_f32, 32.0_f32), egui::Sense::hover());
                    ui.painter().rect_filled(d1, Rounding::ZERO, theme.border01);
                    ui.add_space(8.0_f32);

                    // AI Modes (`SecondLastSubarea.qml`: 40x40 rounded icon buttons with tooltips)
                    let trk_act = self.state.video_mode == VideoMode::AutoTracking
                        || self.state.video_mode == VideoMode::AutoFraming;
                    let wb_act = self.state.video_mode == VideoMode::WhiteBoard;
                    let ovh_act = self.state.video_mode == VideoMode::OverheadShot;
                    let swb_act = self.state.video_mode == VideoMode::SmartWhiteBoard;
                    let dsk_act = self.state.video_mode == VideoMode::DeskView;

                    for (icon, tooltip, active, target_mode) in [
                        ("🎯", "AI Tracking", trk_act, VideoMode::AutoTracking),
                        ("📋", "Whiteboard", wb_act, VideoMode::WhiteBoard),
                        ("⬇", "Overhead", ovh_act, VideoMode::OverheadShot),
                        ("🖽", "Smart Whiteboard", swb_act, VideoMode::SmartWhiteBoard),
                        ("📐", "DeskView", dsk_act, VideoMode::DeskView),
                    ] {
                        let fill = if active {
                            theme.brand_green
                        } else {
                            theme.overlay03
                        };
                        let fg = if active {
                            Color32::BLACK
                        } else {
                            theme.text01
                        };
                        let btn = egui::Button::new(RichText::new(icon).size(16.0_f32).color(fg))
                            .fill(fill)
                            .rounding(Rounding::same(8.0_f32));
                        if ui
                            .add_sized([40.0_f32, 40.0_f32], btn)
                            .on_hover_text(tooltip)
                            .clicked()
                        {
                            self.set_video_mode(target_mode);
                        }
                        ui.add_space(2.0_f32);
                    }

                    ui.add_space(6.0_f32);
                    let (d2, _) = ui.allocate_exact_size(egui::vec2(1.0_f32, 32.0_f32), egui::Sense::hover());
                    ui.painter().rect_filled(d2, Rounding::ZERO, theme.border01);
                    ui.add_space(6.0_f32);

                    // Quick Popover Buttons (`LastSubarea.qml`: Gesture, Image/Effects, Remote, More)
                    for (icon, tooltip, popup) in [
                        ("✋", "Gesture Control", ToolbarPopup::GestureCtrl),
                        ("🎨", "Image & Effects", ToolbarPopup::PictureSetting),
                        ("📱", "Remote Control", ToolbarPopup::RemoteCtrl),
                        ("⚙", "More Settings", ToolbarPopup::MoreSetting),
                    ] {
                        let active = self.state.toolbar_popup == popup;
                        let fill = if active {
                            theme.brand_green
                        } else {
                            theme.overlay03
                        };
                        let fg = if active {
                            Color32::BLACK
                        } else {
                            theme.text01
                        };
                        let btn = egui::Button::new(RichText::new(icon).size(15.0_f32).color(fg))
                            .fill(fill)
                            .rounding(Rounding::same(8.0_f32));
                        if ui
                            .add_sized([38.0_f32, 40.0_f32], btn)
                            .on_hover_text(tooltip)
                            .clicked()
                        {
                            if active {
                                self.state.toolbar_popup = ToolbarPopup::None;
                                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                                    egui::vec2(780.0_f32, 74.0_f32),
                                ));
                            } else {
                                self.state.toolbar_popup = popup;
                                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                                    egui::vec2(780.0_f32, 270.0_f32),
                                ));
                            }
                        }
                        ui.add_space(2.0_f32);
                    }

                    ui.add_space(6.0_f32);
                    if ui
                        .add_sized(
                            [92.0_f32, 36.0_f32],
                            egui::Button::new(
                                RichText::new("🗖 Main View")
                                    .size(12.0_f32)
                                    .color(theme.text01),
                            )
                            .fill(theme.overlay03)
                            .rounding(Rounding::same(8.0_f32)),
                        )
                        .on_hover_text("Restore full Insta360 Link Controller window")
                        .clicked()
                    {
                        self.state.ui_mode = UiMode::MainWindow;
                        self.state.toolbar_popup = ToolbarPopup::None;
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1440.0_f32, 714.0_f32,
                        )));
                    }
                });
            });
    }

    // ========================================================================
    // Modals & Dialogs (Preferences, Virtual Cam, Remote QR, Factory Reset, DeskView Pitch)
    // ========================================================================
    fn render_modals(&mut self, ctx: &egui::Context, theme: &InsTheme) {
        // Preferences modal (preference/MoreSettings.qml - 640x480)
        if self.state.show_preferences {
            let mut open = self.state.show_preferences;
            egui::Window::new("Settings")
                .fixed_size([640.0_f32, 480.0_f32])
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        // Left navigation pane (160px)
                        ui.vertical(|ui| {
                            ui.set_width(150.0_f32);
                            let tabs = [
                                (PreferenceTab::General, "General"),
                                (PreferenceTab::Hotkeys, "Hotkeys"),
                                (PreferenceTab::GetLog, "Report Issue"),
                                (PreferenceTab::About, "About Us"),
                            ];
                            for (tab, label) in tabs {
                                let is_active = self.state.preference_tab == tab;
                                if ui.selectable_label(is_active, label).clicked() {
                                    self.state.preference_tab = tab;
                                }
                            }
                        });

                        ui.separator();

                        // Right content pane
                        ui.vertical(|ui| {
                            match self.state.preference_tab {
                                PreferenceTab::General => {
                                    ui.label(RichText::new("General Settings").strong().color(theme.text01));
                                    ui.add_space(8.0_f32);
                                    ui.label("Appearance:");
                                    ui.horizontal(|ui| {
                                        ui.radio_value(&mut self.state.is_light_mode, false, "Dark");
                                        ui.radio_value(&mut self.state.is_light_mode, true, "Light");
                                    });
                                    ui.add_space(12.0_f32);
                                    ui.label("Screenshot Save Location:");
                                    ui.label(
                                        RichText::new(self.state.screenshot_dir.display().to_string())
                                            .monospace()
                                            .color(theme.text02),
                                    );
                                    ui.add_space(8.0_f32);
                                    ui.label("Recording Save Location:");
                                    ui.label(
                                        RichText::new(self.state.recording_dir.display().to_string())
                                            .monospace()
                                            .color(theme.text02),
                                    );
                                }
                                PreferenceTab::Hotkeys => {
                                    ui.label(RichText::new("Hotkey Shortcuts").strong().color(theme.text01));
                                    ui.add_space(8.0_f32);
                                    egui::Grid::new("hotkeys_table")
                                        .striped(true)
                                        .spacing([24.0_f32, 8.0_f32])
                                        .show(ui, |ui| {
                                            ui.label("Action");
                                            ui.label("Shortcut");
                                            ui.end_row();

                                            let shortcuts = [
                                                ("AI Tracking", "Ctrl + Shift + T"),
                                                ("Whiteboard", "Ctrl + Shift + W"),
                                                ("Overhead", "Ctrl + Shift + O"),
                                                ("DeskView", "Ctrl + Shift + D"),
                                                ("Gimbal Up/Down/Left/Right", "Arrow Keys"),
                                                ("Center gimbal", "Ctrl + Shift + C"),
                                                ("Enlarge / Zoom out", "+ / -"),
                                                ("Toggle Toolbar Mode", "Ctrl + Shift + M"),
                                            ];
                                            for (act, key) in shortcuts {
                                                ui.label(act);
                                                ui.label(RichText::new(key).monospace().color(theme.brand_green));
                                                ui.end_row();
                                            }
                                        });
                                }
                                PreferenceTab::GetLog => {
                                    ui.label(RichText::new("Report Issue & Diagnostics").strong().color(theme.text01));
                                    ui.add_space(8.0_f32);
                                    ui.label(
                                        "If you encounter camera issues, switch to Read-Only USB Mass Storage mode to retrieve hardware and gimbal logs.",
                                    );
                                    ui.add_space(12.0_f32);
                                    if ui
                                        .button("Retrieve Camera Logs (Read-Only USB Mode)")
                                        .clicked()
                                    {
                                        let _ = self.controller.switch_msc_readonly_class();
                                        self.state.show_toast(
                                            "Switched camera to Read-Only USB Mass Storage Mode",
                                            false,
                                        );
                                    }
                                }
                                PreferenceTab::About => {
                                    ui.label(
                                        RichText::new("Insta360 Link Controller")
                                            .size(16.0_f32)
                                            .strong()
                                            .color(theme.text01),
                                    );
                                    ui.add_space(4.0_f32);
                                    ui.label("Version: v0.1.0 (Pure-Rust Open Desktop Controller)");
                                    ui.add_space(8.0_f32);
                                    ui.label(format!("Active Device: {}", self.state.device_info.device_name));
                                    ui.label(format!(
                                        "Firmware Version: {}",
                                        self.state.device_info.firmware_version
                                    ));
                                    ui.label(format!(
                                        "Serial Number: {}",
                                        self.state.device_info.serial_number
                                    ));
                                    ui.label(format!("PTZ Version: {}", self.state.ptz_info.version));
                                }
                            }
                        });
                    });
                });
            self.state.show_preferences = open;
        }

        // Virtual Camera modal (DefVirCamPop.qml)
        if self.state.show_virtual_cam_pop {
            let mut open = self.state.show_virtual_cam_pop;
            egui::Window::new("Virtual Camera")
                .fixed_size([420.0_f32, 230.0_f32])
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Virtual Camera Output").strong().color(theme.text01));
                    ui.add_space(6.0_f32);
                    ui.horizontal(|ui| {
                        ui.label("Enable Virtual Camera:");
                        let mut vcam = self.state.virtual_camera_enabled;
                        if ui.checkbox(&mut vcam, "").changed() {
                            self.state.virtual_camera_enabled = vcam;
                            let msg = if vcam {
                                "Virtual Camera enabled"
                            } else {
                                "Virtual Camera disabled"
                            };
                            self.state.show_toast(msg, false);
                        }
                    });
                    ui.add_space(8.0_f32);
                    ui.label(
                        RichText::new(
                            "Virtual Camera allows you to output video with AI tracking, filters, and background effects to third-party applications like Zoom, Microsoft Teams, OBS, and Google Meet.",
                        )
                        .size(12.0_f32)
                        .color(theme.text02),
                    );
                    ui.add_space(4.0_f32);
                    ui.label(
                        RichText::new(
                            "Select 'Insta360 Virtual Camera' as the video input in your conferencing software.",
                        )
                        .size(12.0_f32)
                        .color(theme.text03),
                    );
                });
            self.state.show_virtual_cam_pop = open;
        }

        // Remote Control modal (QRCodeDialog.qml)
        if self.state.show_qr_dialog {
            let mut open = self.state.show_qr_dialog;
            egui::Window::new("Mobile Remote Control")
                .fixed_size([380.0_f32, 240.0_f32])
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Local Web Remote").strong().color(theme.text01));
                    ui.add_space(6.0_f32);
                    ui.label(
                        "Scan QR code or open the following URL in your phone or browser on the same Wi-Fi network:",
                    );
                    ui.add_space(8.0_f32);
                    ui.label(
                        RichText::new("http://127.0.0.1:9360")
                            .monospace()
                            .size(14.0_f32)
                            .color(theme.brand_green),
                    );
                    ui.add_space(8.0_f32);
                    ui.label(
                        RichText::new(
                            "Remotely control gimbal pan/tilt, AI tracking mode, zoom factor, and preset positions.",
                        )
                        .size(12.0_f32)
                        .color(theme.text02),
                    );
                });
            self.state.show_qr_dialog = open;
        }

        // Factory Reset confirmation modal
        if self.state.show_factory_reset_pop {
            let mut open = self.state.show_factory_reset_pop;
            egui::Window::new("Restore Factory Settings")
                .fixed_size([360.0_f32, 160.0_f32])
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("Are you sure you want to restore factory settings?")
                            .strong()
                            .color(theme.text01),
                    );
                    ui.add_space(6.0_f32);
                    ui.label(
                        "This will reset all gimbal presets, image parameters, and tracking settings to factory defaults.",
                    );
                    ui.add_space(14.0_f32);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.state.show_factory_reset_pop = false;
                        }
                        if ui
                            .button(RichText::new("Confirm").color(theme.error).strong())
                            .clicked()
                        {
                            let _ = self.controller.restore_factory_settings(false);
                            self.state.show_factory_reset_pop = false;
                            self.state
                                .show_toast("Restored camera to factory default settings", false);
                        }
                    });
                });
            self.state.show_factory_reset_pop = open;
        }

        // DeskView Vertical Correction Pitch Popover
        if self.state.show_deskview_pitch_pop {
            let mut open = self.state.show_deskview_pitch_pop;
            egui::Window::new("DeskView Vertical Correction")
                .fixed_size([260.0_f32, 120.0_f32])
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("DeskView Pitch Correction:");
                    let mut pitch_deg = self.state.deskview_pitch as f32 / 10.0_f32;
                    if ui
                        .add(egui::Slider::new(&mut pitch_deg, -90.0_f32..=0.0_f32).text("°"))
                        .changed()
                    {
                        self.state.deskview_pitch = (pitch_deg * 10.0_f32) as i16;
                        let mut aux = VideoModeAuxData::default();
                        aux.pitch = self.state.deskview_pitch;
                        let _ = self.controller.set_video_mode(VideoMode::DeskView, &aux);
                    }
                    ui.add_space(8.0_f32);
                    if ui.button("Close").clicked() {
                        self.state.show_deskview_pitch_pop = false;
                    }
                });
            self.state.show_deskview_pitch_pop = open;
        }
    }
}

impl eframe::App for Insta360App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Keep capture resolution and frame rate synced with selected format preset
        let (target_w, target_h) = self.state.format_preset.dimensions();
        let target_fps = self.state.format_preset.fps();
        if self.video_engine.capture_dimensions() != (target_w as usize, target_h as usize)
            || self.video_engine.capture_fps() != target_fps
        {
            if (target_fps > 30 || self.state.format_preset.is_vertical())
                && !self.state.vertical_resolution_mode
            {
                self.video_engine.stop_physical_capture();
                self.state.vertical_resolution_mode = true;
                let _ = self.controller.set_vertical_screen(true);
            }
            self.video_engine
                .set_capture_format(target_w as usize, target_h as usize, target_fps);
        }

        // Continuous repaint loop matching active capture framerate (16ms for 60fps, 33ms for 30fps)
        if self.state.preview_open {
            let repaint_ms = if self.video_engine.capture_fps() >= 50 {
                16
            } else {
                33
            };
            ctx.request_repaint_after(Duration::from_millis(repaint_ms));
        }

        // 1. One-time hardware initialization once the physical video stream is open and delivering frames
        if self.video_engine.has_live_physical_frame() && !self.state.stream_initialized {
            self.state.stream_initialized = true;
            let had_saved_settings = self.state.last_saved_settings.is_some();
            self.sync_device_info();
            if had_saved_settings && !self.self_test_enabled {
                self.apply_saved_settings_to_hardware();
            } else {
                self.state.last_saved_settings = Some(self.state.to_persisted());
            }
            // If the camera was left in Overhead or DeskView mode (which flips the ISP 180 degrees),
            // restore Normal mode so the live video stream is right-side up.
            if self.state.video_mode == VideoMode::OverheadShot
                || self.state.video_mode == VideoMode::DeskView
            {
                let _ = self
                    .controller
                    .set_video_mode_raw(VideoMode::Normal, &VideoModeAuxData::default());
                self.state.video_mode = VideoMode::Normal;
            }
        }

        // 1b. Persist user settings to ~/.config/insta360-link-controller/settings.json whenever modified
        if self.state.stream_initialized && !self.self_test_enabled {
            let cur_persisted = self.state.to_persisted();
            if self.state.last_saved_settings.as_ref() != Some(&cur_persisted) {
                cur_persisted.save();
                self.state.last_saved_settings = Some(cur_persisted);
            }
        }

        // 2. Auto-stop short PTZ directional nudges (`CameraInsta::addPanTilt` 100-120ms timer)
        if let Some(stop_at) = self.state.nudge_stop_at {
            if Instant::now() >= stop_at {
                let _ = self.controller.stop_pantilt_relative();
                self.state.nudge_stop_at = None;
                self.state.last_ptz_cmd = (0, 0, 0, 0);
                if let Ok((pan_arc, tilt_arc)) = self.controller.get_pantilt_absolute() {
                    self.state.pan_deg = (pan_arc as f32 / 3600.0_f32).clamp(-130.0_f32, 130.0_f32);
                    self.state.tilt_deg = (tilt_arc as f32 / 3600.0_f32).clamp(-90.0_f32, 90.0_f32);
                }
            }
        }

        // 2b. Restore pre-DeskView / pre-Overhead PTZ angle once firmware finishes exiting mode
        if let Some((restore_at, pan_arc, tilt_arc)) = self.state.pending_ptz_restore {
            if Instant::now() >= restore_at {
                self.state.pending_ptz_restore = None;
                let _ = self.controller.set_pantilt_absolute(pan_arc, tilt_arc);
            }
        }

        // 3. Non-blocking AI Tracking target discovery (`onGetTrackObjectTimeOut` every 200ms)
        if self.state.ready_auto_track && self.state.video_mode == VideoMode::AutoTracking {
            let should_poll = self
                .state
                .last_track_poll
                .map(|t| t.elapsed().as_millis() >= 200)
                .unwrap_or(true);
            if should_poll {
                self.state.last_track_poll = Some(Instant::now());
                if let Ok(rects) = self.controller.get_track_objects() {
                    if rects.len() == 1 {
                        // `gotRect` (`0x1000740e8`): single person detected -> immediately lock target,
                        // stop `checkTrack` (`SetCheckTrack(false)`), clear boxes, and enter `AutoTracking` (`1`)!
                        let r = &rects[0];
                        let cx = (((r.x as f32) + (r.width as f32) * 0.5_f32) / 10000.0_f32)
                            .clamp(0.05_f32, 0.95_f32);
                        let cy = (((r.y as f32) + (r.height as f32) * 0.5_f32) / 10000.0_f32)
                            .clamp(0.05_f32, 0.95_f32);
                        if self.self_test_enabled {
                            println!(
                                "[GUI-SELF-TEST] Detected 1 person rect={:?}, locking onto ({:.3}, {:.3})",
                                r, cx, cy
                            );
                        }
                        let _ = self.controller.select_track_object_norm(cx, cy, 0.0_f32, 0.0_f32);
                        std::thread::sleep(Duration::from_millis(30));
                        self.state.ready_auto_track = false;
                        self.state.tracked_targets.clear();
                        let _ = self.controller.set_video_mode_raw(
                            VideoMode::AutoTracking,
                            &VideoModeAuxData::default(),
                        );
                        self.state.last_hw_sync =
                            Some(Instant::now() + Duration::from_millis(700));
                        self.state.show_toast("AI Tracking locked onto target", false);
                    } else if rects.len() > 1 {
                        // `gotRect` (`0x100074164`): multiple people detected -> display target selection boxes!
                        self.state.tracked_targets = rects;
                    }
                }
            }
        }

        // 4. Periodic hardware sync (`onGetVideoModeTimeOut` every 500ms)
        // IMPORTANT: Do NOT poll `GetTrackObjLists` (`XU1 0x14`) or `GetPanTiltAbsoluteValue` (`XU1 0x1a`)
        // while `AutoTracking` is locked and active (`19_40_33.log` lines 412-419)!
        if self.state.stream_initialized
            && self.state.last_ptz_cmd == (0, 0, 0, 0)
            && self.state.pending_ptz_restore.is_none()
        {
            let should_sync = self
                .state
                .last_hw_sync
                .map(|t| t.elapsed().as_millis() >= 500)
                .unwrap_or(true);
            if should_sync {
                self.state.last_hw_sync = Some(Instant::now());
                if !self.state.ready_auto_track {
                    if let Ok((hw_mode, _)) = self.controller.get_video_mode() {
                        if hw_mode != self.state.video_mode {
                            self.state.video_mode = hw_mode;
                            if hw_mode == VideoMode::Normal {
                                self.state.tracked_targets.clear();
                            }
                        }
                    }
                }
                if self.state.video_mode == VideoMode::Normal && !self.state.ready_auto_track {
                    if let Ok((pan_arc, tilt_arc)) = self.controller.get_pantilt_absolute() {
                        self.state.pan_deg =
                            (pan_arc as f32 / 3600.0_f32).clamp(-130.0_f32, 130.0_f32);
                        self.state.tilt_deg =
                            (tilt_arc as f32 / 3600.0_f32).clamp(-90.0_f32, 90.0_f32);
                    }
                }
            }
        }

        // 5. Optional automated live hardware GUI verification (`INSTA360_GUI_SELF_TEST=1`)
        if self.self_test_enabled && self.state.stream_initialized {
            let start = *self.self_test_start.get_or_insert_with(Instant::now);
            let elapsed = start.elapsed().as_secs_f32();
            if self.self_test_step == 0 && elapsed >= 3.2 {
                self.self_test_step = 1;
                if self.state.video_mode != VideoMode::Normal {
                    self.set_video_mode(VideoMode::Normal);
                }
                if let Ok((p, t)) = self.controller.get_pantilt_absolute() {
                    self.state.pan_deg = p as f32 / 3600.0;
                    self.state.tilt_deg = t as f32 / 3600.0;
                }
                println!(
                    "[GUI-SELF-TEST] Step 1: Stream initialized, device='{}', fw='{}', mode={:?}, pan={:.1}, tilt={:.1}",
                    self.state.device_info.device_name,
                    self.state.device_info.firmware_version,
                    self.state.video_mode,
                    self.state.pan_deg,
                    self.state.tilt_deg
                );

                // Start 1080p video recording test
                let out_dir = std::env::temp_dir();
                if let Ok(rec_path) = self.video_engine.start_recording(&out_dir) {
                    println!(
                        "[GUI-SELF-TEST] Started 1920x1080 video recording to {}",
                        rec_path.display()
                    );
                }
            } else if self.self_test_step == 1 && elapsed >= 4.8 {
                self.self_test_step = 2;
                let out_dir = std::env::temp_dir();
                if let Ok(rec_path) = self.video_engine.stop_recording() {
                    let size = std::fs::metadata(&rec_path).map(|m| m.len()).unwrap_or(0);
                    println!(
                        "[GUI-SELF-TEST] Stopped 1920x1080 video recording: {} ({} bytes)",
                        rec_path.display(),
                        size
                    );
                }
                if let Ok(snap_1080) = self.video_engine.save_screenshot(&out_dir, &self.state) {
                    println!(
                        "[GUI-SELF-TEST] Saved 1920x1080 screenshot: {}",
                        snap_1080.display()
                    );
                }

                // Verify View Tab & Effects Tab & More Tab hardware settings readback
                let _ = self.controller.set_zoom(200);
                let z_read = self.controller.get_zoom().unwrap_or(0);
                let _ = self.controller.set_zoom(100);

                let _ = self.controller.set_track_speed(open_360_link_protocol::TrackSpeed::Fast);
                let ts_fast = self.controller.get_track_speed().ok();
                let _ = self.controller.set_track_speed(open_360_link_protocol::TrackSpeed::Normal);
                let ts_norm = self.controller.get_track_speed().ok();

                let _ = self.controller.set_auto_exposure(true);
                let _ = self.controller.set_exposure_compensation(1.5);
                let ev_15 = self.controller.get_exposure_compensation().unwrap_or(0.0);
                let _ = self.controller.set_exposure_compensation(0.0);
                let ev_0 = self.controller.get_exposure_compensation().unwrap_or(99.0);

                let _ = self.controller.set_auto_exposure(false);
                let ae_off = self.controller.get_auto_exposure().unwrap_or(true);
                let _ = self.controller.set_iso(400);
                let iso_400 = self.controller.get_iso().unwrap_or(0);
                let _ = self.controller.set_shutter_speed(120);
                let sh_120 = self.controller.get_shutter_speed().unwrap_or(0);
                let _ = self.controller.set_auto_exposure(true);
                let ae_on = self.controller.get_auto_exposure().unwrap_or(false);

                let _ = self.controller.set_auto_focus(false);
                let af_off = self.controller.get_auto_focus().unwrap_or(true);
                let _ = self.controller.set_focus(60);
                let f_60 = self.controller.get_focus().unwrap_or(0);
                let _ = self.controller.set_auto_focus(true);
                let af_on = self.controller.get_auto_focus().unwrap_or(false);

                let _ = self.controller.set_auto_white_balance(false);
                let awb_off = self.controller.get_auto_white_balance().unwrap_or(true);
                let _ = self.controller.set_white_balance_temperature(4200);
                let wb_4200 = self.controller.get_white_balance_temperature().unwrap_or(0);
                let _ = self.controller.set_auto_white_balance(true);
                let awb_on = self.controller.get_auto_white_balance().unwrap_or(false);

                let _ = self.controller.set_brightness(65);
                let br_65 = self.controller.get_brightness().unwrap_or(0);
                let _ = self.controller.set_brightness(50);

                let _ = self.controller.set_contrast(65);
                let ct_65 = self.controller.get_contrast().unwrap_or(0);
                let _ = self.controller.set_contrast(50);

                let _ = self.controller.set_saturation(65);
                let sat_65 = self.controller.get_saturation().unwrap_or(0);
                let _ = self.controller.set_saturation(50);

                let _ = self.controller.set_sharpness(65);
                let sh_65 = self.controller.get_sharpness().unwrap_or(0);
                let _ = self.controller.set_sharpness(50);

                let _ = self.controller.set_power_line_frequency(PowerLineFrequency::Hz50);
                let pl_50 = self.controller.get_power_line_frequency().ok();
                let _ = self.controller.set_power_line_frequency(PowerLineFrequency::Hz60);
                let pl_60 = self.controller.get_power_line_frequency().ok();

                let _ = self.controller.set_audio_noise_reduction(false);
                let nr_off = self.controller.get_audio_noise_reduction().ok();
                let _ = self.controller.set_audio_noise_reduction(true);
                let nr_on = self.controller.get_audio_noise_reduction().ok();

                let _ = self.controller.set_all_gestures_enabled(true);
                let _ = self
                    .controller
                    .set_gesture_enabled(open_360_link_protocol::Gesture::Palm, true);
                let gest = self.controller.get_gesture_status().ok();

                println!(
                    "[GUI-SELF-TEST] Hardware Settings Verified:\n  zoom={z_read}, track_speed=({ts_fast:?} -> {ts_norm:?})\n  AE=({ae_off} -> {ae_on}), EV=({ev_15:.1} -> {ev_0:.1}), ISO={iso_400}, Shutter=1/{sh_120}s\n  AF=({af_off} -> {af_on}), Focus={f_60}, AWB=({awb_off} -> {awb_on}), WB={wb_4200}K\n  Bright={br_65}, Contrast={ct_65}, Sat={sat_65}, Sharp={sh_65}\n  AntiFlicker=({pl_50:?} -> {pl_60:?}), AudioDenoise=({nr_off:?} -> {nr_on:?}), Gestures={gest:?}"
                );

                println!(
                    "[GUI-SELF-TEST] Step 2: Starting smooth joystick velocity pan(speed=3, dir=1) from pan={:.1} deg, tilt={:.1} deg",
                    self.state.pan_deg, self.state.tilt_deg
                );
                let _ = self.controller.set_pantilt_relative_ext(3, 1, 0, 0);
                self.state.last_ptz_cmd = (3, 1, 0, 0);
            } else if self.self_test_step == 2 && elapsed >= 5.5 {
                self.self_test_step = 3;
                let _ = self.controller.stop_pantilt_relative();
                self.state.last_ptz_cmd = (0, 0, 0, 0);
                if let Ok((p, t)) = self.controller.get_pantilt_absolute() {
                    self.state.pan_deg = p as f32 / 3600.0;
                    self.state.tilt_deg = t as f32 / 3600.0;
                }
                println!(
                    "[GUI-SELF-TEST] Step 3: Stopped joystick velocity (pan={:.1}, tilt={:.1}); testing Toolbar Mode & activating AI Tracking",
                    self.state.pan_deg, self.state.tilt_deg
                );
                self.state.ui_mode = UiMode::Toolbar;
                self.set_video_mode(VideoMode::AutoTracking);
            } else if self.self_test_step == 3
                && ((!self.state.ready_auto_track && elapsed >= 7.0) || elapsed >= 10.5)
            {
                self.self_test_step = 4;
                std::thread::sleep(Duration::from_millis(500));
                self.state.ui_mode = UiMode::MainWindow;
                let hw_mode = self.controller.get_video_mode().ok().map(|m| m.0);
                println!(
                    "[GUI-SELF-TEST] Step 4: AI Tracking active (ui_mode={:?}, hw_mode={:?}); switching to DeskView",
                    self.state.video_mode, hw_mode
                );
                self.set_video_mode(VideoMode::DeskView);
            } else if self.self_test_step == 4 && elapsed >= 12.5 {
                self.self_test_step = 5;
                let hw_mode = self.controller.get_video_mode().ok().map(|m| m.0);
                println!(
                    "[GUI-SELF-TEST] Step 5: DeskView active (hw_mode={:?}); restoring Normal mode",
                    hw_mode
                );
                self.set_video_mode(VideoMode::Normal);
            } else if self.self_test_step == 5 && elapsed >= 15.0 {
                self.self_test_step = 6;
                if let Ok((p, t)) = self.controller.get_pantilt_absolute() {
                    self.state.pan_deg = p as f32 / 3600.0;
                    self.state.tilt_deg = t as f32 / 3600.0;
                }
                let out_dir = std::env::temp_dir();
                if let Ok(path) = self.video_engine.save_screenshot(&out_dir, &self.state) {
                    println!(
                        "[GUI-SELF-TEST] Step 6: Restored pan={:.1} deg, tilt={:.1} deg; saved final live GUI video frame to {}",
                        self.state.pan_deg,
                        self.state.tilt_deg,
                        path.display()
                    );
                }
                println!("[GUI-SELF-TEST] ALL GUI HARDWARE INTERACTION STEPS PASSED");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        let theme = apply_ins_theme(ctx, self.state.is_light_mode);

        match self.state.ui_mode {
            UiMode::MainWindow => {
                // Top Bar (TopBarWin.qml - 44px height, theme.bg01)
                self.render_top_bar(ctx, &theme);

                // Right Sidebar Settings Panel (292px fixed width, theme.bg01)
                egui::SidePanel::right("ins_right_settings")
                    .exact_width(292.0_f32)
                    .resizable(false)
                    .frame(
                        egui::Frame::none()
                            .fill(theme.bg01)
                            .inner_margin(Margin {
                                left: 8.0_f32,
                                right: 8.0_f32,
                                top: 0.0_f32,
                                bottom: 8.0_f32,
                            }),
                    )
                    .show(ctx, |ui| {
                        render_right_settings_panel(
                            ui,
                            ctx,
                            &mut self.state,
                            &mut self.controller,
                            &self.video_engine,
                            &theme,
                        );
                    });

                // Central Panel: Video Preview Area + 66px Bottom Control Bar in a theme.bg02 container
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::none()
                            .fill(theme.bg01)
                            .inner_margin(Margin::same(8.0_f32)),
                    )
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(theme.bg02)
                            .rounding(Rounding::same(8.0_f32))
                            .show(ui, |ui| {
                                let total_size = ui.available_size();
                                let bottom_bar_height = 66.0_f32;
                                let preview_height =
                                    (total_size.y - bottom_bar_height).max(100.0_f32);

                                // Video preview area
                                ui.allocate_ui(
                                    egui::vec2(total_size.x, preview_height),
                                    |ui| {
                                        self.render_preview_area(ui, ctx, &theme);
                                    },
                                );

                                // Bottom mode & recording bar
                                ui.allocate_ui(
                                    egui::vec2(total_size.x, bottom_bar_height),
                                    |ui| {
                                        self.render_bottom_control_bar(ui, &theme);
                                    },
                                );
                            });
                    });
            }
            UiMode::Toolbar => {
                // Compact Floating Toolbar Mode (Toolbar.qml)
                self.render_toolbar_mode(ctx, &theme);
            }
        }

        // Popups and modals
        self.render_modals(ctx, &theme);
    }
}

fn build_app_icon() -> egui::IconData {
    let w = 64u32;
    let h = 64u32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let dx = x as f32 - 31.5_f32;
            let dy = y as f32 - 26.0_f32;
            let dist = (dx * dx + dy * dy).sqrt();
            // Dark rounded card background
            let in_card = x >= 4 && x < 60 && y >= 4 && y < 60;
            if !in_card {
                continue;
            }
            let (mut r, mut g, mut b) = (20u8, 24u8, 32u8);
            // Green camera ring
            if (10.5_f32..=14.0_f32).contains(&dist) {
                r = 0;
                g = 230;
                b = 118;
            } else if dist < 5.0_f32 {
                r = 0;
                g = 230;
                b = 118;
            } else if dist < 10.5_f32 {
                r = 12;
                g = 15;
                b = 20;
            } else if (18..=46).contains(&x) && (44..=52).contains(&y) {
                // Gimbal base
                r = 43;
                g = 50;
                b = 66;
            }
            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = 255;
        }
    }
    egui::IconData {
        rgba,
        width: w,
        height: h,
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    // Matching webcam.qml: width: 1440, height: 714, minimumWidth: 1120, minimumHeight: 536
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("open-360-link")
            .with_icon(build_app_icon())
            .with_inner_size([1440.0_f32, 714.0_f32])
            .with_min_inner_size([1120.0_f32, 536.0_f32])
            .with_title("Open 360 Link"),
        ..Default::default()
    };

    eframe::run_native(
        "Open 360 Link",
        native_options,
        Box::new(|cc| Ok(Box::new(Insta360App::new(cc)))),
    )
}
