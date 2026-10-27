use egui::{
    Align, Align2, Color32, Context, FontId, Grid, Layout, Pos2, Rect, RichText, Rounding,
    ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};
use open_360_link_protocol::{
    AudioCaptureMode, CameraModel, CompositionStyle, ExposureCurve, FilterMode, HostPtzInfo,
    ImageOperation, Insta360CameraController, PowerLineFrequency, TrackSpeed, UvcControlTransport,
    Gesture,
};

use crate::state::{
    BackgroundEffectMode, CameraAppState, ColorPresetSlot, PtzPresetSlot, RightSidebarTab,
    ISO_LIST, SHUTTER_DENOM_LIST,
};
use crate::theme::{
    ins_card, ins_checkbox_row, ins_section_header, ins_slider_f32, ins_slider_i32,
    ins_switch_row, ins_tab_button, InsTheme,
};
use crate::video_engine::VideoPreviewEngine;

/// Evaluates a smooth Hermite spline through monotonic control points normalized to [0..1].
fn evaluate_spline(points: &[(f32, f32)], t: f32) -> f32 {
    if points.is_empty() {
        return t;
    }
    if points.len() == 1 {
        return points[0].1;
    }
    if t <= points[0].0 {
        return points[0].1;
    }
    if t >= points[points.len() - 1].0 {
        return points[points.len() - 1].1;
    }
    for i in 0..points.len() - 1 {
        let p0 = points[i];
        let p1 = points[i + 1];
        if t >= p0.0 && t <= p1.0 {
            let span = p1.0 - p0.0;
            if span.abs() < 1e-5 {
                return p0.1;
            }
            let alpha = (t - p0.0) / span;
            let s = alpha * alpha * (3.0_f32 - 2.0_f32 * alpha);
            return p0.1 + s * (p1.1 - p0.1);
        }
    }
    t
}

/// Constructs a 128-point hardware ExposureCurve payload from spline points.
fn build_exposure_curve(points: &[(f32, f32)], mode: u8, iso: u16) -> ExposureCurve {
    let mut curve_points = Vec::with_capacity(128);
    for i in 0..128 {
        let t = i as f32 / 127.0_f32;
        let val = evaluate_spline(points, t).clamp(0.0_f32, 1.0_f32);
        curve_points.push((val * 255.0_f32) as u16);
    }
    ExposureCurve {
        mode,
        iso,
        points: curve_points,
    }
}

/// Renders the exact 284px right-hand sidebar (`Settings.qml`) and its 3 tabs
/// (`View`, `Effects`, `More`) of the official Insta360 Link Controller v2.0.2 desktop app.
pub fn render_right_settings_panel<T: UvcControlTransport>(
    ui: &mut Ui,
    ctx: &Context,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    video: &VideoPreviewEngine,
    theme: &InsTheme,
) {
    ui.set_width(284.0_f32);

    // ========================================================================
    // 1. Top Tab Bar (`Settings.qml`)
    // ========================================================================
    let tab_bar_rect = ui.allocate_space(Vec2::new(ui.available_width(), 40.0_f32)).1;
    ui.painter().rect_filled(tab_bar_rect, Rounding::same(8.0_f32), theme.bg02);

    let tab_w = ((tab_bar_rect.width() - 16.0_f32) / 3.0_f32).max(10.0_f32);
    let mut tab_ui = ui.new_child(egui::UiBuilder::new().max_rect(tab_bar_rect));
    tab_ui.horizontal(|ui| {
        ui.add_space(4.0_f32);
        if ins_tab_button(ui, "View", state.right_tab == RightSidebarTab::View, tab_w, theme) {
            state.right_tab = RightSidebarTab::View;
        }
        if ins_tab_button(ui, "Effects", state.right_tab == RightSidebarTab::Effects, tab_w, theme) {
            state.right_tab = RightSidebarTab::Effects;
        }
        if ins_tab_button(ui, "More", state.right_tab == RightSidebarTab::More, tab_w, theme) {
            state.right_tab = RightSidebarTab::More;
        }
    });

    ui.add_space(8.0_f32);

    // ========================================================================
    // 2. Scrollable Tab Content (`ScrollArea::vertical()`)
    // ========================================================================
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| match state.right_tab {
            RightSidebarTab::View => render_view_tab(ui, ctx, state, ctrl, video, theme),
            RightSidebarTab::Effects => render_effects_tab(ui, ctx, state, ctrl, video, theme),
            RightSidebarTab::More => render_more_tab(ui, state, ctrl, theme),
        });
}

// ============================================================================
// Tab 1: `RightSidebarTab::View` (`PTZControl.qml`)
// ============================================================================
fn render_view_tab<T: UvcControlTransport>(
    ui: &mut Ui,
    _ctx: &Context,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    _video: &VideoPreviewEngine,
    theme: &InsTheme,
) {
    // ------------------------------------------------------------------------
    // Card 1: `View Adjustment`
    // ------------------------------------------------------------------------
    ins_card(ui, theme, |ui| {
        if ins_section_header(ui, "View Adjustment", theme, Some("Center gimbal & reset zoom")) {
            state.pan_deg = 0.0_f32;
            state.tilt_deg = 0.0_f32;
            state.zoom_x = 1.0_f32;
            state.host_ptz_x = 0.5_f32;
            state.host_ptz_y = 0.5_f32;
            let _ = ctrl.reset_gimbal();
            let _ = ctrl.set_zoom(100);
        }

        ui.add_space(6.0_f32);

        ui.horizontal(|ui| {
            // Left Side: Joystick D-Pad (Link / Link 2) or Digital Viewport Crop Box (Link 2C)
            if ctrl.capabilities().support_ptz_drag {
                render_joystick_pad(ui, state, ctrl, theme);
            } else {
                render_viewport_crop_box(ui, state, ctrl, theme);
            }

            ui.add_space(10.0_f32);

            // Right Side: Vertical Zoom Rocker (`DefAdjustment.qml`)
            render_zoom_rocker(ui, state, ctrl, theme);
        });
    });

    ui.add_space(10.0_f32);

    // ------------------------------------------------------------------------
    // Card 2: `Tracking Settings` / `Framing Settings`
    // ------------------------------------------------------------------------
    ins_card(ui, theme, |ui| {
        let title = if ctrl.capabilities().support_ptz_drag {
            "Tracking Settings"
        } else {
            "Framing Settings"
        };
        ins_section_header(ui, title, theme, None);

        ui.add_space(6.0_f32);

        // Pause-Track Area row
        if ctrl.capabilities().support_area_track {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Pause-Track Area").size(13.0_f32).color(theme.text01));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let btn_text = if state.show_pause_track_overlay {
                        "Editing..."
                    } else {
                        "Set"
                    };
                    let btn_color = if state.show_pause_track_overlay {
                        theme.brand_green
                    } else {
                        theme.text01
                    };
                    if ui.button(RichText::new(btn_text).size(12.0_f32).color(btn_color)).clicked() {
                        state.show_pause_track_overlay = !state.show_pause_track_overlay;
                        if state.show_pause_track_overlay {
                            let _ = ctrl.enter_track_forbidden_area_setting();
                        } else {
                            let _ = ctrl.exit_track_forbidden_area_setting();
                        }
                    }
                });
            });
            ui.add_space(6.0_f32);
        }

        // Tracking Speed row
        if ctrl.capabilities().support_track_speed {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tracking Speed").size(13.0_f32).color(theme.text01));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let cur_label = match state.track_speed {
                        TrackSpeed::Fast => "Quick",
                        TrackSpeed::Normal => "Ordinary",
                        TrackSpeed::Slow => "Slow",
                    };
                    egui::ComboBox::from_id_salt("tracking_speed_combo")
                        .selected_text(cur_label)
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut state.track_speed, TrackSpeed::Fast, "Quick").clicked() {
                                let _ = ctrl.set_track_speed(TrackSpeed::Fast);
                            }
                            if ui.selectable_value(&mut state.track_speed, TrackSpeed::Normal, "Ordinary").clicked() {
                                let _ = ctrl.set_track_speed(TrackSpeed::Normal);
                            }
                            if ui.selectable_value(&mut state.track_speed, TrackSpeed::Slow, "Slow").clicked() {
                                let _ = ctrl.set_track_speed(TrackSpeed::Slow);
                            }
                        });
                });
            });
            ui.add_space(6.0_f32);
        }

        // Touch Key AI Tracking switch row (Link 1 only)
        if ctrl.model() == CameraModel::Link {
            if ins_switch_row(
                ui,
                "Touch Key AI Tracking",
                Some("Tap the Touch once to enter/exit AI Tracking"),
                &mut state.enable_single_tap_tracking,
                true,
                theme,
            ) {
                let _ = ctrl.set_single_tap_tracking_enabled(state.enable_single_tap_tracking);
            }
        }
    });
}

// ============================================================================
// Tab 2: `RightSidebarTab::Effects` (`ImageAdjust.qml`)
// ============================================================================
fn render_effects_tab<T: UvcControlTransport>(
    ui: &mut Ui,
    ctx: &Context,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    video: &VideoPreviewEngine,
    theme: &InsTheme,
) {
    // ------------------------------------------------------------------------
    // Card 1: `Color` (`ImageScreen.qml`)
    // ------------------------------------------------------------------------
    ins_card(ui, theme, |ui| {
        if ins_section_header(ui, "Color", theme, Some("Reset color settings")) {
            state.auto_exposure = true;
            state.ev_compensation = 0.0_f32;
            state.auto_focus = true;
            state.auto_white_balance = true;
            state.wb_kelvin = 5000;
            state.brightness = 50;
            state.contrast = 50;
            state.saturation = 50;
            state.sharpness = 50;
            state.hdr_enabled = false;
            state.spline_points = vec![
                (0.0_f32, 0.0_f32),
                (0.33_f32, 0.33_f32),
                (0.67_f32, 0.67_f32),
                (1.0_f32, 1.0_f32),
            ];

            let _ = ctrl.set_auto_exposure(true);
            let _ = ctrl.set_exposure_compensation(0.0_f32);
            let _ = ctrl.set_auto_focus(true);
            let _ = ctrl.set_auto_white_balance(true);
            let _ = ctrl.set_white_balance_temperature(5000);
            let _ = ctrl.set_brightness(50);
            let _ = ctrl.set_contrast(50);
            let _ = ctrl.set_saturation(50);
            let _ = ctrl.set_sharpness(50);
            let _ = ctrl.set_hdr(false);
            let curve = build_exposure_curve(&state.spline_points, 1, 100);
            let _ = ctrl.set_exposure_curve(&curve);
        }

        ui.add_space(6.0_f32);

        // Exposure switch row
        if ins_switch_row(
            ui,
            "Exposure",
            Some("Auto Exposure (disabled when HDR is on)"),
            &mut state.auto_exposure,
            !state.hdr_enabled,
            theme,
        ) {
            let _ = ctrl.set_auto_exposure(state.auto_exposure);
        }

        if state.auto_exposure {
            if ins_slider_f32(
                ui,
                Some("Exposure Compensation"),
                &mut state.ev_compensation,
                -3.0_f32,
                3.0_f32,
                0.3_f32,
                1,
                "EV",
                !state.hdr_enabled,
                theme,
            ) {
                let _ = ctrl.set_exposure_compensation(state.ev_compensation);
            }
        } else {
            // ISO Slider
            let mut iso_idx = state.iso_index as i32;
            let iso_suffix = format!(" ({})", ISO_LIST.get(state.iso_index).copied().unwrap_or(100));
            if ins_slider_i32(ui, Some("ISO"), &mut iso_idx, 0, 15, &iso_suffix, true, theme) {
                state.iso_index = iso_idx as usize;
                if let Some(&iso_val) = ISO_LIST.get(state.iso_index) {
                    let _ = ctrl.set_iso(iso_val);
                }
            }

            // Shutter Slider
            let mut shutter_idx = state.shutter_index as i32;
            let shutter_suffix = format!(
                " (1/{}s)",
                SHUTTER_DENOM_LIST.get(state.shutter_index).copied().unwrap_or(100)
            );
            if ins_slider_i32(ui, Some("Shutter"), &mut shutter_idx, 0, 19, &shutter_suffix, true, theme) {
                state.shutter_index = shutter_idx as usize;
                if let Some(&shutter_denom) = SHUTTER_DENOM_LIST.get(state.shutter_index) {
                    let _ = ctrl.set_shutter_speed(shutter_denom);
                }
            }

            ui.add_space(6.0_f32);

            // Exposure Curve (`DefSplineCanvas.qml`)
            render_exposure_curve_canvas(ui, state, ctrl, theme);
        }

        ui.add_space(6.0_f32);

        // Auto Focus switch row
        if ins_switch_row(ui, "Auto Focus", None, &mut state.auto_focus, true, theme) {
            let _ = ctrl.set_auto_focus(state.auto_focus);
        }
        if !state.auto_focus {
            let mut focus_val = state.manual_focus as i32;
            if ins_slider_i32(ui, Some("Focus"), &mut focus_val, 0, 100, "%", true, theme) {
                state.manual_focus = focus_val as u16;
                let _ = ctrl.set_focus(state.manual_focus);
            }
        }

        ui.add_space(6.0_f32);

        // White Balance switch row
        if ins_switch_row(ui, "White Balance", None, &mut state.auto_white_balance, true, theme) {
            let _ = ctrl.set_auto_white_balance(state.auto_white_balance);
        }
        if !state.auto_white_balance {
            let mut wb_val = state.wb_kelvin as i32;
            if ins_slider_i32(ui, Some("Temperature"), &mut wb_val, 2000, 10000, "K", true, theme) {
                state.wb_kelvin = wb_val as u16;
                let _ = ctrl.set_white_balance_temperature(state.wb_kelvin);
            }
        }

        ui.add_space(6.0_f32);

        // More Color Section (Link 1 always shows; Link 2/2C collapsible)
        let show_more = ctrl.model() == CameraModel::Link || state.open_more_color;
        if show_more {
            let mut br_val = state.brightness as i32;
            if ins_slider_i32(ui, Some("Brightness"), &mut br_val, 0, 100, "%", true, theme) {
                state.brightness = br_val as i16;
                let _ = ctrl.set_brightness(state.brightness);
            }

            let mut ct_val = state.contrast as i32;
            if ins_slider_i32(ui, Some("Contrast"), &mut ct_val, 0, 100, "%", true, theme) {
                state.contrast = ct_val as u16;
                let _ = ctrl.set_contrast(state.contrast);
            }

            let mut sat_val = state.saturation as i32;
            if ins_slider_i32(ui, Some("Saturation"), &mut sat_val, 0, 100, "%", true, theme) {
                state.saturation = sat_val as u16;
                let _ = ctrl.set_saturation(state.saturation);
            }

            let mut sh_val = state.sharpness as i32;
            if ins_slider_i32(ui, Some("Sharpness"), &mut sh_val, 0, 100, "%", true, theme) {
                state.sharpness = sh_val as u16;
                let _ = ctrl.set_sharpness(state.sharpness);
            }

            if ins_switch_row(
                ui,
                "HDR",
                Some("Currently not supported at 4K 50/60fps"),
                &mut state.hdr_enabled,
                true,
                theme,
            ) {
                let _ = ctrl.set_hdr(state.hdr_enabled);
                if let Ok(ae) = ctrl.get_auto_exposure() {
                    state.auto_exposure = ae;
                }
                if state.auto_exposure {
                    if let Ok(ev) = ctrl.get_exposure_compensation() {
                        state.ev_compensation = ev;
                    }
                } else {
                    if let Ok(iso) = ctrl.get_iso() {
                        if let Some(idx) = ISO_LIST.iter().position(|&v| v == iso) {
                            state.iso_index = idx;
                        }
                    }
                    if let Ok(shutter) = ctrl.get_shutter_speed() {
                        if let Some(idx) = SHUTTER_DENOM_LIST.iter().position(|&v| v == shutter) {
                            state.shutter_index = idx;
                        }
                    }
                }
            }

            // Anti-Flicker combo
            ui.horizontal(|ui| {
                ui.label(RichText::new("Anti-Flicker").size(13.0_f32).color(theme.text01));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let cur_label = match state.anti_flicker {
                        PowerLineFrequency::Auto => "Auto",
                        PowerLineFrequency::Hz50 => "50Hz",
                        PowerLineFrequency::Hz60 => "60Hz",
                        PowerLineFrequency::Disabled => "Off",
                    };
                    egui::ComboBox::from_id_salt("anti_flicker_combo")
                        .selected_text(cur_label)
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut state.anti_flicker, PowerLineFrequency::Auto, "Auto").clicked() {
                                let _ = ctrl.set_power_line_frequency(PowerLineFrequency::Auto);
                            }
                            if ui.selectable_value(&mut state.anti_flicker, PowerLineFrequency::Hz50, "50Hz").clicked() {
                                let _ = ctrl.set_power_line_frequency(PowerLineFrequency::Hz50);
                            }
                            if ui.selectable_value(&mut state.anti_flicker, PowerLineFrequency::Hz60, "60Hz").clicked() {
                                let _ = ctrl.set_power_line_frequency(PowerLineFrequency::Hz60);
                            }
                        });
                });
            });

            if ctrl.model() == CameraModel::Link {
                if ins_switch_row(
                    ui,
                    "Save Startup Parameters",
                    None,
                    &mut state.startup_params_enabled,
                    true,
                    theme,
                ) {
                    let _ = ctrl.set_startup_params_enabled(state.startup_params_enabled);
                }
            }
        }

        if ctrl.model() != CameraModel::Link {
            ui.add_space(4.0_f32);
            ui.vertical_centered(|ui| {
                let label = if state.open_more_color {
                    "Collapse ▴"
                } else {
                    "Show ▾"
                };
                if ui.button(RichText::new(label).size(12.0_f32).color(theme.text02)).clicked() {
                    state.open_more_color = !state.open_more_color;
                }
            });
        }

        // Color Presets Grid (`right_img_color_preset`)
        if ctrl.capabilities().support_image_template {
            ui.add_space(8.0_f32);
            ui.separator();
            ui.add_space(6.0_f32);
            render_color_presets(ui, ctx, state, ctrl, video, theme);
        }
    });

    // ------------------------------------------------------------------------
    // Card 2: `Background` (`ImageBackground.qml`) (Link 2 / Link 2C only)
    // ------------------------------------------------------------------------
    if ctrl.model() != CameraModel::Link {
        ui.add_space(10.0_f32);
        ins_card(ui, theme, |ui| {
            ins_section_header(ui, "Background", theme, None);
            ui.add_space(6.0_f32);

            let modes = [
                (BackgroundEffectMode::None, "⊘", "None"),
                (BackgroundEffectMode::StandardBlur, "💧", "Blur"),
                (BackgroundEffectMode::NaturalBokeh, "✨", "Bokeh"),
                (BackgroundEffectMode::CustomImage(0), "🖼", "Office"),
            ];

            ui.horizontal(|ui| {
                for (mode, icon, label) in modes {
                    let is_active = state.background_mode == mode;
                    let (rect, response) = ui.allocate_exact_size(Vec2::splat(52.0_f32), Sense::click());
                    let stroke = if is_active {
                        Stroke::new(2.0_f32, theme.brand_green)
                    } else if response.hovered() {
                        Stroke::new(1.0_f32, theme.text02)
                    } else {
                        Stroke::new(1.0_f32, theme.border02)
                    };
                    ui.painter().rect_filled(rect, Rounding::same(8.0_f32), theme.bg03);
                    ui.painter().rect_stroke(rect, Rounding::same(8.0_f32), stroke);

                    ui.painter().text(
                        Pos2::new(rect.center().x, rect.center().y - 8.0_f32),
                        Align2::CENTER_CENTER,
                        icon,
                        FontId::proportional(18.0_f32),
                        if is_active { theme.brand_green } else { theme.text01 },
                    );
                    ui.painter().text(
                        Pos2::new(rect.center().x, rect.center().y + 14.0_f32),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(10.5_f32),
                        if is_active { theme.brand_green } else { theme.text02 },
                    );

                    if response.clicked() {
                        state.background_mode = mode;
                    }
                }
            });

            if matches!(
                state.background_mode,
                BackgroundEffectMode::StandardBlur | BackgroundEffectMode::NaturalBokeh
            ) {
                ui.add_space(8.0_f32);
                let mut blur_val = state.blur_intensity as i32;
                if ins_slider_i32(ui, Some("Intensity"), &mut blur_val, 0, 100, "%", true, theme) {
                    state.blur_intensity = blur_val as u8;
                }
            }
        });
    }

    // ------------------------------------------------------------------------
    // Card 3: `Makeup` (`ImageBeauty.qml`)
    // ------------------------------------------------------------------------
    ui.add_space(10.0_f32);
    ins_card(ui, theme, |ui| {
        if ins_switch_row(ui, "Makeup & Retouch", None, &mut state.makeup_enabled, true, theme) {
            // Toggled
        }
        if state.makeup_enabled {
            ui.add_space(6.0_f32);
            let mut makeup_val = state.makeup_intensity as i32;
            if ins_slider_i32(ui, Some("Intensity"), &mut makeup_val, 0, 100, "%", true, theme) {
                state.makeup_intensity = makeup_val as u8;
            }
        }
    });

    // ------------------------------------------------------------------------
    // Card 4: `Filter` (`ImageFilter.qml`)
    // ------------------------------------------------------------------------
    ui.add_space(10.0_f32);
    ins_card(ui, theme, |ui| {
        ins_section_header(ui, "Filter", theme, None);
        ui.add_space(6.0_f32);

        const FILTERS: [(FilterMode, &str); 9] = [
            (FilterMode::None, "None"),
            (FilterMode::Style1, "Neon"),
            (FilterMode::Style2, "Vintage1"),
            (FilterMode::Amber, "Vintage2"),
            (FilterMode::Film1, "Daylight"),
            (FilterMode::Fresh, "Clear"),
            (FilterMode::Film2, "Portrait"),
            (FilterMode::Cool, "Cool"),
            (FilterMode::Warm, "Warm"),
        ];

        Grid::new("filter_tiles_grid")
            .spacing([8.0_f32, 8.0_f32])
            .show(ui, |ui| {
                let mut col = 0;
                for (mode, label) in FILTERS {
                    let is_active = state.filter_mode == mode;
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(52.0_f32, 68.0_f32), Sense::click());
                    let thumb_rect = Rect::from_min_size(rect.min, Vec2::new(52.0_f32, 50.0_f32));

                    let stroke = if is_active {
                        Stroke::new(2.0_f32, theme.brand_green)
                    } else if response.hovered() {
                        Stroke::new(1.0_f32, theme.text02)
                    } else {
                        Stroke::new(1.0_f32, theme.border02)
                    };

                    let (c1, c2) = match mode {
                        FilterMode::None => (
                            Color32::from_rgb(80, 80, 80),
                            Color32::from_rgb(60, 60, 60),
                        ),
                        FilterMode::Style1 => (
                            Color32::from_rgb(255, 60, 180),
                            Color32::from_rgb(0, 220, 255),
                        ),
                        FilterMode::Style2 => (
                            Color32::from_rgb(210, 160, 100),
                            Color32::from_rgb(140, 90, 50),
                        ),
                        FilterMode::Amber => (
                            Color32::from_rgb(255, 170, 40),
                            Color32::from_rgb(180, 90, 20),
                        ),
                        FilterMode::Film1 => (
                            Color32::from_rgb(255, 230, 150),
                            Color32::from_rgb(100, 180, 240),
                        ),
                        FilterMode::Fresh => (
                            Color32::from_rgb(130, 230, 200),
                            Color32::from_rgb(70, 160, 200),
                        ),
                        FilterMode::Film2 => (
                            Color32::from_rgb(255, 190, 180),
                            Color32::from_rgb(200, 130, 150),
                        ),
                        FilterMode::Cool => (
                            Color32::from_rgb(80, 160, 255),
                            Color32::from_rgb(40, 80, 180),
                        ),
                        FilterMode::Warm => (
                            Color32::from_rgb(255, 140, 60),
                            Color32::from_rgb(200, 70, 30),
                        ),
                    };

                    ui.painter().rect_filled(thumb_rect, Rounding::same(6.0_f32), c1);
                    let sub_rect = Rect::from_min_max(
                        Pos2::new(thumb_rect.left(), thumb_rect.center().y),
                        thumb_rect.max,
                    );
                    ui.painter().rect_filled(sub_rect, Rounding::same(6.0_f32), c2);
                    ui.painter().rect_stroke(thumb_rect, Rounding::same(6.0_f32), stroke);

                    ui.painter().text(
                        Pos2::new(rect.center().x, rect.max.y - 8.0_f32),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(10.5_f32),
                        if is_active {
                            theme.brand_green
                        } else {
                            theme.text02
                        },
                    );

                    if response.clicked() {
                        state.filter_mode = mode;
                        let _ = ctrl.set_filter(mode);
                    }

                    col += 1;
                    if col % 4 == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}

// ============================================================================
// Tab 3: `RightSidebarTab::More` (`MoreSettings.qml`)
// ============================================================================
fn render_more_tab<T: UvcControlTransport>(
    ui: &mut Ui,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    theme: &InsTheme,
) {
    // ------------------------------------------------------------------------
    // Card 1: `Gesture`
    // ------------------------------------------------------------------------
    ins_card(ui, theme, |ui| {
        if ins_switch_row(ui, "Gesture", None, &mut state.enable_all_gestures, true, theme) {
            let _ = ctrl.set_all_gestures_enabled(state.enable_all_gestures);
        }

        ui.add_space(4.0_f32);

        let tracking_label = if ctrl.capabilities().support_ptz_drag {
            "AI Tracking"
        } else {
            "Auto Framing"
        };
        if ins_checkbox_row(
            ui,
            tracking_label,
            Some("✋"),
            None,
            &mut state.use_gesture_palm,
            state.enable_all_gestures,
            theme,
        ) {
            let _ = ctrl.set_gesture_enabled(Gesture::Palm, state.use_gesture_palm);
        }

        if ins_checkbox_row(
            ui,
            "Whiteboard",
            Some("✌"),
            None,
            &mut state.use_gesture_v,
            state.enable_all_gestures,
            theme,
        ) {
            let _ = ctrl.set_gesture_enabled(Gesture::VShape, state.use_gesture_v);
        }

        if ins_checkbox_row(
            ui,
            "Zoom",
            Some("👆"),
            None,
            &mut state.use_gesture_l,
            state.enable_all_gestures,
            theme,
        ) {
            let _ = ctrl.set_gesture_enabled(Gesture::LShape, state.use_gesture_l);
        }
    });

    ui.add_space(10.0_f32);

    // ------------------------------------------------------------------------
    // Card 2: `Audio Mode`
    // ------------------------------------------------------------------------
    ins_card(ui, theme, |ui| {
        ins_section_header(ui, "Audio Mode", theme, None);
        ui.add_space(6.0_f32);

        if ctrl.capabilities().support_audio_capture_mode {
            let mut modes = vec![
                (AudioCaptureMode::VoiceFocus, "Voice Focus"),
                (AudioCaptureMode::VoiceSuppression, "Voice Suppression"),
                (AudioCaptureMode::MusicBalance, "Music Balance"),
            ];
            if ctrl.capabilities().support_sound_wall {
                modes.push((AudioCaptureMode::SoundWall, "Sound Wall"));
            }

            for (mode, label) in modes {
                let is_selected = state.audio_capture_mode == mode;
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0_f32), Sense::click());

                ui.painter().text(
                    Pos2::new(rect.left(), rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(13.0_f32),
                    if is_selected { theme.text01 } else { theme.text02 },
                );

                let radio_center = Pos2::new(rect.right() - 10.0_f32, rect.center().y);
                if is_selected {
                    ui.painter().circle_stroke(radio_center, 7.0_f32, Stroke::new(1.5_f32, theme.brand_green));
                    ui.painter().circle_filled(radio_center, 4.0_f32, theme.brand_green);
                } else {
                    ui.painter().circle_stroke(radio_center, 7.0_f32, Stroke::new(1.0_f32, theme.border02));
                }

                if response.clicked() {
                    state.audio_capture_mode = mode;
                    let _ = ctrl.set_audio_capture_mode(mode);
                }
            }
        } else if ins_switch_row(
            ui,
            "AI Audio Noise Reduction",
            None,
            &mut state.audio_denoise,
            true,
            theme,
        ) {
            let _ = ctrl.set_audio_noise_reduction(state.audio_denoise);
        }
    });

    ui.add_space(10.0_f32);

    // ------------------------------------------------------------------------
    // Card 3: `Other Settings`
    // ------------------------------------------------------------------------
    ins_card(ui, theme, |ui| {
        ins_section_header(ui, "Other Settings", theme, None);
        ui.add_space(6.0_f32);

        if ins_checkbox_row(ui, "Horizontal Flip", None, None, &mut state.horizontal_mirror, true, theme) {
            let _ = ctrl.set_horizontal_mirror(state.horizontal_mirror);
        }

        if ctrl.capabilities().support_mirror_vertically {
            if ins_checkbox_row(ui, "Vertical Flip", None, None, &mut state.vertical_mirror, true, theme) {
                let _ = ctrl.set_vertical_mirror(state.vertical_mirror);
            }
        }

        if ctrl.capabilities().support_smart_adjustment {
            if ins_checkbox_row(
                ui,
                "Smart Adjustment",
                None,
                Some("Automatically search for your head within a small range and adjust to the optimal angle."),
                &mut state.smart_adjustment,
                true,
                theme,
            ) {
                let _ = ctrl.set_smart_adjustment(state.smart_adjustment);
            }
        }

        if ctrl.capabilities().support_horizontal_correction {
            let enabled = !state.format_preset.is_4k();
            if ins_checkbox_row(
                ui,
                "Horizontal Correction",
                None,
                Some("Keep the horizon level automatically."),
                &mut state.horizontal_correction,
                enabled,
                theme,
            ) {
                let _ = ctrl.set_horizontal_correction(state.horizontal_correction);
            }
        }

        if ctrl.capabilities().support_roll_adjustment {
            if ins_slider_f32(
                ui,
                Some("Horizontal fine-tuning"),
                &mut state.roll_deg,
                -1.0_f32,
                1.0_f32,
                0.1_f32,
                1,
                "°",
                true,
                theme,
            ) {
                let _ = ctrl.set_roll((state.roll_deg * 10.0_f32) as i16);
            }
        }
    });

    ui.add_space(10.0_f32);

    // ------------------------------------------------------------------------
    // Card 4: `Compatibility Settings`
    // ------------------------------------------------------------------------
    ins_card(ui, theme, |ui| {
        ins_section_header(ui, "Compatibility Settings", theme, None);
        ui.add_space(6.0_f32);

        if ins_checkbox_row(
            ui,
            "Portrait Resolution and High Frame Rate",
            None,
            Some("Unlock 60fps high frame rate and 9:16 portrait resolution in UVC."),
            &mut state.vertical_resolution_mode,
            !state.is_recording,
            theme,
        ) {
            let _ = ctrl.set_vertical_screen(state.vertical_resolution_mode);
            if !state.vertical_resolution_mode
                && (state.format_preset.is_vertical() || state.format_preset.fps() > 30)
            {
                state.format_preset = crate::state::VideoFormatPreset::Fhd1080p30;
            }
        }

        if state.vertical_resolution_mode && ctrl.model() == CameraModel::Link {
            if ins_checkbox_row(
                ui,
                "Lock vertical shot",
                None,
                Some("Lock gimbal at 90° in vertical shooting mode."),
                &mut state.forced_vertical_lock,
                true,
                theme,
            ) {
                let _ = ctrl.set_forced_vertical(state.forced_vertical_lock);
            }
        }

        if ctrl.model() != CameraModel::Link {
            if ins_checkbox_row(
                ui,
                "Enable Low Resolution",
                None,
                Some("Allow 360p resolution selection."),
                &mut state.low_resolution_mode,
                !state.is_recording,
                theme,
            ) {
                let _ = ctrl.set_low_resolution(state.low_resolution_mode);
                if state.low_resolution_mode {
                    state.format_preset = crate::state::VideoFormatPreset::Sd360p30;
                } else {
                    state.format_preset = crate::state::VideoFormatPreset::Fhd1080p30;
                }
            }
        }
    });

    ui.add_space(10.0_f32);

    // ------------------------------------------------------------------------
    // Card 5: `Device Info & Factory Reset`
    // ------------------------------------------------------------------------
    ins_card(ui, theme, |ui| {
        ins_section_header(ui, "Device Info", theme, None);
        ui.add_space(6.0_f32);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Firmware version").size(12.0_f32).color(theme.text02));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(&state.device_info.firmware_version).size(12.0_f32).color(theme.text01));
            });
        });
        ui.add_space(3.0_f32);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Serial number").size(12.0_f32).color(theme.text02));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(&state.device_info.serial_number).size(12.0_f32).color(theme.text01));
            });
        });

        ui.add_space(10.0_f32);
        ui.vertical_centered_justified(|ui| {
            if ui.button(RichText::new("Factory reset").size(13.0_f32).color(theme.text02)).clicked() {
                state.show_factory_reset_pop = true;
            }
        });
    });
}

// ============================================================================
// Specialized Widgets
// ============================================================================

/// 128×128 Circular Gimbal Joystick / D-Pad (`DefControlPanel.qml`).
fn render_joystick_pad<T: UvcControlTransport>(
    ui: &mut Ui,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    theme: &InsTheme,
) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(128.0_f32), Sense::click_and_drag());
    let center = rect.center();
    let painter = ui.painter();

    // Outer circle
    painter.circle_filled(center, 63.0_f32, theme.bg03);
    painter.circle_stroke(center, 63.0_f32, Stroke::new(1.0_f32, theme.border02));

    // Inner ring
    painter.circle_stroke(center, 38.0_f32, Stroke::new(1.0_f32, theme.border02));

    // Directional chevrons
    let chev_font = FontId::proportional(12.0_f32);
    painter.text(Pos2::new(center.x, center.y - 48.0_f32), Align2::CENTER_CENTER, "▲", chev_font.clone(), theme.text03);
    painter.text(Pos2::new(center.x, center.y + 48.0_f32), Align2::CENTER_CENTER, "▼", chev_font.clone(), theme.text03);
    painter.text(Pos2::new(center.x - 48.0_f32, center.y), Align2::CENTER_CENTER, "◀", chev_font.clone(), theme.text03);
    painter.text(Pos2::new(center.x + 48.0_f32, center.y), Align2::CENTER_CENTER, "▶", chev_font, theme.text03);

    // Draggable Center Knob with continuous velocity (`DefControlPanel.qml` + `CameraInsta::movePanTilt`)
    let mut knob_pos = center;
    let is_holding = response.dragged() || response.is_pointer_button_down_on();

    if is_holding {
        state.joystick_dragging = true;
        if let Some(ptr) = response.interact_pointer_pos() {
            let diff = ptr - center;
            let offset = if diff.length() > 38.0_f32 {
                diff.normalized() * 38.0_f32
            } else {
                diff
            };
            knob_pos = center + offset;

            let deadzone = 8.0_f32;
            let (pan_speed, pan_dir) = if diff.x.abs() > deadzone {
                let norm = ((diff.x.abs() - deadzone) / 30.0_f32).clamp(0.0_f32, 1.0_f32);
                let spd = ((norm * 6.0_f32 + 1.0_f32).round() as u8).clamp(1, 7);
                let dir = if diff.x > 0.0_f32 { 1i8 } else { -1i8 };
                (spd, dir)
            } else {
                (0u8, 0i8)
            };

            let (tilt_speed, tilt_dir) = if diff.y.abs() > deadzone {
                let norm = ((diff.y.abs() - deadzone) / 30.0_f32).clamp(0.0_f32, 1.0_f32);
                let spd = ((norm * 6.0_f32 + 1.0_f32).round() as u8).clamp(1, 7);
                // Screen Y < 0 (up) -> tilt_dir = -1 (0xff); Screen Y > 0 (down) -> tilt_dir = 1 (0x01)
                let dir = if diff.y > 0.0_f32 { 1i8 } else { -1i8 };
                (spd, dir)
            } else {
                (0u8, 0i8)
            };

            // Update visual preview angles smoothly for simulator
            state.pan_deg = (state.pan_deg + (pan_dir as f32) * (pan_speed as f32) * 0.12_f32).clamp(-130.0_f32, 130.0_f32);
            state.tilt_deg = (state.tilt_deg - (tilt_dir as f32) * (tilt_speed as f32) * 0.12_f32).clamp(-90.0_f32, 90.0_f32);

            let cmd = (pan_speed, pan_dir, tilt_speed, tilt_dir);
            let can_send = state
                .last_ptz_send_time
                .map(|t| t.elapsed().as_millis() >= 70)
                .unwrap_or(true);
            if cmd != state.last_ptz_cmd && (can_send || cmd == (0, 0, 0, 0)) {
                state.nudge_stop_at = None;
                let _ = ctrl.set_pantilt_relative_ext(pan_speed, pan_dir, tilt_speed, tilt_dir);
                state.last_ptz_cmd = cmd;
                state.last_ptz_send_time = Some(std::time::Instant::now());
            }
        }
    } else if response.clicked() {
        // Short click on one of the 4 directional sectors (`DefControlPanel.qml` `addPanTilt`)
        if let Some(ptr) = response.interact_pointer_pos() {
            let diff = ptr - center;
            if diff.length() > 10.0_f32 {
                let (pan_speed, pan_dir, tilt_speed, tilt_dir) = if diff.x.abs() >= diff.y.abs() {
                    let dir = if diff.x > 0.0_f32 { 1i8 } else { -1i8 };
                    state.pan_deg = (state.pan_deg + (dir as f32) * 1.5_f32).clamp(-130.0_f32, 130.0_f32);
                    (4u8, dir, 0u8, 0i8)
                } else {
                    let dir = if diff.y > 0.0_f32 { 1i8 } else { -1i8 };
                    state.tilt_deg = (state.tilt_deg - (dir as f32) * 1.5_f32).clamp(-90.0_f32, 90.0_f32);
                    (0u8, 0i8, 4u8, dir)
                };
                let _ = ctrl.set_pantilt_relative_ext(pan_speed, pan_dir, tilt_speed, tilt_dir);
                state.last_ptz_cmd = (pan_speed, pan_dir, tilt_speed, tilt_dir);
                state.nudge_stop_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_millis(120));
            }
        }
    } else if state.joystick_dragging {
        // Released drag: immediately stop gimbal movement (`[0, 1, 0, 1]`)
        state.joystick_dragging = false;
        let _ = ctrl.stop_pantilt_relative();
        state.last_ptz_cmd = (0, 0, 0, 0);
        if let Ok((pan_arc, tilt_arc)) = ctrl.get_pantilt_absolute() {
            state.pan_deg = (pan_arc as f32 / 3600.0_f32).clamp(-130.0_f32, 130.0_f32);
            state.tilt_deg = (tilt_arc as f32 / 3600.0_f32).clamp(-90.0_f32, 90.0_f32);
        }
    }

    let knob_stroke = if response.dragged() || response.hovered() {
        Stroke::new(1.5_f32, theme.brand_green)
    } else {
        Stroke::new(1.0_f32, theme.border02)
    };
    painter.circle_filled(knob_pos, 20.0_f32, theme.bg04);
    painter.circle_stroke(knob_pos, 20.0_f32, knob_stroke);
    painter.circle_filled(knob_pos, 3.0_f32, if response.dragged() { theme.brand_green } else { theme.text03 });
}

/// 168×128 Viewport Crop Box (`bottomPic` / `selectPic`) for Link 2C fixed lens.
fn render_viewport_crop_box<T: UvcControlTransport>(
    ui: &mut Ui,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    theme: &InsTheme,
) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(168.0_f32, 128.0_f32), Sense::click_and_drag());
    let painter = ui.painter();

    // Outer frame
    painter.rect_filled(rect, Rounding::same(6.0_f32), theme.bg03);
    painter.rect_stroke(rect, Rounding::same(6.0_f32), Stroke::new(1.0_f32, theme.border02));

    if state.zoom_x <= 1.01_f32 {
        painter.text(
            Pos2::new(rect.center().x, rect.center().y - 10.0_f32),
            Align2::CENTER_CENTER,
            "⛶",
            FontId::proportional(22.0_f32),
            theme.text03,
        );
        painter.text(
            Pos2::new(rect.center().x, rect.center().y + 12.0_f32),
            Align2::CENTER_CENTER,
            "Displayed maximum range",
            FontId::proportional(11.0_f32),
            theme.text03,
        );
    } else {
        let crop_w = rect.width() / state.zoom_x;
        let crop_h = rect.height() / state.zoom_x;
        let min_cx = rect.left() + crop_w * 0.5_f32;
        let max_cx = rect.right() - crop_w * 0.5_f32;
        let min_cy = rect.top() + crop_h * 0.5_f32;
        let max_cy = rect.bottom() - crop_h * 0.5_f32;

        if response.dragged() {
            let delta = response.drag_delta();
            let span_x = (max_cx - min_cx).max(1.0_f32);
            let span_y = (max_cy - min_cy).max(1.0_f32);
            state.host_ptz_x = (state.host_ptz_x + delta.x / span_x).clamp(0.0_f32, 1.0_f32);
            state.host_ptz_y = (state.host_ptz_y + delta.y / span_y).clamp(0.0_f32, 1.0_f32);

            let _ = ctrl.set_host_ptz(&HostPtzInfo {
                x: (state.host_ptz_x * 5140.0_f32) as u16,
                y: (state.host_ptz_y * 5140.0_f32) as u16,
                width: (5140.0_f32 / state.zoom_x) as u16,
                height: (5140.0_f32 / state.zoom_x) as u16,
            });
        }

        let cx = min_cx + state.host_ptz_x * (max_cx - min_cx);
        let cy = min_cy + state.host_ptz_y * (max_cy - min_cy);
        let crop_rect = Rect::from_center_size(Pos2::new(cx, cy), Vec2::new(crop_w, crop_h));

        painter.rect_filled(
            crop_rect,
            Rounding::same(4.0_f32),
            Color32::from_rgba_premultiplied(0, 208, 120, 25),
        );
        painter.rect_stroke(
            crop_rect,
            Rounding::same(4.0_f32),
            Stroke::new(1.5_f32, theme.brand_green),
        );
    }
}

/// Vertical Zoom Rocker (`DefAdjustment.qml`).
fn render_zoom_rocker<T: UvcControlTransport>(
    ui: &mut Ui,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    theme: &InsTheme,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(52.0_f32, 124.0_f32), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(8.0_f32), theme.bg03);
    painter.rect_stroke(rect, Rounding::same(8.0_f32), Stroke::new(1.0_f32, theme.border02));

    let mut rocker_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    rocker_ui.vertical_centered(|ui| {
        ui.add_space(2.0_f32);
        // [ + ]
        if ui
            .add_sized(
                Vec2::new(48.0_f32, 34.0_f32),
                egui::Button::new(RichText::new("+").size(18.0_f32).strong().color(theme.text01)).frame(false),
            )
            .clicked()
        {
            state.zoom_x = (state.zoom_x + 0.1_f32).min(4.0_f32);
            let _ = ctrl.set_zoom((state.zoom_x * 100.0_f32) as u16);
        }

        ui.add_space(8.0_f32);

        // Center zoom label
        let zoom_color = if state.zoom_x > 1.01_f32 {
            theme.brand_green
        } else {
            theme.text01
        };
        ui.label(RichText::new(format!("{:.1}x", state.zoom_x)).size(13.0_f32).strong().color(zoom_color));

        ui.add_space(8.0_f32);

        // [ - ]
        if ui
            .add_sized(
                Vec2::new(48.0_f32, 34.0_f32),
                egui::Button::new(RichText::new("−").size(18.0_f32).strong().color(theme.text01)).frame(false),
            )
            .clicked()
        {
            state.zoom_x = (state.zoom_x - 0.1_f32).max(1.0_f32);
            let _ = ctrl.set_zoom((state.zoom_x * 100.0_f32) as u16);
        }
    });
}

/// PTZ Presets Grid (`gimbal_preset`).
fn render_ptz_presets<T: UvcControlTransport>(
    ui: &mut Ui,
    ctx: &Context,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    video: &VideoPreviewEngine,
    theme: &InsTheme,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Presets").size(12.0_f32).strong().color(theme.text03))
            .on_hover_text("Presets will remember the position and mode you added");
    });

    ui.add_space(6.0_f32);

    Grid::new("ptz_presets_grid")
        .spacing([8.0_f32, 8.0_f32])
        .show(ui, |ui| {
            let mut col = 0;

            // First Cell: [ + ] Add Preset tile
            let (add_rect, add_resp) = ui.allocate_exact_size(Vec2::new(52.0_f32, 52.0_f32), Sense::click());
            let add_stroke = if add_resp.hovered() {
                Stroke::new(1.0_f32, theme.brand_green)
            } else {
                Stroke::new(1.0_f32, theme.border02)
            };
            ui.painter().rect_filled(add_rect, Rounding::same(8.0_f32), theme.overlay02);
            ui.painter().rect_stroke(add_rect, Rounding::same(8.0_f32), add_stroke);
            ui.painter().text(
                add_rect.center(),
                Align2::CENTER_CENTER,
                "+",
                FontId::proportional(22.0_f32),
                theme.text01,
            );

            if add_resp.clicked() {
                let next_id = state
                    .ptz_presets
                    .iter()
                    .position(|s| !s.saved)
                    .map(|idx| state.ptz_presets[idx].id)
                    .unwrap_or_else(|| state.ptz_presets.len() as u8);

                if next_id < 10 {
                    if let Ok((pan_arc, tilt_arc)) = ctrl.get_pantilt_absolute() {
                        state.pan_deg = (pan_arc as f32 / 3600.0_f32).clamp(-130.0_f32, 130.0_f32);
                        state.tilt_deg = (tilt_arc as f32 / 3600.0_f32).clamp(-90.0_f32, 90.0_f32);
                    }
                    if let Ok(z) = ctrl.get_zoom() {
                        state.zoom_x = (z as f32 / 100.0_f32).clamp(1.0_f32, 4.0_f32);
                    }
                    let img = video.capture_thumbnail();
                    let tex = ctx.load_texture(
                        format!("ptz_preset_{}", next_id),
                        img.clone(),
                        egui::TextureOptions::LINEAR,
                    );

                    if let Some(slot) = state.ptz_presets.iter_mut().find(|s| s.id == next_id) {
                        slot.saved = true;
                        slot.pan_deg = state.pan_deg;
                        slot.tilt_deg = state.tilt_deg;
                        slot.zoom_x = state.zoom_x;
                        slot.host_x = state.host_ptz_x;
                        slot.host_y = state.host_ptz_y;
                        slot.mode = state.video_mode;
                        slot.thumbnail = Some(tex);
                        slot.raw_thumb = Some(img);
                        slot.renaming = false;
                        state.active_preset_id = Some(slot.id);
                    } else {
                        let slot = PtzPresetSlot {
                            id: next_id,
                            name: format!("Preset {}", next_id + 1),
                            saved: true,
                            pan_deg: state.pan_deg,
                            tilt_deg: state.tilt_deg,
                            zoom_x: state.zoom_x,
                            host_x: state.host_ptz_x,
                            host_y: state.host_ptz_y,
                            mode: state.video_mode,
                            thumbnail: Some(tex),
                            raw_thumb: Some(img),
                            renaming: false,
                        };
                        state.ptz_presets.push(slot);
                        state.active_preset_id = Some(next_id);
                    }
                }
            }

            col += 1;

            // Saved Preset Cells
            let mut recall_id = None;
            let mut update_id = None;
            let mut delete_id = None;

            for slot in state.ptz_presets.iter_mut().filter(|s| s.saved) {
                let is_active = state.active_preset_id == Some(slot.id);
                let (tile_rect, tile_resp) = ui.allocate_exact_size(Vec2::new(52.0_f32, 68.0_f32), Sense::click());
                let box_rect = Rect::from_min_size(tile_rect.min, Vec2::new(52.0_f32, 52.0_f32));

                // Thumbnail texture upload if missing
                if slot.thumbnail.is_none() {
                    if let Some(raw) = &slot.raw_thumb {
                        slot.thumbnail = Some(ctx.load_texture(
                            format!("ptz_preset_{}", slot.id),
                            raw.clone(),
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }

                if let Some(tex) = &slot.thumbnail {
                    ui.painter().image(
                        tex.id(),
                        box_rect,
                        Rect::from_min_max(Pos2::new(0.0_f32, 0.0_f32), Pos2::new(1.0_f32, 1.0_f32)),
                        Color32::WHITE,
                    );
                } else {
                    ui.painter().rect_filled(box_rect, Rounding::same(8.0_f32), theme.bg03);
                    ui.painter().text(
                        box_rect.center(),
                        Align2::CENTER_CENTER,
                        "📷",
                        FontId::proportional(18.0_f32),
                        theme.text03,
                    );
                }

                let border_stroke = if is_active {
                    Stroke::new(2.0_f32, theme.brand_green)
                } else if tile_resp.hovered() {
                    Stroke::new(1.0_f32, theme.text02)
                } else {
                    Stroke::new(1.0_f32, theme.border02)
                };
                ui.painter().rect_stroke(box_rect, Rounding::same(8.0_f32), border_stroke);

                // Label / rename below
                if slot.renaming {
                    let mut name_ui = ui.new_child(
                        egui::UiBuilder::new().max_rect(Rect::from_min_size(
                            Pos2::new(tile_rect.left(), box_rect.bottom() + 2.0_f32),
                            Vec2::new(52.0_f32, 14.0_f32),
                        )),
                    );
                    let resp = name_ui.add(TextEdit::singleline(&mut slot.name).font(FontId::proportional(10.0_f32)));
                    if resp.lost_focus() || name_ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        slot.renaming = false;
                    }
                } else {
                    ui.painter().text(
                        Pos2::new(tile_rect.center().x, box_rect.bottom() + 8.0_f32),
                        Align2::CENTER_CENTER,
                        &slot.name,
                        FontId::proportional(11.0_f32),
                        if is_active { theme.brand_green } else { theme.text02 },
                    );
                }

                if tile_resp.clicked() {
                    recall_id = Some(slot.id);
                }

                tile_resp.context_menu(|ui| {
                    if ui.button("Update with Current View").clicked() {
                        update_id = Some(slot.id);
                        ui.close_menu();
                    }
                    if ui.button("Rename").clicked() {
                        slot.renaming = true;
                        ui.close_menu();
                    }
                    if ui.button("Delete").clicked() {
                        delete_id = Some(slot.id);
                        ui.close_menu();
                    }
                });

                col += 1;
                if col % 4 == 0 {
                    ui.end_row();
                }
            }

            if let Some(id) = recall_id {
                if let Some(slot) = state.ptz_presets.iter().find(|s| s.id == id).cloned() {
                    state.pan_deg = slot.pan_deg;
                    state.tilt_deg = slot.tilt_deg;
                    state.zoom_x = slot.zoom_x;
                    state.video_mode = slot.mode;
                    state.active_preset_id = Some(slot.id);
                    state.ready_auto_track = false;
                    let _ = ctrl.set_pantilt_absolute(
                        (slot.pan_deg * 3600.0_f32) as i32,
                        (slot.tilt_deg * 3600.0_f32) as i32,
                    );
                    let _ = ctrl.set_zoom((slot.zoom_x * 100.0_f32) as u16);
                }
            }

            if let Some(id) = update_id {
                if let Ok((pan_arc, tilt_arc)) = ctrl.get_pantilt_absolute() {
                    state.pan_deg = (pan_arc as f32 / 3600.0_f32).clamp(-130.0_f32, 130.0_f32);
                    state.tilt_deg = (tilt_arc as f32 / 3600.0_f32).clamp(-90.0_f32, 90.0_f32);
                }
                if let Ok(z) = ctrl.get_zoom() {
                    state.zoom_x = (z as f32 / 100.0_f32).clamp(1.0_f32, 4.0_f32);
                }
                if let Some(slot) = state.ptz_presets.iter_mut().find(|s| s.id == id) {
                    slot.pan_deg = state.pan_deg;
                    slot.tilt_deg = state.tilt_deg;
                    slot.zoom_x = state.zoom_x;
                    slot.host_x = state.host_ptz_x;
                    slot.host_y = state.host_ptz_y;
                    slot.mode = state.video_mode;
                    let img = video.capture_thumbnail();
                    let tex = ctx.load_texture(
                        format!("ptz_preset_{}", slot.id),
                        img.clone(),
                        egui::TextureOptions::LINEAR,
                    );
                    slot.thumbnail = Some(tex);
                    slot.raw_thumb = Some(img);
                }
            }

            if let Some(id) = delete_id {
                if let Some(slot) = state.ptz_presets.iter_mut().find(|s| s.id == id) {
                    slot.saved = false;
                }
            }
        });
}

/// 3 Visual Composition Cards (`72.0 x 68.0` each).
fn render_composition_cards<T: UvcControlTransport>(
    ui: &mut Ui,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    theme: &InsTheme,
) {
    let cards = [
        (CompositionStyle::Head, "Head"),
        (CompositionStyle::HalfBody, "Half Body"),
        (CompositionStyle::WholeBody, "Whole Body"),
    ];

    ui.horizontal(|ui| {
        for (style, label) in cards {
            let is_selected = state.composition_style == style;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(72.0_f32, 68.0_f32), Sense::click());
            let thumb_rect = Rect::from_min_size(rect.min, Vec2::new(72.0_f32, 48.0_f32));

            let stroke = if is_selected {
                Stroke::new(2.0_f32, theme.brand_green)
            } else if resp.hovered() {
                Stroke::new(1.0_f32, theme.text02)
            } else {
                Stroke::new(1.0_f32, theme.border02)
            };

            ui.painter().rect_filled(thumb_rect, Rounding::same(6.0_f32), theme.bg03);
            ui.painter().rect_stroke(thumb_rect, Rounding::same(6.0_f32), stroke);

            // Silhouette icon
            let cx = thumb_rect.center().x;
            let cy = thumb_rect.center().y;
            let sil_color = if is_selected { theme.brand_green } else { theme.text02 };

            match style {
                CompositionStyle::Head => {
                    ui.painter().circle_filled(Pos2::new(cx, cy - 4.0_f32), 9.0_f32, sil_color);
                    ui.painter().rect_filled(
                        Rect::from_center_size(Pos2::new(cx, cy + 14.0_f32), Vec2::new(32.0_f32, 12.0_f32)),
                        Rounding::same(4.0_f32),
                        sil_color,
                    );
                }
                CompositionStyle::HalfBody => {
                    ui.painter().circle_filled(Pos2::new(cx, cy - 8.0_f32), 6.5_f32, sil_color);
                    ui.painter().rect_filled(
                        Rect::from_center_size(Pos2::new(cx, cy + 8.0_f32), Vec2::new(22.0_f32, 18.0_f32)),
                        Rounding::same(3.0_f32),
                        sil_color,
                    );
                }
                CompositionStyle::WholeBody | CompositionStyle::Custom => {
                    ui.painter().circle_filled(Pos2::new(cx, cy - 12.0_f32), 4.5_f32, sil_color);
                    ui.painter().rect_filled(
                        Rect::from_center_size(Pos2::new(cx, cy - 1.0_f32), Vec2::new(14.0_f32, 14.0_f32)),
                        Rounding::same(2.0_f32),
                        sil_color,
                    );
                    ui.painter().rect_filled(
                        Rect::from_center_size(Pos2::new(cx, cy + 12.0_f32), Vec2::new(10.0_f32, 10.0_f32)),
                        Rounding::same(1.0_f32),
                        sil_color,
                    );
                }
            }

            ui.painter().text(
                Pos2::new(rect.center().x, rect.max.y - 8.0_f32),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0_f32),
                if is_selected { theme.brand_green } else { theme.text02 },
            );

            if resp.clicked() {
                if !state.ai_zoom_enabled {
                    state.ai_zoom_enabled = true;
                    let _ = ctrl.set_ai_zoom(true);
                }
                state.composition_style = style;
                let _ = ctrl.set_composition_style(style);
            }
        }
    });
}

/// 236×150 Interactive Exposure Curve Canvas (`DefSplineCanvas.qml`).
fn render_exposure_curve_canvas<T: UvcControlTransport>(
    ui: &mut Ui,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    theme: &InsTheme,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Exposure curve").size(13.0_f32).color(theme.text01));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(RichText::new("—").size(14.0_f32).color(theme.text02)).clicked() {
                if state.spline_points.len() > 2 {
                    state.spline_points.remove(state.spline_points.len() - 2);
                    let curve = build_exposure_curve(&state.spline_points, 0, ISO_LIST[state.iso_index]);
                    let _ = ctrl.set_exposure_curve(&curve);
                }
            }
            if ui.button(RichText::new("↺").size(14.0_f32).color(theme.text02)).clicked() {
                state.spline_points = vec![
                    (0.0_f32, 0.0_f32),
                    (0.33_f32, 0.33_f32),
                    (0.67_f32, 0.67_f32),
                    (1.0_f32, 1.0_f32),
                ];
                let curve = build_exposure_curve(&state.spline_points, 0, ISO_LIST[state.iso_index]);
                let _ = ctrl.set_exposure_curve(&curve);
            }
        });
    });

    ui.add_space(4.0_f32);

    let (canvas_rect, response) =
        ui.allocate_exact_size(Vec2::new(236.0_f32, 150.0_f32), Sense::click_and_drag());
    let painter = ui.painter();

    painter.rect_filled(canvas_rect, Rounding::same(8.0_f32), theme.bg03);
    painter.rect_stroke(canvas_rect, Rounding::same(8.0_f32), Stroke::new(1.0_f32, theme.border02));

    // 4×4 Grid lines
    for i in 1..4 {
        let frac = i as f32 / 4.0_f32;
        let gx = canvas_rect.left() + frac * canvas_rect.width();
        painter.line_segment(
            [Pos2::new(gx, canvas_rect.top()), Pos2::new(gx, canvas_rect.bottom())],
            Stroke::new(1.0_f32, theme.border02),
        );
        let gy = canvas_rect.top() + frac * canvas_rect.height();
        painter.line_segment(
            [Pos2::new(canvas_rect.left(), gy), Pos2::new(canvas_rect.right(), gy)],
            Stroke::new(1.0_f32, theme.border02),
        );
    }

    // Diagonal reference line
    painter.line_segment(
        [
            Pos2::new(canvas_rect.left(), canvas_rect.bottom()),
            Pos2::new(canvas_rect.right(), canvas_rect.top()),
        ],
        Stroke::new(1.0_f32, theme.border02),
    );

    let mut curve_changed = false;

    // Drag point or click to add point
    if response.dragged() {
        if let Some(ptr) = response.interact_pointer_pos() {
            let norm_x = ((ptr.x - canvas_rect.left()) / canvas_rect.width()).clamp(0.0_f32, 1.0_f32);
            let norm_y = ((canvas_rect.bottom() - ptr.y) / canvas_rect.height()).clamp(0.0_f32, 1.0_f32);

            // Find closest control point
            let mut best_idx = 0;
            let mut best_dist = f32::MAX;
            for (idx, &(px, py)) in state.spline_points.iter().enumerate() {
                let dx = px - norm_x;
                let dy = py - norm_y;
                let d = dx * dx + dy * dy;
                if d < best_dist {
                    best_dist = d;
                    best_idx = idx;
                }
            }

            if best_dist < 0.05_f32 {
                let n = state.spline_points.len();
                if best_idx == 0 {
                    state.spline_points[0] = (0.0_f32, norm_y);
                } else if best_idx == n - 1 {
                    state.spline_points[best_idx] = (1.0_f32, norm_y);
                } else {
                    let min_x = state.spline_points[best_idx - 1].0 + 0.02_f32;
                    let max_x = state.spline_points[best_idx + 1].0 - 0.02_f32;
                    state.spline_points[best_idx] = (norm_x.clamp(min_x, max_x), norm_y);
                }
                curve_changed = true;
            }
        }
    } else if response.clicked() {
        if let Some(ptr) = response.interact_pointer_pos() {
            let norm_x = ((ptr.x - canvas_rect.left()) / canvas_rect.width()).clamp(0.0_f32, 1.0_f32);
            let norm_y = ((canvas_rect.bottom() - ptr.y) / canvas_rect.height()).clamp(0.0_f32, 1.0_f32);

            let is_near = state.spline_points.iter().any(|&(px, py)| {
                let dx = px - norm_x;
                let dy = py - norm_y;
                dx * dx + dy * dy < 0.01_f32
            });

            if !is_near && state.spline_points.len() < 8 {
                state.spline_points.push((norm_x, norm_y));
                state.spline_points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                curve_changed = true;
            }
        }
    }

    if curve_changed {
        let curve = build_exposure_curve(&state.spline_points, 0, ISO_LIST[state.iso_index]);
        let _ = ctrl.set_exposure_curve(&curve);
    }

    // Draw spline curve
    let mut prev_pt = None;
    for step in 0..=80 {
        let t = step as f32 / 80.0_f32;
        let val = evaluate_spline(&state.spline_points, t).clamp(0.0_f32, 1.0_f32);
        let pt = Pos2::new(
            canvas_rect.left() + t * canvas_rect.width(),
            canvas_rect.bottom() - val * canvas_rect.height(),
        );
        if let Some(prev) = prev_pt {
            painter.line_segment([prev, pt], Stroke::new(2.0_f32, theme.brand_green));
        }
        prev_pt = Some(pt);
    }

    // Draw control point circles
    for &(px, py) in &state.spline_points {
        let pt = Pos2::new(
            canvas_rect.left() + px * canvas_rect.width(),
            canvas_rect.bottom() - py * canvas_rect.height(),
        );
        painter.circle_filled(pt, 5.0_f32, theme.bg04);
        painter.circle_stroke(pt, 5.0_f32, Stroke::new(1.5_f32, theme.brand_green));
    }
}

/// Color Presets Grid (`right_img_color_preset`).
fn render_color_presets<T: UvcControlTransport>(
    ui: &mut Ui,
    ctx: &Context,
    state: &mut CameraAppState,
    ctrl: &mut Insta360CameraController<T>,
    video: &VideoPreviewEngine,
    theme: &InsTheme,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Color Presets").size(12.0_f32).strong().color(theme.text03));
    });

    ui.add_space(6.0_f32);

    Grid::new("color_presets_grid")
        .spacing([8.0_f32, 8.0_f32])
        .show(ui, |ui| {
            let mut col = 0;

            // [ + ] Add Color Preset tile
            let (add_rect, add_resp) = ui.allocate_exact_size(Vec2::new(52.0_f32, 52.0_f32), Sense::click());
            let add_stroke = if add_resp.hovered() {
                Stroke::new(1.0_f32, theme.brand_green)
            } else {
                Stroke::new(1.0_f32, theme.border02)
            };
            ui.painter().rect_filled(add_rect, Rounding::same(8.0_f32), theme.overlay02);
            ui.painter().rect_stroke(add_rect, Rounding::same(8.0_f32), add_stroke);
            ui.painter().text(
                add_rect.center(),
                Align2::CENTER_CENTER,
                "+",
                FontId::proportional(22.0_f32),
                theme.text01,
            );

            if add_resp.clicked() {
                let next_id = (state.color_presets.len() as i8) + 1;
                let img = video.capture_thumbnail();
                let tex = ctx.load_texture(
                    format!("color_preset_{}", next_id),
                    img.clone(),
                    egui::TextureOptions::LINEAR,
                );

                state.color_presets.push(ColorPresetSlot {
                    id: next_id,
                    name: format!("Preset {}", next_id),
                    saved: true,
                    brightness: state.brightness,
                    contrast: state.contrast,
                    saturation: state.saturation,
                    sharpness: state.sharpness,
                    wb_auto: state.auto_white_balance,
                    wb_kelvin: state.wb_kelvin,
                    auto_exposure: state.auto_exposure,
                    ev_comp: state.ev_compensation,
                    iso: ISO_LIST.get(state.iso_index).copied().unwrap_or(100),
                    shutter: SHUTTER_DENOM_LIST.get(state.shutter_index).copied().unwrap_or(100),
                    hdr: state.hdr_enabled,
                    thumbnail: Some(tex),
                    raw_thumb: Some(img),
                    renaming: false,
                });
                let _ = ctrl.set_image_template(next_id, ImageOperation::SaveOrUpdate);
            }

            col += 1;

            // Saved Color Preset Slots
            let mut apply_id = None;
            let mut update_id = None;
            let mut delete_id = None;

            for slot in state.color_presets.iter_mut().filter(|s| s.saved) {
                let is_active = state.active_color_preset_id == Some(slot.id);
                let (tile_rect, tile_resp) = ui.allocate_exact_size(Vec2::new(52.0_f32, 68.0_f32), Sense::click());
                let box_rect = Rect::from_min_size(tile_rect.min, Vec2::new(52.0_f32, 52.0_f32));

                if slot.thumbnail.is_none() {
                    if let Some(raw) = &slot.raw_thumb {
                        slot.thumbnail = Some(ctx.load_texture(
                            format!("color_preset_{}", slot.id),
                            raw.clone(),
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }

                if let Some(tex) = &slot.thumbnail {
                    ui.painter().image(
                        tex.id(),
                        box_rect,
                        Rect::from_min_max(Pos2::new(0.0_f32, 0.0_f32), Pos2::new(1.0_f32, 1.0_f32)),
                        Color32::WHITE,
                    );
                } else {
                    ui.painter().rect_filled(box_rect, Rounding::same(8.0_f32), theme.bg03);
                    ui.painter().text(
                        box_rect.center(),
                        Align2::CENTER_CENTER,
                        "🎨",
                        FontId::proportional(18.0_f32),
                        theme.text03,
                    );
                }

                let border_stroke = if is_active {
                    Stroke::new(2.0_f32, theme.brand_green)
                } else if tile_resp.hovered() {
                    Stroke::new(1.0_f32, theme.text02)
                } else {
                    Stroke::new(1.0_f32, theme.border02)
                };
                ui.painter().rect_stroke(box_rect, Rounding::same(8.0_f32), border_stroke);

                if slot.renaming {
                    let mut name_ui = ui.new_child(
                        egui::UiBuilder::new().max_rect(Rect::from_min_size(
                            Pos2::new(tile_rect.left(), box_rect.bottom() + 2.0_f32),
                            Vec2::new(52.0_f32, 14.0_f32),
                        )),
                    );
                    let resp = name_ui.add(TextEdit::singleline(&mut slot.name).font(FontId::proportional(10.0_f32)));
                    if resp.lost_focus() || name_ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        slot.renaming = false;
                    }
                } else {
                    ui.painter().text(
                        Pos2::new(tile_rect.center().x, box_rect.bottom() + 8.0_f32),
                        Align2::CENTER_CENTER,
                        &slot.name,
                        FontId::proportional(11.0_f32),
                        if is_active { theme.brand_green } else { theme.text02 },
                    );
                }

                if tile_resp.clicked() {
                    apply_id = Some(slot.id);
                }

                tile_resp.context_menu(|ui| {
                    if ui.button("Update").clicked() {
                        update_id = Some(slot.id);
                        ui.close_menu();
                    }
                    if ui.button("Rename").clicked() {
                        slot.renaming = true;
                        ui.close_menu();
                    }
                    if ui.button("Delete").clicked() {
                        delete_id = Some(slot.id);
                        ui.close_menu();
                    }
                });

                col += 1;
                if col % 4 == 0 {
                    ui.end_row();
                }
            }

            if let Some(id) = apply_id {
                if let Some(slot) = state.color_presets.iter().find(|s| s.id == id) {
                    state.active_color_preset_id = Some(slot.id);
                    state.brightness = slot.brightness;
                    state.contrast = slot.contrast;
                    state.saturation = slot.saturation;
                    state.sharpness = slot.sharpness;
                    state.auto_white_balance = slot.wb_auto;
                    state.wb_kelvin = slot.wb_kelvin;
                    state.auto_exposure = slot.auto_exposure;
                    state.ev_compensation = slot.ev_comp;
                    state.hdr_enabled = slot.hdr;
                    let _ = ctrl.set_image_template(slot.id, ImageOperation::Apply);
                }
            }

            if let Some(id) = update_id {
                if let Some(slot) = state.color_presets.iter_mut().find(|s| s.id == id) {
                    slot.brightness = state.brightness;
                    slot.contrast = state.contrast;
                    slot.saturation = state.saturation;
                    slot.sharpness = state.sharpness;
                    slot.wb_auto = state.auto_white_balance;
                    slot.wb_kelvin = state.wb_kelvin;
                    slot.auto_exposure = state.auto_exposure;
                    slot.ev_comp = state.ev_compensation;
                    slot.hdr = state.hdr_enabled;
                    let img = video.capture_thumbnail();
                    let tex = ctx.load_texture(
                        format!("color_preset_{}", slot.id),
                        img.clone(),
                        egui::TextureOptions::LINEAR,
                    );
                    slot.thumbnail = Some(tex);
                    slot.raw_thumb = Some(img);
                    let _ = ctrl.set_image_template(slot.id, ImageOperation::SaveOrUpdate);
                }
            }

            if let Some(id) = delete_id {
                if let Some(slot) = state.color_presets.iter_mut().find(|s| s.id == id) {
                    slot.saved = false;
                    let _ = ctrl.delete_image_template(slot.id);
                }
            }
        });
}
