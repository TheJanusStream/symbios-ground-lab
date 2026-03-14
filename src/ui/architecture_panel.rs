//! Architect GUI window.
//!
//! Provides an enable toggle, a max-buildings slider, a live CGA grammar
//! editor (monospace text area), and collapsible per-material texture config
//! sections for each building material (Brick, Stucco, Concrete, Shingle,
//! Wood, Glass, Metal). Texture parameter changes are debounced through
//! [`ArchitectureMaterialState`].

use crate::core::architecture_config::{ArchitectureConfig, ArchitectureMaterialState};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use bevy_symbios_texture::ui::{
    brick_config_editor, concrete_config_editor, metal_config_editor, plank_config_editor,
    shingle_config_editor, stucco_config_editor, window_config_editor,
};

/// Render the "Architect" egui window.
///
/// Runs every frame in the [`EguiPrimaryContextPass`] schedule. Provides an
/// enable toggle, max-buildings slider, a live CGA grammar text editor, and
/// collapsible material config sections. Texture parameter changes are routed
/// through [`ArchitectureMaterialState`] debounce.
pub fn render_architecture_ui(
    mut contexts: EguiContexts,
    mut config: ResMut<ArchitectureConfig>,
    mut mat_state: ResMut<ArchitectureMaterialState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let mut texture_changed = false;

    {
        let cfg = config.bypass_change_detection();

        egui::Window::new("Architect")
            .default_width(400.0)
            .default_pos((10.0, 500.0))
            .vscroll(true)
            .show(ctx, |ui| {
                let prev_enabled = cfg.enabled;
                ui.checkbox(&mut cfg.enabled, "Enable Architecture");
                if cfg.enabled != prev_enabled {
                    texture_changed = true;
                }

                if !cfg.enabled {
                    return;
                }

                ui.separator();
                ui.add(egui::Slider::new(&mut cfg.max_buildings, 0..=1000).text("Max Buildings"));

                ui.separator();
                ui.heading("Grammar Source");
                ui.label("Edit the CGA rules below. Changes trigger a rebuild.");

                egui::ScrollArea::vertical()
                    .min_scrolled_height(200.0)
                    .max_height(400.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut cfg.grammar_source)
                                .font(egui::TextStyle::Monospace)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });

                ui.separator();
                ui.heading("Material Styles");

                /// Returns the `regen` flag from a config editor shown inside a
                /// collapsing header. `false` when the header is collapsed.
                macro_rules! mat_section {
                    ($ui:expr, $label:expr, $editor:expr) => {
                        egui::CollapsingHeader::new($label)
                            .default_open(false)
                            .show($ui, |ui| {
                                let id = egui::Id::new(("arch", $label));
                                $editor(ui, id).1
                            })
                            .body_returned
                            .unwrap_or(false)
                    };
                }

                texture_changed |=
                    mat_section!(ui, "Brick (Main Facade)", |ui: &mut egui::Ui, id| {
                        brick_config_editor(ui, &mut cfg.brick, id)
                    });
                texture_changed |=
                    mat_section!(ui, "Stucco (Upper Facade)", |ui: &mut egui::Ui, id| {
                        stucco_config_editor(ui, &mut cfg.stucco, id)
                    });
                texture_changed |=
                    mat_section!(ui, "Concrete (Trim/Frames)", |ui: &mut egui::Ui, id| {
                        concrete_config_editor(ui, &mut cfg.concrete, id)
                    });
                texture_changed |= mat_section!(ui, "Shingle (Roof)", |ui: &mut egui::Ui, id| {
                    shingle_config_editor(ui, &mut cfg.shingle, id)
                });
                texture_changed |=
                    mat_section!(ui, "Wood (Doors/Decks)", |ui: &mut egui::Ui, id| {
                        plank_config_editor(ui, &mut cfg.wood, id)
                    });
                texture_changed |= mat_section!(ui, "Glass (Windows)", |ui: &mut egui::Ui, id| {
                    window_config_editor(ui, &mut cfg.glass, id)
                });
                texture_changed |=
                    mat_section!(ui, "Metal (Fascia/Gutters)", |ui: &mut egui::Ui, id| {
                        metal_config_editor(ui, &mut cfg.metal, id)
                    });
            });
    }

    if texture_changed {
        config.set_changed();
        mat_state.texture_debounce_timer.reset();
        mat_state.texture_debounce_pending = true;
    }
}
