//! Material settings GUI window.
//!
//! Shows as a second egui window alongside the Terraformer panel.  Exposes
//! all [`MaterialConfig`] parameters: enable toggle, texture size, tile
//! scale, per-layer splat rules, and per-layer texture generator configs.
//!
//! Texture-specific parameter editors are provided by [`bevy_symbios_texture::ui`].
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
use bevy_symbios_texture::ui::{f32_slider, ground_config_editor, rock_config_editor};

use crate::core::material_config::{
    MaterialConfig, MaterialState, MaterialStatus, SplatRuleParams,
};

/// Render the "Materials" egui window.
///
/// Runs every frame in [`EguiPrimaryContextPass`].  Uses manual change
/// detection (`bypass_change_detection` / `set_changed`) to prevent Bevy's
/// `ResMut` auto-marking from cancelling in-flight texture tasks.  Splat
/// rule changes trigger an immediate weight-map rebuild; texture parameter
/// changes are routed through the debounce timer on [`MaterialState`].
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
            .default_open(false)
            .default_width(340.0)
            .default_pos((10.0, 50.0))
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
                                    let tex_id = egui::Id::new(("layer_tex", i));
                                    let (_, tex_ch) = match i {
                                        0 => ground_config_editor(ui, &mut cfg.grass, tex_id),
                                        1 => ground_config_editor(ui, &mut cfg.dirt, tex_id),
                                        2 => rock_config_editor(ui, &mut cfg.rock, tex_id),
                                        3 => ground_config_editor(ui, &mut cfg.snow, tex_id),
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
// Sub-panels specific to symbios-ground-lab
// ---------------------------------------------------------------------------

fn show_splat_rule(ui: &mut egui::Ui, rule: &mut SplatRuleParams) -> bool {
    ui.label(egui::RichText::new("Splat rule").strong());
    let mut ch = false;

    ui.horizontal(|ui| {
        ui.label("Height");
        let h_max = rule.height_max;
        ch |= ui
            .add(
                egui::Slider::new(&mut rule.height_min, 0.0f32..=h_max)
                    .prefix("min ")
                    .max_decimals(2),
            )
            .changed();
        let h_min = rule.height_min;
        ch |= ui
            .add(
                egui::Slider::new(&mut rule.height_max, h_min..=1.0)
                    .prefix("max ")
                    .max_decimals(2),
            )
            .changed();
    });

    ui.horizontal(|ui| {
        ui.label("Slope");
        let s_max = rule.slope_max;
        ch |= ui
            .add(
                egui::Slider::new(&mut rule.slope_min, 0.0f32..=s_max)
                    .prefix("min ")
                    .max_decimals(2),
            )
            .changed();
        let s_min = rule.slope_min;
        ch |= ui
            .add(
                egui::Slider::new(&mut rule.slope_max, s_min..=10.0)
                    .prefix("max ")
                    .max_decimals(2),
            )
            .changed();
    });

    ch |= f32_slider(ui, &mut rule.sharpness, "Sharpness", 0.5..=10.0);
    ch
}
