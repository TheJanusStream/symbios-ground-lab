use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use bevy_panorbit_camera::PanOrbitCameraPlugin;

use symbios_ground_lab::core::config::{
    CurrentHeightMap, DirtyFlags, DirtyMesh, ErosionVizState, ExportStatus, ExportTask,
    GenerationTask, TerrainConfig, TerrainDebounce,
};
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
        // ── Startup ───────────────────────────────────────────────────────
        .add_systems(
            Startup,
            (visuals::scene::setup_scene, visuals::terrain::spawn_terrain).chain(),
        )
        // ── UI (egui pass) ────────────────────────────────────────────────
        .add_systems(EguiPrimaryContextPass, ui::panel::render_ui)
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
            )
                .chain(),
        )
        .run();
}
