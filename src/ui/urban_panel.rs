use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::core::config::{DirtyFlags, TerrainDebounce};
use crate::core::urban_config::{CurrentBuildingLots, CurrentRoadGraph, UrbanConfig};

/// Render the "Urban Planner" egui window.
pub fn render_urban_ui(
    mut contexts: EguiContexts,
    mut config: ResMut<UrbanConfig>,
    mut dirty: ResMut<DirtyFlags>,
    mut debounce: ResMut<TerrainDebounce>,
    current_rg: Res<CurrentRoadGraph>,
    current_lots: Res<CurrentBuildingLots>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

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
                        changed |= slider(ui, &mut config.tensor.step_size, "Step Size", 0.5..=5.0);
                        changed |= slider(
                            ui,
                            &mut config.tensor.major_road_dist,
                            "Major Road Dist",
                            10.0..=100.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.tensor.minor_road_dist,
                            "Minor Road Dist",
                            5.0..=50.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.tensor.snap_radius,
                            "Snap Radius",
                            1.0..=10.0,
                        );
                        changed |= slider(ui, &mut config.road_width, "Road Width", 0.5..=10.0);
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
