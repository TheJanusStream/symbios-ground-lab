use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::core::config::{
    CurrentHeightMap, DirtyFlags, ErosionVizState, ExportStatus, ExportTask, GenerationTask,
    GeneratorKind, TerrainConfig, TerrainDebounce,
};
use crate::logic::erosion_viz::start_erosion_viz;
use crate::visuals::export::{export_heightmap_png, export_json, spawn_obj_export};

#[allow(clippy::too_many_arguments)]
pub fn render_ui(
    mut contexts: EguiContexts,
    mut config: ResMut<TerrainConfig>,
    mut dirty: ResMut<DirtyFlags>,
    mut debounce: ResMut<TerrainDebounce>,
    mut viz: ResMut<ErosionVizState>,
    mut export_status: ResMut<ExportStatus>,
    mut export_task: ResMut<ExportTask>,
    task: Res<GenerationTask>,
    current_hm: Res<CurrentHeightMap>,
    time: Res<Time>,
) {
    // Tick debounce
    if debounce.pending {
        debounce.timer.tick(time.delta());
        if debounce.timer.is_finished() {
            dirty.terrain = true;
            debounce.pending = false;
        }
    }

    let is_generating = task.0.is_some();
    let viz_active = viz.enabled;

    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("Terraformer")
        .default_width(320.0)
        .show(ctx, |ui| {
            // ── Grid / World ──────────────────────────────────────────────
            egui::CollapsingHeader::new("Grid / World")
                .default_open(true)
                .show(ui, |ui| {
                    let mut changed = false;

                    ui.horizontal(|ui| {
                        ui.label("Grid size");
                        let prev = config.grid_size;
                        egui::ComboBox::from_id_salt("grid_size")
                            .selected_text(format!("{}×{}", config.grid_size, config.grid_size))
                            .show_ui(ui, |ui| {
                                for &s in &[64u32, 128, 256, 512, 1024, 2048] {
                                    if ui
                                        .selectable_label(config.grid_size == s, format!("{s}×{s}"))
                                        .clicked()
                                    {
                                        config.grid_size = s;
                                    }
                                }
                            });
                        if config.grid_size != prev {
                            changed = true;
                        }
                    });

                    changed |= slider(ui, &mut config.cell_scale, "Cell scale", 0.1..=4.0);
                    changed |= slider(ui, &mut config.height_scale, "Height scale", 1.0..=200.0);

                    if changed {
                        trigger_debounce(&mut debounce, &mut dirty);
                    }
                });

            ui.separator();

            // ── Generator ─────────────────────────────────────────────────
            egui::CollapsingHeader::new("Generator")
                .default_open(true)
                .show(ui, |ui| {
                    let mut changed = false;

                    // Algorithm picker
                    ui.horizontal(|ui| {
                        ui.label("Algorithm");
                        let prev = config.generator_kind;
                        egui::ComboBox::from_id_salt("generator_kind")
                            .selected_text(match config.generator_kind {
                                GeneratorKind::FbmNoise => "FBM Noise",
                                GeneratorKind::DiamondSquare => "Diamond Square",
                                GeneratorKind::VoronoiTerracing => "Voronoi Terracing",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut config.generator_kind,
                                    GeneratorKind::FbmNoise,
                                    "FBM Noise",
                                );
                                ui.selectable_value(
                                    &mut config.generator_kind,
                                    GeneratorKind::DiamondSquare,
                                    "Diamond Square",
                                );
                                ui.selectable_value(
                                    &mut config.generator_kind,
                                    GeneratorKind::VoronoiTerracing,
                                    "Voronoi Terracing",
                                );
                            });
                        if config.generator_kind != prev {
                            changed = true;
                        }
                    });

                    // Seed is shared by all generators
                    ui.horizontal(|ui| {
                        ui.label("Seed");
                        let prev = config.seed;
                        ui.add(egui::DragValue::new(&mut config.seed).speed(1.0));
                        if config.seed != prev {
                            changed = true;
                        }
                    });

                    // Algorithm-specific parameters
                    match config.generator_kind {
                        GeneratorKind::FbmNoise => {
                            changed |= int_slider(ui, &mut config.octaves, "Octaves", 1..=10);
                            changed |=
                                slider(ui, &mut config.persistence, "Persistence", 0.1..=0.9);
                            changed |= slider(ui, &mut config.lacunarity, "Lacunarity", 1.5..=4.0);
                            changed |=
                                slider(ui, &mut config.base_frequency, "Frequency", 0.5..=16.0);
                        }
                        GeneratorKind::DiamondSquare => {
                            changed |= slider(ui, &mut config.ds_roughness, "Roughness", 0.0..=1.0);
                        }
                        GeneratorKind::VoronoiTerracing => {
                            changed |= int_slider(
                                ui,
                                &mut config.voronoi_num_seeds,
                                "Seed points",
                                1..=1000,
                            );
                            changed |= int_slider(
                                ui,
                                &mut config.voronoi_num_terraces,
                                "Terraces",
                                1..=32,
                            );
                        }
                    }

                    if changed {
                        trigger_debounce(&mut debounce, &mut dirty);
                    }
                });

            ui.separator();

            // ── Hydraulic Erosion ─────────────────────────────────────────
            egui::CollapsingHeader::new("Hydraulic Erosion")
                .default_open(true)
                .show(ui, |ui| {
                    let mut changed = false;

                    changed |=
                        checkbox(ui, &mut config.erosion_enabled, "Enable hydraulic erosion");

                    ui.add_enabled_ui(config.erosion_enabled, |ui| {
                        changed |=
                            int_slider(ui, &mut config.erosion_drops, "Drops", 1_000..=500_000);
                        changed |= slider(ui, &mut config.inertia, "Inertia", 0.0..=0.5);
                        changed |= slider(ui, &mut config.erosion_rate, "Erosion rate", 0.01..=1.0);
                        changed |= slider(
                            ui,
                            &mut config.deposition_rate,
                            "Deposition rate",
                            0.01..=1.0,
                        );
                        changed |= slider(
                            ui,
                            &mut config.evaporation_rate,
                            "Evaporation rate",
                            0.001..=0.1,
                        );
                        changed |= slider(
                            ui,
                            &mut config.capacity_factor,
                            "Capacity factor",
                            1.0..=20.0,
                        );
                    });

                    if changed {
                        trigger_debounce(&mut debounce, &mut dirty);
                    }
                });

            ui.separator();

            // ── Thermal Erosion ───────────────────────────────────────────
            egui::CollapsingHeader::new("Thermal Erosion")
                .default_open(false)
                .show(ui, |ui| {
                    let mut changed = false;
                    changed |= checkbox(ui, &mut config.thermal_enabled, "Enable thermal erosion");

                    ui.add_enabled_ui(config.thermal_enabled, |ui| {
                        changed |=
                            int_slider(ui, &mut config.thermal_iterations, "Iterations", 1..=500);
                        changed |= slider(
                            ui,
                            &mut config.thermal_talus_angle,
                            "Talus angle",
                            0.001..=0.5,
                        );
                    });

                    if changed {
                        trigger_debounce(&mut debounce, &mut dirty);
                    }
                });

            ui.separator();

            // ── Status ────────────────────────────────────────────────────
            if is_generating {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Generating terrain…");
                });
            } else if viz_active {
                let pct = if viz.total > 0 {
                    viz.completed as f32 / viz.total as f32
                } else {
                    0.0
                };
                ui.label(format!(
                    "Erosion viz: {}/{} drops  ({} active)",
                    viz.completed,
                    viz.total,
                    viz.active.len()
                ));
                ui.add(egui::ProgressBar::new(pct).show_percentage());
            } else {
                ui.label("Ready.");
            }

            ui.separator();

            // ── Controls ──────────────────────────────────────────────────
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !is_generating && !viz_active,
                        egui::Button::new("Regenerate"),
                    )
                    .clicked()
                {
                    dirty.terrain = true;
                    debounce.pending = false;
                }

                if ui
                    .add_enabled(
                        !is_generating && !viz_active && config.erosion_enabled,
                        egui::Button::new("Visualise Erosion"),
                    )
                    .clicked()
                {
                    let cfg = config.clone();
                    start_erosion_viz(&cfg, &mut viz);
                }

                if viz_active && ui.button("Stop Viz").clicked() {
                    viz.enabled = false;
                }
            });

            ui.separator();

            // ── Export ────────────────────────────────────────────────────
            egui::CollapsingHeader::new("Export")
                .default_open(false)
                .show(ui, |ui| {
                    let has_hm = current_hm.0.is_some();
                    let is_exporting = matches!(&*export_status, ExportStatus::Exporting);
                    ui.add_enabled_ui(has_hm, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("PNG (16-bit heightmap)").clicked()
                                && let Some(hm) = &current_hm.0
                            {
                                export_heightmap_png(hm, &mut export_status);
                            }
                            if ui
                                .add_enabled(!is_exporting, egui::Button::new("OBJ mesh"))
                                .clicked()
                                && let Some(hm) = &current_hm.0
                            {
                                spawn_obj_export(hm.clone(), &mut export_task, &mut export_status);
                            }
                            if ui.button("JSON config").clicked() {
                                export_json(&config, current_hm.0.as_ref(), &mut export_status);
                            }
                        });
                    });

                    match &*export_status {
                        ExportStatus::Idle => {}
                        ExportStatus::Exporting => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Exporting OBJ…");
                            });
                        }
                        ExportStatus::Done(f) => {
                            ui.colored_label(egui::Color32::GREEN, format!("✓ Saved: {f}"));
                        }
                        ExportStatus::Error(e) => {
                            ui.colored_label(egui::Color32::RED, format!("✗ {e}"));
                        }
                    }
                });
        });
}

// ---------------------------------------------------------------------------
// UI helpers
// ---------------------------------------------------------------------------

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

fn int_slider<T>(
    ui: &mut egui::Ui,
    val: &mut T,
    label: &str,
    range: std::ops::RangeInclusive<T>,
) -> bool
where
    T: egui::emath::Numeric,
{
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(val, range)).changed()
    })
    .inner
}

fn checkbox(ui: &mut egui::Ui, val: &mut bool, label: &str) -> bool {
    ui.checkbox(val, label).changed()
}

fn trigger_debounce(debounce: &mut TerrainDebounce, dirty: &mut DirtyFlags) {
    debounce.timer.reset();
    debounce.pending = true;
    dirty.terrain = false;
}
