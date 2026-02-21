//! Material settings GUI window.
//!
//! Shows as a second egui window alongside the Terraformer panel.  Exposes
//! all [`MaterialConfig`] parameters: enable toggle, texture size, tile
//! scale, per-layer splat rules, and per-layer texture generator configs.
//!
//! # Change-detection design
//!
//! `MaterialConfig` is accessed via [`DetectChanges::bypass_change_detection`]
//! to prevent Bevy's `ResMut` auto-marking every frame (which would cancel
//! in-flight texture tasks on every frame).  Instead, changes are detected
//! through egui's widget return values and flushed at the end of the system
//! by calling [`DetectChanges::set_changed`].

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use bevy_symbios_texture::ground::GroundConfig;
use bevy_symbios_texture::rock::RockConfig;

use crate::core::material_config::{
    MaterialConfig, MaterialState, MaterialStatus, SplatRuleParams,
};

pub fn render_material_ui(
    mut contexts: EguiContexts,
    mut config: ResMut<MaterialConfig>,
    mut mat_state: ResMut<MaterialState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Pre-extract display values without triggering MaterialState change detection.
    let (status, layers_ready) = {
        let ms = mat_state.bypass_change_detection();
        let ready = ms.layer_albedo.iter().filter(|d| d.is_some()).count();
        (ms.status.clone(), ready)
    };

    // Separate change tracking:
    //   rules_changed  → splat weight map must be rebuilt (cheap, immediate)
    //   texture_changed → procedural textures must be regenerated (expensive, debounced)
    let mut rules_changed = false;
    let mut texture_changed = false;

    {
        // bypass_change_detection lets us read/write MaterialConfig fields
        // without Bevy marking the resource as changed on every render frame.
        // We call set_changed() manually below when egui reports a real edit.
        let cfg = config.bypass_change_detection();

        egui::Window::new("Materials")
            .default_width(340.0)
            .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-10.0, 10.0))
            .show(ctx, |ui| {
                // ── Enable ────────────────────────────────────────────────
                // Toggling the enable switch requires a full rebuild of both
                // textures and splat weights.
                let prev = cfg.enabled;
                ui.checkbox(&mut cfg.enabled, "Enable splat materials");
                if cfg.enabled != prev {
                    rules_changed = true;
                    texture_changed = true;
                }

                // Returns (rules_changed, texture_changed).
                let (inner_rules, inner_tex) = ui
                    .add_enabled_ui(cfg.enabled, |ui| {
                        let mut rules = false;
                        let mut tex = false;

                        // ── Global settings ───────────────────────────────
                        // texture_size triggers procedural texture regeneration.
                        // tile_scale is a shader uniform only — it must NOT
                        // trigger texture tasks; route it to rules_changed so
                        // only the cheap weight-map + uniform update runs.
                        let (gs_tex, gs_rules) = egui::CollapsingHeader::new("Global Settings")
                            .default_open(true)
                            .show(ui, |ui| {
                                let mut tex_inner = false;
                                let mut rules_inner = false;

                                let prev_sz = cfg.texture_size;
                                ui.horizontal(|ui| {
                                    ui.label("Texture size");
                                    egui::ComboBox::from_id_salt("tex_size")
                                        .selected_text(format!(
                                            "{}×{}",
                                            cfg.texture_size, cfg.texture_size
                                        ))
                                        .show_ui(ui, |ui| {
                                            for &s in &[256u32, 512, 1024, 2048, 4096] {
                                                if ui
                                                    .selectable_label(
                                                        cfg.texture_size == s,
                                                        format!("{s}×{s}"),
                                                    )
                                                    .clicked()
                                                {
                                                    cfg.texture_size = s;
                                                }
                                            }
                                        });
                                });
                                if cfg.texture_size != prev_sz {
                                    tex_inner = true;
                                }

                                rules_inner |=
                                    f32_slider(ui, &mut cfg.tile_scale, "Tile scale", 1.0..=512.0);

                                (tex_inner, rules_inner)
                            })
                            .body_returned
                            .unwrap_or((false, false));
                        tex |= gs_tex;
                        rules |= gs_rules;

                        ui.separator();

                        // ── Layers ────────────────────────────────────────
                        // Each layer header returns (rules_changed, texture_changed)
                        // so we can route the change to the right rebuild path.
                        let layer_names = ["Grass (R)", "Dirt (G)", "Rock (B)", "Snow (A)"];

                        for (i, name) in layer_names.iter().enumerate() {
                            let (lr, lt) = egui::CollapsingHeader::new(*name)
                                .default_open(false)
                                .show(ui, |ui| {
                                    let rule_ch = show_splat_rule(ui, &mut cfg.rules[i]);
                                    ui.separator();
                                    let tex_ch = match i {
                                        0 => show_ground_config(ui, &mut cfg.grass),
                                        1 => show_ground_config(ui, &mut cfg.dirt),
                                        2 => show_rock_config(ui, &mut cfg.rock),
                                        3 => show_ground_config(ui, &mut cfg.snow),
                                        _ => unreachable!(),
                                    };
                                    (rule_ch, tex_ch)
                                })
                                .body_returned
                                .unwrap_or((false, false));
                            rules |= lr;
                            tex |= lt;
                        }

                        (rules, tex)
                    })
                    .inner;

                rules_changed |= inner_rules;
                texture_changed |= inner_tex;

                ui.separator();

                // ── Status ────────────────────────────────────────────────
                match &status {
                    MaterialStatus::Idle => {
                        ui.label("Material: idle.");
                    }
                    MaterialStatus::GeneratingTextures => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(format!("Generating textures… ({layers_ready}/4)"));
                        });
                    }
                    MaterialStatus::Ready => {
                        ui.colored_label(egui::Color32::GREEN, "Material applied.");
                    }
                }
            });
    } // cfg borrow ends here; config is accessible again

    if rules_changed || texture_changed {
        config.set_changed();
    }
    // Splat rules changed → rebuild weight map immediately (cheap CPU pass).
    if rules_changed {
        mat_state.splat_dirty = true;
    }
    // Texture params changed → debounce to avoid saturating the thread pool
    // with abandoned generation tasks during continuous slider drags.
    if texture_changed {
        mat_state.texture_debounce_timer.reset();
        mat_state.texture_debounce_pending = true;
    }
}

// ---------------------------------------------------------------------------
// Sub-panels — all return `true` when the user edited something.
// ---------------------------------------------------------------------------

fn show_splat_rule(ui: &mut egui::Ui, rule: &mut SplatRuleParams) -> bool {
    ui.label(egui::RichText::new("Splat rule").strong());
    let mut ch = false;

    ui.horizontal(|ui| {
        ui.label("Height");
        ch |= ui
            .add(
                egui::Slider::new(&mut rule.height_min, 0.0f32..=1.0)
                    .prefix("min ")
                    .max_decimals(2),
            )
            .changed();
        ch |= ui
            .add(
                egui::Slider::new(&mut rule.height_max, 0.0f32..=1.0)
                    .prefix("max ")
                    .max_decimals(2),
            )
            .changed();
    });

    ui.horizontal(|ui| {
        ui.label("Slope");
        ch |= ui
            .add(
                egui::Slider::new(&mut rule.slope_min, 0.0f32..=1.0)
                    .prefix("min ")
                    .max_decimals(2),
            )
            .changed();
        ch |= ui
            .add(
                egui::Slider::new(&mut rule.slope_max, 0.0f32..=1.0)
                    .prefix("max ")
                    .max_decimals(2),
            )
            .changed();
    });

    ch |= f32_slider(ui, &mut rule.sharpness, "Sharpness", 0.5..=10.0);
    ch
}

fn show_ground_config(ui: &mut egui::Ui, c: &mut GroundConfig) -> bool {
    ui.label(egui::RichText::new("Texture (Ground)").strong());
    let mut ch = false;

    ch |= u32_drag(ui, &mut c.seed, "Seed");
    ch |= f64_slider(ui, &mut c.macro_scale, "Macro scale", 0.5..=8.0);
    ch |= usize_slider(ui, &mut c.macro_octaves, "Macro octaves", 1..=8);
    ch |= f64_slider(ui, &mut c.micro_scale, "Micro scale", 2.0..=20.0);
    ch |= usize_slider(ui, &mut c.micro_octaves, "Micro octaves", 1..=6);
    ch |= f64_slider(ui, &mut c.micro_weight, "Micro weight", 0.0..=1.0);

    ui.horizontal(|ui| {
        ui.label("Color dry");
        ch |= ui.color_edit_button_rgb(&mut c.color_dry).changed();
    });

    ui.horizontal(|ui| {
        ui.label("Color moist");
        ch |= ui.color_edit_button_rgb(&mut c.color_moist).changed();
    });

    ch |= f32_slider(ui, &mut c.normal_strength, "Normal strength", 0.0..=8.0);
    ch
}

fn show_rock_config(ui: &mut egui::Ui, c: &mut RockConfig) -> bool {
    ui.label(egui::RichText::new("Texture (Rock)").strong());
    let mut ch = false;

    ch |= u32_drag(ui, &mut c.seed, "Seed");
    ch |= f64_slider(ui, &mut c.scale, "Scale", 0.5..=12.0);
    ch |= usize_slider(ui, &mut c.octaves, "Octaves", 1..=12);
    ch |= f64_slider(ui, &mut c.attenuation, "Attenuation", 0.5..=6.0);

    ui.horizontal(|ui| {
        ui.label("Color gaps");
        ch |= ui.color_edit_button_rgb(&mut c.color_light).changed();
    });

    ui.horizontal(|ui| {
        ui.label("Color stone");
        ch |= ui.color_edit_button_rgb(&mut c.color_dark).changed();
    });

    ch |= f32_slider(ui, &mut c.normal_strength, "Normal strength", 0.0..=8.0);
    ch
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn f32_slider(
    ui: &mut egui::Ui,
    val: &mut f32,
    label: &str,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(val, range)).changed()
    })
    .inner
}

fn f64_slider(
    ui: &mut egui::Ui,
    val: &mut f64,
    label: &str,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(val, range)).changed()
    })
    .inner
}

fn usize_slider(
    ui: &mut egui::Ui,
    val: &mut usize,
    label: &str,
    range: std::ops::RangeInclusive<usize>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(val, range)).changed()
    })
    .inner
}

fn u32_drag(ui: &mut egui::Ui, val: &mut u32, label: &str) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(val).speed(1.0)).changed()
    })
    .inner
}
