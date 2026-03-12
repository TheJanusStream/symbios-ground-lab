use bevy::{pbr::MaterialPlugin, prelude::*};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_symbios_shape::BevySymbiosShapePlugin;
use bevy_symbios_texture::SymbiosTexturePlugin;

use symbios_ground_lab::core::architecture_config::{ArchitectureConfig, ArchitectureMaterialState};
use symbios_ground_lab::core::config::{
    CurrentHeightMap, DirtyFlags, DirtyMesh, ErosionVizState, ExportStatus, ExportTask,
    GenerationTask, TerrainConfig, TerrainDebounce,
};
use symbios_ground_lab::core::material_config::{MaterialConfig, MaterialState};
use symbios_ground_lab::core::urban_config::{
    CurrentBuildingLots, CurrentRoadGraph, RoadMaterialState, UrbanConfig,
};
use symbios_ground_lab::visuals::splat_material::SplatTerrainMaterial;
use symbios_ground_lab::visuals::water_material::WaterMaterial;
use symbios_ground_lab::{logic, ui, visuals};

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

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
            BevySymbiosShapePlugin,
            MaterialPlugin::<SplatTerrainMaterial>::default(),
            MaterialPlugin::<WaterMaterial>::default(),
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
        .init_resource::<UrbanConfig>()
        .init_resource::<CurrentRoadGraph>()
        .init_resource::<CurrentBuildingLots>()
        .init_resource::<ArchitectureConfig>()
        .init_resource::<ArchitectureMaterialState>()
        .init_resource::<RoadMaterialState>()
        // ── Startup ───────────────────────────────────────────────────────
        .add_systems(
            Startup,
            (
                visuals::scene::setup_scene,
                visuals::terrain::spawn_terrain,
                visuals::building_materials::setup_building_materials,
                visuals::road_materials::setup_road_materials,
            )
                .chain(),
        )
        // ── UI (egui pass) ────────────────────────────────────────────────
        .add_systems(
            EguiPrimaryContextPass,
            (
                ui::panel::render_ui,
                ui::material_panel::render_material_ui,
                ui::urban_panel::render_urban_ui,
                ui::architecture_panel::render_architecture_ui,
            ),
        )
        // ── Update ────────────────────────────────────────────────────────
        .add_systems(
            Update,
            (
                logic::generation::start_generation,
                logic::generation::poll_generation,
                logic::erosion_viz::poll_viz_init,
                logic::erosion_viz::step_erosion_viz,
                visuals::terrain::rebuild_terrain,
                visuals::droplets::draw_droplet_gizmos,
                visuals::urban_gizmos::draw_road_gizmos,
                visuals::urban_gizmos::draw_block_gizmos,
                visuals::urban_gizmos::draw_lot_gizmos,
                visuals::export::poll_export_task,
                // Material pipeline runs after terrain so the heightmap is fresh.
                visuals::material::detect_material_dirty,
                visuals::material::start_texture_tasks,
                visuals::material::collect_texture_results,
                visuals::material::apply_splat_material,
                visuals::building_materials::regenerate_building_textures,
                visuals::buildings::rebuild_buildings,
                visuals::building_materials::apply_building_textures,
                visuals::road_materials::regenerate_road_textures,
                visuals::roads::rebuild_roads,
                visuals::road_materials::apply_road_textures,
            )
                .chain(),
        )
        .run();
}
