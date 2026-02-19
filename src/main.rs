use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_symbios_texture::SymbiosTexturePlugin;

use symbios_ground_lab::core::config::{
    CurrentHeightMap, DirtyFlags, DirtyMesh, ErosionVizState, ExportStatus, ExportTask,
    GenerationTask, TerrainConfig, TerrainDebounce,
};
use symbios_ground_lab::core::material_config::{MaterialConfig, MaterialState};
use symbios_ground_lab::{logic, ui, visuals};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Symbios Ground Lab – The Terraformer".into(),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
            PanOrbitCameraPlugin,
            SymbiosTexturePlugin,
        ))
        // ── Resources ────────────────────────────────────────────────────
        .init_resource::<TerrainConfig>()
        .init_resource::<DirtyFlags>()
        .init_resource::<TerrainDebounce>()
        .init_resource::<CurrentHeightMap>()
        .init_resource::<GenerationTask>()
        .init_resource::<DirtyMesh>()
        .init_resource::<ExportStatus>()
        .init_resource::<ExportTask>()
        .init_resource::<ErosionVizState>()
        .init_resource::<MaterialConfig>()
        .init_resource::<MaterialState>()
        // ── Startup ───────────────────────────────────────────────────────
        .add_systems(
            Startup,
            (visuals::scene::setup_scene, visuals::terrain::spawn_terrain).chain(),
        )
        // ── UI (egui pass) ────────────────────────────────────────────────
        .add_systems(
            EguiPrimaryContextPass,
            (ui::panel::render_ui, ui::material_panel::render_material_ui),
        )
        // ── Update ────────────────────────────────────────────────────────
        .add_systems(
            Update,
            (
                logic::generation::start_generation,
                logic::generation::poll_generation,
                logic::erosion_viz::step_erosion_viz,
                visuals::terrain::rebuild_terrain,
                visuals::droplets::draw_droplet_gizmos,
                visuals::export::poll_export_task,
                // Material pipeline runs after terrain so the heightmap is fresh.
                visuals::material::detect_material_dirty,
                visuals::material::start_texture_tasks,
                visuals::material::collect_texture_results,
                visuals::material::bake_and_apply_material,
            )
                .chain(),
        )
        .run();
}
