#![allow(dead_code)]

use egui::{
    Align, Align2, Button, Color32, Context, FontId, Frame, Layout, Margin, Pos2, Rect, Response,
    RichText, Rounding, Sense, Stroke, Style, Ui, Vec2, Visuals, WidgetInfo, WidgetType,
};

/// Official Insta360 Link Controller v2.0.2 theme palette (`InsUI.qml`).
#[derive(Debug, Clone, Copy)]
pub struct InsTheme {
    pub is_light_mode: bool,
    pub brand_green: Color32,
    pub brand_green_lighter: Color32,
    pub brand_light: Color32,
    pub brand_text: Color32,
    pub bg01: Color32,
    pub bg02: Color32,
    pub bg03: Color32,
    pub bg04: Color32,
    pub bg05: Color32,
    pub border01: Color32,
    pub border02: Color32,
    pub border03: Color32,
    pub text01: Color32,
    pub text02: Color32,
    pub text03: Color32,
    pub text04: Color32,
    pub overlay01: Color32,
    pub overlay02: Color32,
    pub overlay03: Color32,
    pub mask01: Color32,
    pub error: Color32,
    pub success: Color32,
    pub warning: Color32,
}

impl InsTheme {
    pub fn new(is_light_mode: bool) -> Self {
        if is_light_mode {
            Self {
                is_light_mode: true,
                brand_green: Color32::from_rgb(18, 219, 139),
                brand_green_lighter: Color32::from_rgb(81, 245, 179),
                brand_light: Color32::from_rgb(240, 255, 249),
                brand_text: Color32::from_rgb(0, 178, 107),
                bg01: Color32::from_rgb(229, 229, 229),
                bg02: Color32::from_rgb(240, 240, 240),
                bg03: Color32::from_rgb(250, 250, 250),
                bg04: Color32::from_rgb(255, 255, 255),
                bg05: Color32::from_rgb(255, 255, 255),
                border01: Color32::from_rgba_unmultiplied(0, 0, 0, 26),
                border02: Color32::from_rgba_unmultiplied(0, 0, 0, 46),
                border03: Color32::from_rgba_unmultiplied(0, 0, 0, 66),
                text01: Color32::from_rgb(41, 41, 41),
                text02: Color32::from_rgb(82, 82, 82),
                text03: Color32::from_rgb(153, 153, 153),
                text04: Color32::from_rgb(191, 191, 191),
                overlay01: Color32::from_rgba_unmultiplied(255, 255, 255, 255),
                overlay02: Color32::from_rgba_unmultiplied(255, 255, 255, 255),
                overlay03: Color32::from_rgba_unmultiplied(255, 255, 255, 255),
                mask01: Color32::from_rgba_unmultiplied(0, 0, 0, 153),
                error: Color32::from_rgb(255, 64, 77),
                success: Color32::from_rgb(0, 209, 112),
                warning: Color32::from_rgb(245, 217, 37),
            }
        } else {
            Self {
                is_light_mode: false,
                brand_green: Color32::from_rgb(42, 235, 158),
                brand_green_lighter: Color32::from_rgb(26, 255, 163),
                brand_light: Color32::from_rgb(47, 51, 49),
                brand_text: Color32::from_rgb(15, 184, 116),
                bg01: Color32::from_rgb(33, 33, 33),
                bg02: Color32::from_rgb(43, 43, 43),
                bg03: Color32::from_rgb(51, 51, 51),
                bg04: Color32::from_rgb(56, 56, 56),
                bg05: Color32::from_rgb(71, 71, 71),
                border01: Color32::from_rgba_unmultiplied(255, 255, 255, 20),
                border02: Color32::from_rgba_unmultiplied(255, 255, 255, 36),
                border03: Color32::from_rgba_unmultiplied(255, 255, 255, 51),
                text01: Color32::from_rgb(235, 235, 235),
                text02: Color32::from_rgb(184, 184, 184),
                text03: Color32::from_rgb(138, 138, 138),
                text04: Color32::from_rgb(89, 89, 89),
                overlay01: Color32::from_rgba_unmultiplied(216, 231, 245, 15),
                overlay02: Color32::from_rgba_unmultiplied(235, 245, 255, 26),
                overlay03: Color32::from_rgba_unmultiplied(235, 245, 255, 38),
                mask01: Color32::from_rgba_unmultiplied(0, 0, 0, 153),
                error: Color32::from_rgb(255, 84, 96),
                success: Color32::from_rgb(0, 209, 49),
                warning: Color32::from_rgb(249, 232, 124),
            }
        }
    }
}

impl Default for InsTheme {
    fn default() -> Self {
        Self::new(false)
    }
}

/// Configures `egui::Style` and `egui::Visuals` with the official Insta360 Link Controller palette
/// (`bg01`, `bg02`, `bg03`, `brand_green` accents) and returns the configured `InsTheme`.
pub fn apply_ins_theme(ctx: &Context, is_light_mode: bool) -> InsTheme {
    let theme = InsTheme::new(is_light_mode);
    let mut style: Style = (*ctx.style()).clone();
    let mut visuals = if is_light_mode {
        Visuals::light()
    } else {
        Visuals::dark()
    };

    visuals.panel_fill = theme.bg01;
    visuals.window_fill = theme.bg02;
    visuals.extreme_bg_color = theme.bg01;
    visuals.faint_bg_color = theme.bg03;

    visuals.selection.bg_fill = theme.brand_green;
    visuals.selection.stroke = Stroke::new(
        1.0_f32,
        if is_light_mode {
            Color32::WHITE
        } else {
            theme.bg01
        },
    );
    visuals.hyperlink_color = theme.brand_green;

    let r6 = Rounding::same(6.0_f32);
    let r8 = Rounding::same(8.0_f32);

    visuals.widgets.noninteractive.bg_fill = theme.bg02;
    visuals.widgets.noninteractive.weak_bg_fill = theme.bg02;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, theme.border01);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, theme.text01);
    visuals.widgets.noninteractive.rounding = r8;

    visuals.widgets.inactive.bg_fill = theme.bg03;
    visuals.widgets.inactive.weak_bg_fill = theme.bg03;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, theme.border01);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, theme.text02);
    visuals.widgets.inactive.rounding = r6;

    visuals.widgets.hovered.bg_fill = theme.bg05;
    visuals.widgets.hovered.weak_bg_fill = theme.bg05;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, theme.brand_green);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, theme.text01);
    visuals.widgets.hovered.rounding = r6;

    visuals.widgets.active.bg_fill = theme.brand_green;
    visuals.widgets.active.weak_bg_fill = theme.brand_green;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, theme.brand_green_lighter);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.rounding = r6;

    visuals.widgets.open.bg_fill = theme.bg04;
    visuals.widgets.open.weak_bg_fill = theme.bg04;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, theme.brand_green);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, theme.text01);
    visuals.widgets.open.rounding = r6;

    style.spacing.item_spacing = Vec2::new(8.0_f32, 6.0_f32);
    style.spacing.button_padding = Vec2::new(8.0_f32, 5.0_f32);
    style.visuals = visuals;
    ctx.set_style(style);

    theme
}

/// Renders a rounded card container with `fill(theme.bg02)`, `rounding(8.0)`,
/// and `inner_margin(Margin::symmetric(16.0, 12.0))`.
pub fn ins_card<R>(
    ui: &mut Ui,
    theme: &InsTheme,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    Frame::none()
        .fill(theme.bg02)
        .rounding(Rounding::same(8.0_f32))
        .inner_margin(Margin::symmetric(16.0_f32, 12.0_f32))
        .show(ui, add_contents)
        .inner
}

/// Renders a 32px-tall header row with `title` on the left (14.0 strong `theme.text01`)
/// and an optional `↺` reset button on the right (returns `true` if clicked).
pub fn ins_section_header(
    ui: &mut Ui,
    title: &str,
    theme: &InsTheme,
    reset_tooltip: Option<&str>,
) -> bool {
    let mut reset_clicked = false;
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), 32.0_f32),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.label(
                RichText::new(title)
                    .size(14.0_f32)
                    .strong()
                    .color(theme.text01),
            );
            if let Some(tooltip) = reset_tooltip {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let btn = ui.add(
                        Button::new(RichText::new("↺").size(14.0_f32).color(theme.text03))
                            .frame(false),
                    );
                    if !tooltip.is_empty() {
                        btn.clone().on_hover_text(tooltip);
                    }
                    if btn.clicked() {
                        reset_clicked = true;
                    }
                });
            }
        },
    );
    reset_clicked
}

/// Draws a 34.0 x 18.0 pill toggle switch (`theme.brand_green` when `*checked`,
/// `theme.bg05` when false, white circle thumb) and returns `true` if toggled.
pub fn ins_switch(ui: &mut Ui, checked: &mut bool, enabled: bool, theme: &InsTheme) -> bool {
    let desired_size = Vec2::new(34.0_f32, 18.0_f32);
    let (rect, mut response) = ui.allocate_exact_size(
        desired_size,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::Checkbox,
            ui.is_enabled() && enabled,
            *checked,
            "",
        )
    });

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *checked);
        let track_rounding = Rounding::same(rect.height() / 2.0_f32);

        let track_color = if enabled {
            if *checked {
                theme.brand_green
            } else {
                theme.bg05
            }
        } else if *checked {
            Color32::from_rgba_unmultiplied(
                theme.brand_green.r(),
                theme.brand_green.g(),
                theme.brand_green.b(),
                100,
            )
        } else {
            theme.border01
        };

        ui.painter().rect_filled(rect, track_rounding, track_color);

        let thumb_radius = (rect.height() - 4.0_f32) / 2.0_f32;
        let min_x = rect.min.x + 2.0_f32 + thumb_radius;
        let max_x = rect.max.x - 2.0_f32 - thumb_radius;
        let thumb_x = egui::lerp(min_x..=max_x, how_on);
        let thumb_center = Pos2::new(thumb_x, rect.center().y);
        let thumb_color = if enabled {
            Color32::WHITE
        } else {
            theme.text04
        };

        ui.painter()
            .circle_filled(thumb_center, thumb_radius, thumb_color);
    }

    response.changed()
}

/// Full-width row (28px height) with `label` (13.0 `theme.text01`) + optional `?` hover tooltip
/// on the left, and `ins_switch` on the right. Returns `true` if toggled.
pub fn ins_switch_row(
    ui: &mut Ui,
    label: &str,
    tooltip: Option<&str>,
    checked: &mut bool,
    enabled: bool,
    theme: &InsTheme,
) -> bool {
    let row_height = 28.0_f32;
    let (row_rect, row_response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    let mut toggled = false;

    let switch_width = 34.0_f32;
    let switch_height = 18.0_f32;
    let switch_rect = Rect::from_min_size(
        Pos2::new(
            row_rect.max.x - switch_width,
            row_rect.center().y - switch_height / 2.0_f32,
        ),
        Vec2::new(switch_width, switch_height),
    );

    let switch_id = row_response.id.with("switch");
    let switch_response = ui.interact(
        switch_rect,
        switch_id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    if enabled {
        if switch_response.clicked() {
            *checked = !*checked;
            toggled = true;
        } else if row_response.clicked() {
            *checked = !*checked;
            toggled = true;
        }
    }

    if ui.is_rect_visible(row_rect) {
        let text_color = if enabled {
            theme.text01
        } else {
            theme.text04
        };
        let text_pos = Pos2::new(row_rect.min.x, row_rect.center().y);

        let label_galley = ui.painter().layout_no_wrap(
            label.to_string(),
            FontId::proportional(13.0_f32),
            text_color,
        );
        let label_size = label_galley.size();
        ui.painter().galley(
            Pos2::new(text_pos.x, text_pos.y - label_size.y / 2.0_f32),
            label_galley,
            text_color,
        );

        if let Some(tt) = tooltip {
            let tip_x = text_pos.x + label_size.x + 6.0_f32;
            let tip_rect = Rect::from_min_size(
                Pos2::new(tip_x, text_pos.y - 8.0_f32),
                Vec2::new(16.0_f32, 16.0_f32),
            );
            let tip_resp = ui.interact(tip_rect, row_response.id.with("tt"), Sense::hover());
            let tip_color = if tip_resp.hovered() {
                theme.text01
            } else {
                theme.text03
            };
            ui.painter().text(
                tip_rect.center(),
                Align2::CENTER_CENTER,
                "?",
                FontId::proportional(12.0_f32),
                tip_color,
            );
            if !tt.is_empty() {
                tip_resp.on_hover_text(tt);
            }
        }

        // Render switch
        let how_on = ui.ctx().animate_bool(switch_id, *checked);
        let track_rounding = Rounding::same(switch_rect.height() / 2.0_f32);
        let track_color = if enabled {
            if *checked {
                theme.brand_green
            } else {
                theme.bg05
            }
        } else if *checked {
            Color32::from_rgba_unmultiplied(
                theme.brand_green.r(),
                theme.brand_green.g(),
                theme.brand_green.b(),
                100,
            )
        } else {
            theme.border01
        };

        ui.painter()
            .rect_filled(switch_rect, track_rounding, track_color);

        let thumb_radius = (switch_rect.height() - 4.0_f32) / 2.0_f32;
        let min_x = switch_rect.min.x + 2.0_f32 + thumb_radius;
        let max_x = switch_rect.max.x - 2.0_f32 - thumb_radius;
        let thumb_x = egui::lerp(min_x..=max_x, how_on);
        let thumb_center = Pos2::new(thumb_x, switch_rect.center().y);
        let thumb_color = if enabled {
            Color32::WHITE
        } else {
            theme.text04
        };

        ui.painter()
            .circle_filled(thumb_center, thumb_radius, thumb_color);
    }

    toggled
}

/// Full-width row (28px height) with optional `icon_prefix` + `label` on the left
/// and a custom 16x16 rounded checkbox (`theme.brand_green` fill + checkmark when `*checked`)
/// on the right. Returns `true` if toggled.
pub fn ins_checkbox_row(
    ui: &mut Ui,
    label: &str,
    icon_prefix: Option<&str>,
    tooltip: Option<&str>,
    checked: &mut bool,
    enabled: bool,
    theme: &InsTheme,
) -> bool {
    let row_height = 28.0_f32;
    let (row_rect, row_response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    let mut toggled = false;

    let box_size = 16.0_f32;
    let box_rect = Rect::from_min_size(
        Pos2::new(
            row_rect.max.x - box_size,
            row_rect.center().y - box_size / 2.0_f32,
        ),
        Vec2::new(box_size, box_size),
    );

    let box_id = row_response.id.with("cb");
    let box_response = ui.interact(
        box_rect,
        box_id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    if enabled && (box_response.clicked() || row_response.clicked()) {
        *checked = !*checked;
        toggled = true;
    }

    if ui.is_rect_visible(row_rect) {
        let text_color = if enabled {
            theme.text01
        } else {
            theme.text04
        };
        let mut cur_x = row_rect.min.x;

        if let Some(prefix) = icon_prefix {
            let prefix_galley = ui.painter().layout_no_wrap(
                prefix.to_string(),
                FontId::proportional(14.0_f32),
                theme.text03,
            );
            let p_size = prefix_galley.size();
            ui.painter().galley(
                Pos2::new(cur_x, row_rect.center().y - p_size.y / 2.0_f32),
                prefix_galley,
                theme.text03,
            );
            cur_x += p_size.x + 6.0_f32;
        }

        let label_galley = ui.painter().layout_no_wrap(
            label.to_string(),
            FontId::proportional(13.0_f32),
            text_color,
        );
        let label_size = label_galley.size();
        ui.painter().galley(
            Pos2::new(cur_x, row_rect.center().y - label_size.y / 2.0_f32),
            label_galley,
            text_color,
        );
        cur_x += label_size.x + 6.0_f32;

        if let Some(tt) = tooltip {
            let tip_rect = Rect::from_min_size(
                Pos2::new(cur_x, row_rect.center().y - 8.0_f32),
                Vec2::new(16.0_f32, 16.0_f32),
            );
            let tip_resp = ui.interact(tip_rect, row_response.id.with("cb_tt"), Sense::hover());
            let tip_color = if tip_resp.hovered() {
                theme.text01
            } else {
                theme.text03
            };
            ui.painter().text(
                tip_rect.center(),
                Align2::CENTER_CENTER,
                "?",
                FontId::proportional(12.0_f32),
                tip_color,
            );
            if !tt.is_empty() {
                tip_resp.on_hover_text(tt);
            }
        }

        // Draw checkbox indicator
        let r3 = Rounding::same(3.0_f32);
        if *checked {
            let fill = if enabled {
                theme.brand_green
            } else {
                Color32::from_rgba_unmultiplied(
                    theme.brand_green.r(),
                    theme.brand_green.g(),
                    theme.brand_green.b(),
                    120,
                )
            };
            ui.painter().rect_filled(box_rect, r3, fill);

            // Dark checkmark
            let check_color = if theme.is_light_mode {
                Color32::WHITE
            } else {
                Color32::from_rgb(20, 20, 20)
            };
            let stroke = Stroke::new(1.8_f32, check_color);
            let p1 = Pos2::new(box_rect.min.x + 3.5_f32, box_rect.min.y + 8.0_f32);
            let p2 = Pos2::new(box_rect.min.x + 6.5_f32, box_rect.min.y + 11.5_f32);
            let p3 = Pos2::new(box_rect.min.x + 12.5_f32, box_rect.min.y + 4.5_f32);
            ui.painter().line_segment([p1, p2], stroke);
            ui.painter().line_segment([p2, p3], stroke);
        } else {
            let bg = if enabled && box_response.hovered() {
                theme.bg04
            } else {
                theme.bg03
            };
            let stroke_color = if enabled && box_response.hovered() {
                theme.brand_green
            } else {
                theme.border02
            };
            ui.painter().rect_filled(box_rect, r3, bg);
            ui.painter()
                .rect_stroke(box_rect, r3, Stroke::new(1.0_f32, stroke_color));
        }
    }

    toggled
}

/// Matches `DefSlider.qml`: top row has `label` on the left (if `Some`) and current value + `unit`
/// on the right; bottom row has a custom full-width slider track (`theme.brand_green` left of thumb,
/// `theme.bg05` right of thumb). Returns `true` if changed.
pub fn ins_slider_i32(
    ui: &mut Ui,
    label: Option<&str>,
    value: &mut i32,
    min: i32,
    max: i32,
    unit: &str,
    enabled: bool,
    theme: &InsTheme,
) -> bool {
    let mut changed = false;
    let initial_val = *value;

    ui.vertical(|ui| {
        // Top row
        ui.horizontal(|ui| {
            if let Some(lbl) = label {
                ui.label(
                    RichText::new(lbl).size(13.0_f32).color(if enabled {
                        theme.text02
                    } else {
                        theme.text04
                    }),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let val_str = format!("{}{}", *value, unit);
                ui.label(
                    RichText::new(val_str).size(12.0_f32).color(if enabled {
                        theme.text01
                    } else {
                        theme.text04
                    }),
                );
            });
        });

        // Bottom track
        let track_height = 16.0_f32;
        let (track_rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), track_height),
            if enabled {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );

        let thumb_radius = 6.0_f32;
        let min_x = track_rect.min.x + thumb_radius;
        let max_x = track_rect.max.x - thumb_radius;
        let range = (max - min).max(1) as f32;

        if enabled && (response.clicked() || response.dragged()) {
            if let Some(pos) = response.interact_pointer_pos() {
                let norm = ((pos.x - min_x) / (max_x - min_x).max(1.0_f32)).clamp(0.0_f32, 1.0_f32);
                let new_val = min + (norm * range).round() as i32;
                let clamped_val = new_val.clamp(min, max);
                if clamped_val != *value {
                    *value = clamped_val;
                    changed = true;
                }
            }
        }

        if ui.is_rect_visible(track_rect) {
            let norm = ((*value - min) as f32 / range).clamp(0.0_f32, 1.0_f32);
            let thumb_x = min_x + norm * (max_x - min_x);
            let bar_y = track_rect.center().y;
            let bar_h = 3.0_f32;
            let bar_rounding = Rounding::same(1.5_f32);

            // Left track (brand_green)
            let left_rect = Rect::from_min_max(
                Pos2::new(track_rect.min.x, bar_y - bar_h / 2.0_f32),
                Pos2::new(thumb_x, bar_y + bar_h / 2.0_f32),
            );
            ui.painter().rect_filled(
                left_rect,
                bar_rounding,
                if enabled {
                    theme.brand_green
                } else {
                    theme.border01
                },
            );

            // Right track (bg05)
            let right_rect = Rect::from_min_max(
                Pos2::new(thumb_x, bar_y - bar_h / 2.0_f32),
                Pos2::new(track_rect.max.x, bar_y + bar_h / 2.0_f32),
            );
            ui.painter().rect_filled(right_rect, bar_rounding, theme.bg05);

            // Thumb
            let thumb_center = Pos2::new(thumb_x, bar_y);
            let thumb_fill = if enabled {
                Color32::WHITE
            } else {
                theme.text04
            };
            ui.painter()
                .circle_filled(thumb_center, thumb_radius, thumb_fill);
            ui.painter()
                .circle_stroke(thumb_center, thumb_radius, Stroke::new(1.0_f32, theme.border03));
        }
    });

    changed || *value != initial_val
}

/// Matches `DefSlider.qml` for `f32`: top row has `label` and formatted value + `unit`;
/// bottom row has custom track. Returns `true` if changed.
pub fn ins_slider_f32(
    ui: &mut Ui,
    label: Option<&str>,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
    decimals: usize,
    unit: &str,
    enabled: bool,
    theme: &InsTheme,
) -> bool {
    let mut changed = false;
    let initial_val = *value;

    ui.vertical(|ui| {
        // Top row
        ui.horizontal(|ui| {
            if let Some(lbl) = label {
                ui.label(
                    RichText::new(lbl).size(13.0_f32).color(if enabled {
                        theme.text02
                    } else {
                        theme.text04
                    }),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let val_str = format!("{:.decimals$}{}", *value, unit);
                ui.label(
                    RichText::new(val_str).size(12.0_f32).color(if enabled {
                        theme.text01
                    } else {
                        theme.text04
                    }),
                );
            });
        });

        // Bottom track
        let track_height = 16.0_f32;
        let (track_rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), track_height),
            if enabled {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );

        let thumb_radius = 6.0_f32;
        let min_x = track_rect.min.x + thumb_radius;
        let max_x = track_rect.max.x - thumb_radius;
        let range = (max - min).max(0.0001_f32);

        if enabled && (response.clicked() || response.dragged()) {
            if let Some(pos) = response.interact_pointer_pos() {
                let norm = ((pos.x - min_x) / (max_x - min_x).max(1.0_f32)).clamp(0.0_f32, 1.0_f32);
                let mut new_val = min + norm * range;
                if step > 0.0_f32 {
                    new_val = (new_val / step).round() * step;
                }
                let clamped_val = new_val.clamp(min, max);
                if (clamped_val - *value).abs() > 1e-6_f32 {
                    *value = clamped_val;
                    changed = true;
                }
            }
        }

        if ui.is_rect_visible(track_rect) {
            let norm = ((*value - min) / range).clamp(0.0_f32, 1.0_f32);
            let thumb_x = min_x + norm * (max_x - min_x);
            let bar_y = track_rect.center().y;
            let bar_h = 3.0_f32;
            let bar_rounding = Rounding::same(1.5_f32);

            // Left track (brand_green)
            let left_rect = Rect::from_min_max(
                Pos2::new(track_rect.min.x, bar_y - bar_h / 2.0_f32),
                Pos2::new(thumb_x, bar_y + bar_h / 2.0_f32),
            );
            ui.painter().rect_filled(
                left_rect,
                bar_rounding,
                if enabled {
                    theme.brand_green
                } else {
                    theme.border01
                },
            );

            // Right track (bg05)
            let right_rect = Rect::from_min_max(
                Pos2::new(thumb_x, bar_y - bar_h / 2.0_f32),
                Pos2::new(track_rect.max.x, bar_y + bar_h / 2.0_f32),
            );
            ui.painter().rect_filled(right_rect, bar_rounding, theme.bg05);

            // Thumb
            let thumb_center = Pos2::new(thumb_x, bar_y);
            let thumb_fill = if enabled {
                Color32::WHITE
            } else {
                theme.text04
            };
            ui.painter()
                .circle_filled(thumb_center, thumb_radius, thumb_fill);
            ui.painter()
                .circle_stroke(thumb_center, thumb_radius, Stroke::new(1.0_f32, theme.border03));
        }
    });

    changed || (*value - initial_val).abs() > 1e-6_f32
}

/// Renders a tab button (`width x 32.0`, `rounding(6.0)`, `fill(if active { theme.bg03 } else { Color32::TRANSPARENT })`,
/// text color `if active { theme.brand_green } else { theme.text02 }`). Returns `true` if clicked.
pub fn ins_tab_button(
    ui: &mut Ui,
    label: &str,
    active: bool,
    width: f32,
    theme: &InsTheme,
) -> bool {
    let size = Vec2::new(width, 32.0_f32);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let fill = if active {
            theme.bg03
        } else if response.hovered() {
            theme.bg04
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, Rounding::same(6.0_f32), fill);

        let text_color = if active {
            theme.brand_green
        } else if response.hovered() {
            theme.text01
        } else {
            theme.text02
        };

        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(13.0_f32),
            text_color,
        );
    }

    response.clicked()
}

/// Renders a 74.0 x 52.0 mode button for `BottomControl.qml` with `icon` (18px)
/// centered above `label` (11.5px), colored `theme.brand_green` when `active` or
/// `theme.text01` when inactive.
pub fn ins_bottom_mode_button(
    ui: &mut Ui,
    icon: &str,
    label: &str,
    active: bool,
    enabled: bool,
    theme: &InsTheme,
) -> Response {
    let size = Vec2::new(74.0_f32, 52.0_f32);
    let (rect, response) = ui.allocate_exact_size(
        size,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    if ui.is_rect_visible(rect) {
        let r6 = Rounding::same(6.0_f32);
        let bg_fill = if active {
            theme.brand_light
        } else if enabled && response.hovered() {
            theme.bg03
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, r6, bg_fill);

        if active {
            ui.painter()
                .rect_stroke(rect, r6, Stroke::new(1.0_f32, theme.brand_green));
        }

        let main_color = if !enabled {
            theme.text04
        } else if active {
            theme.brand_green
        } else {
            theme.text01
        };

        let label_color = if !enabled {
            theme.text04
        } else if active {
            theme.brand_green
        } else {
            theme.text02
        };

        // Icon at top
        let icon_pos = Pos2::new(rect.center().x, rect.min.y + 18.0_f32);
        ui.painter().text(
            icon_pos,
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(18.0_f32),
            main_color,
        );

        // Label below
        let label_pos = Pos2::new(rect.center().x, rect.min.y + 38.0_f32);
        ui.painter().text(
            label_pos,
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.5_f32),
            label_color,
        );
    }

    response
}
