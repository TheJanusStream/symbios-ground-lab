use crate::core::architecture_config::ArchitectureConfig;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

pub fn render_architecture_ui(mut contexts: EguiContexts, mut config: ResMut<ArchitectureConfig>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("Architect")
        .default_width(400.0)
        .default_pos((10.0, 500.0)) // Positioned below Terraformer
        .vscroll(true)
        .show(ctx, |ui| {
            ui.checkbox(&mut config.enabled, "Enable Architecture");

            if !config.enabled {
                return;
            }

            ui.separator();
            ui.heading("Grammar Source");
            ui.label("Edit the CGA rules below. Changes trigger a rebuild.");

            // Code Editor
            egui::ScrollArea::vertical()
                .min_scrolled_height(200.0)
                .max_height(400.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut config.grammar_source)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });

            ui.separator();
            ui.heading("Material Styles");

            // ── Material Configs ──────────────────────────────────────────
            // We use simple manual sliders here to ensure robustness without
            // relying on internal UI helpers from bevy_symbios_texture.

            egui::CollapsingHeader::new("Brick (Main Facade)")
                .default_open(false)
                .show(ui, |ui| {
                    color_edit(ui, &mut config.brick.color_brick, "Brick Color");
                    ui.add(
                        egui::Slider::new(&mut config.brick.scale, 1.0..=20.0)
                            .text("Texture Scale"),
                    );
                    ui.add(
                        egui::Slider::new(&mut config.brick.roughness, 0.0..=1.0).text("Roughness"),
                    );
                });

            egui::CollapsingHeader::new("Stucco (Upper Facade)")
                .default_open(false)
                .show(ui, |ui| {
                    color_edit(ui, &mut config.stucco.color_base, "Base Color");
                    ui.add(
                        egui::Slider::new(&mut config.stucco.roughness, 0.0..=1.0)
                            .text("Roughness"),
                    );
                });

            egui::CollapsingHeader::new("Concrete (Trim/Frames)")
                .default_open(false)
                .show(ui, |ui| {
                    color_edit(ui, &mut config.concrete.color_base, "Color");
                    ui.add(
                        egui::Slider::new(&mut config.concrete.formwork_lines, 0.0..=10.0)
                            .text("Formwork Lines"),
                    );
                });

            egui::CollapsingHeader::new("Shingle (Roof)")
                .default_open(false)
                .show(ui, |ui| {
                    color_edit(ui, &mut config.shingle.color_tile, "Tile Color");
                    ui.add(egui::Slider::new(&mut config.shingle.scale, 1.0..=20.0).text("Scale"));
                });

            egui::CollapsingHeader::new("Wood (Doors/Decks)")
                .default_open(false)
                .show(ui, |ui| {
                    color_edit(ui, &mut config.wood.color_wood_light, "Light Wood");
                    color_edit(ui, &mut config.wood.color_wood_dark, "Dark Wood");
                });

            egui::CollapsingHeader::new("Glass (Windows)")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add(
                        egui::Slider::new(&mut config.glass.glass_opacity, 0.0..=1.0)
                            .text("Opacity"),
                    );
                    ui.add(
                        egui::Slider::new(&mut config.glass.grime_level, 0.0..=1.0).text("Grime Level"),
                    );
                });

            egui::CollapsingHeader::new("Metal (Fascia/Gutters)")
                .default_open(false)
                .show(ui, |ui| {
                    color_edit(ui, &mut config.metal.color_metal, "Base Color");
                    ui.add(
                        egui::Slider::new(&mut config.metal.metallic, 0.0..=1.0).text("Metallic"),
                    );
                    ui.add(
                        egui::Slider::new(&mut config.metal.roughness, 0.0..=1.0)
                            .text("Roughness"),
                    );
                });
        });
}

fn color_edit(ui: &mut egui::Ui, rgb: &mut [f32; 3], label: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.color_edit_button_rgb(rgb);
    });
}
