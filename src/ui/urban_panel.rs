//! Urban Planner GUI window.
//!
//! Exposes all [`UrbanConfig`] parameters: tensor-field road generation
//! (step size, major/minor road distance, snap radius), road rendering
//! (width, spline resolution, hub segments, 3D toggle), block and lot
//! subdivision settings, gizmo visibility toggles, and an embedded asphalt
//! material editor for road surfaces.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use bevy_symbios_texture::ui::asphalt_config_editor;

use crate::core::config::{DirtyFlags, TerrainDebounce};
use crate::core::urban_config::{CurrentBuildingLots, CurrentRoadGraph, RoadMaterialState, UrbanConfig};

/// Render the "Urban Planner" egui window.
pub fn render_urban_ui(
    mut contexts: EguiContexts,
    mut config: ResMut<UrbanConfig>,
    mut dirty: ResMut<DirtyFlags>,
    mut debounce: ResMut<TerrainDebounce>,
    mut road_mat_state: ResMut<RoadMaterialState>,
    current_rg: Res<CurrentRoadGraph>,
    current_lots: Res<CurrentBuildingLots>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let mut texture_changed = false;

    egui::Window::new("Urban Planner")
        .default_width(300.0)
        .default_pos((710.0, 10.0))
        .show(ctx, |ui| {
            let mut changed = false;

            changed |= ui
                .checkbox(&mut config.enabled, "Enable Urban Generation")
                .changed();

            ui.add_enabled_ui(config.enabled, |ui| {
                // ── Roads ────────────────────────────────────────────
                ui.separator();
                egui::CollapsingHeader::new("Roads")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut config.show_gizmos, "Show Road Gizmos");
                        ui.checkbox(&mut config.render_roads, "Render 3D Roads");
                        changed |= slider(ui, &mut config.tensor.step_size, "Step Size", 0.5..=15.0);
                        changed |= slider(
                            ui,
                            &mut config.tensor.major_road_dist,
                            "Major Road Dist",
                            10.0..=300.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.tensor.minor_road_dist,
                            "Minor Road Dist",
                            5.0..=150.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.tensor.snap_radius,
                            "Snap Radius",
                            1.0..=30.0,
                        );
                        changed |= slider(ui, &mut config.road_width, "Road Width", 0.5..=10.0);
                        changed |= slider(
                            ui,
                            &mut config.road_blend_radius,
                            "Blend Radius",
                            0.0..=30.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.road_resolution,
                            "Spline Resolution",
                            1.0..=32.0,
                        );
                        changed |= ui.add(
                            egui::Slider::new(&mut config.hub_segments, 3..=32)
                                .text("Hub Segments"),
                        ).changed();

                        // Embankment skirts
                        changed |= slider(
                            ui,
                            &mut config.skirt_width,
                            "Skirt Width",
                            0.0..=10.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.skirt_bury_depth,
                            "Skirt Bury Depth",
                            0.0..=5.0,
                        );

                        // Rationalization controls
                        egui::CollapsingHeader::new("Rationalization")
                            .default_open(false)
                            .show(ui, |ui| {
                                changed |= ui
                                    .checkbox(&mut config.rationalize.enabled, "Enable Rationalization")
                                    .changed();
                                ui.add_enabled_ui(config.rationalize.enabled, |ui| {
                                    changed |= slider(
                                        ui,
                                        &mut config.rationalize.rdp_tolerance,
                                        "RDP Tolerance",
                                        0.1..=10.0,
                                    );
                                    changed |= slider(
                                        ui,
                                        &mut config.rationalize.major_fillet_radius,
                                        "Major Fillet Radius",
                                        0.0..=50.0,
                                    );
                                    changed |= slider(
                                        ui,
                                        &mut config.rationalize.minor_fillet_radius,
                                        "Minor Fillet Radius",
                                        0.0..=30.0,
                                    );
                                    changed |= ui.add(
                                        egui::Slider::new(&mut config.rationalize.fillet_segments, 1..=16)
                                            .text("Fillet Segments"),
                                    ).changed();
                                });
                            });

                        // Road material editor
                        egui::CollapsingHeader::new("Road Material")
                            .default_open(false)
                            .show(ui, |ui| {
                                let id = egui::Id::new(("urban", "road_material"));
                                let (_wb, regen) = asphalt_config_editor(ui, &mut config.road_material, id);
                                texture_changed |= regen;
                            });
                    });

                // ── Blocks ──────────────────────────────────────────
                ui.separator();
                egui::CollapsingHeader::new("Blocks")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut config.show_block_gizmos, "Show Block Outlines");
                        ui.checkbox(&mut config.show_block_centroids, "Show Block Centroids");

                        let block_count = current_rg.0.as_ref().map_or(0, |g| g.blocks.len());
                        ui.label(format!("Blocks: {block_count}"));
                    });

                // ── Building Lots ────────────────────────────────────
                ui.separator();
                egui::CollapsingHeader::new("Building Lots")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut config.show_lot_gizmos, "Show Lot Footprints");

                        changed |= slider(
                            ui,
                            &mut config.lot.max_lot_area,
                            "Max Lot Area",
                            100.0..=2000.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.lot.min_lot_area,
                            "Min Lot Area",
                            10.0..=200.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.lot.front_setback,
                            "Front Setback",
                            0.0..=10.0,
                        );
                        changed |=
                            slider(ui, &mut config.lot.side_setback, "Side Setback", 0.0..=5.0);
                        changed |=
                            slider(ui, &mut config.lot.rear_setback, "Rear Setback", 0.0..=10.0);
                        changed |=
                            slider(ui, &mut config.lot_blend_radius, "Blend Radius", 0.0..=30.0);

                        ui.label(format!("Lots: {}", current_lots.0.len()));
                    });
            });

            if changed {
                debounce.timer.reset();
                debounce.pending = true;
                dirty.terrain = false;
            }
        });

    if texture_changed {
        config.set_changed();
        road_mat_state.texture_debounce_timer.reset();
        road_mat_state.texture_debounce_pending = true;
    }
}

fn slider(
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
