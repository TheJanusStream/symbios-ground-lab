use bevy::prelude::*;
use bevy_symbios_texture::async_gen::{PendingTexture, TextureReady};

use crate::core::urban_config::{RoadMaterialState, UrbanConfig};

/// Marker for the road material handle so we can find it later.
#[derive(Component)]
pub struct RoadMaterialHandle(pub Handle<StandardMaterial>);

/// Component linking a pending road texture to its material.
#[derive(Component)]
pub struct PendingRoadTexture {
    pub material_handle: Handle<StandardMaterial>,
}

/// Spawns the initial road material and kicks off the first async texture task.
pub fn setup_road_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<UrbanConfig>,
) {
    let handle = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.15, 0.15),
        perceptual_roughness: 0.8,
        depth_bias: 100_000.0,
        ..default()
    });

    commands.spawn(RoadMaterialHandle(handle.clone()));

    commands.spawn((
        PendingTexture::asphalt(config.road_material.clone(), 1024, 1024),
        PendingRoadTexture {
            material_handle: handle,
        },
    ));
}

/// Debounces road material config changes and re-spawns texture tasks.
pub fn regenerate_road_textures(
    mut commands: Commands,
    config: Res<UrbanConfig>,
    mut mat_state: ResMut<RoadMaterialState>,
    time: Res<Time>,
    pending_q: Query<Entity, With<PendingRoadTexture>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    handle_q: Query<&RoadMaterialHandle>,
) {
    if mat_state.texture_debounce_pending {
        mat_state.texture_debounce_timer.tick(time.delta());
        if mat_state.texture_debounce_timer.just_finished() {
            mat_state.texture_debounce_pending = false;
            mat_state.textures_dirty = true;
        }
    }

    if !mat_state.textures_dirty {
        return;
    }
    mat_state.textures_dirty = false;

    if !config.enabled || !config.render_roads {
        return;
    }

    // Cancel any in-flight tasks.
    for e in &pending_q {
        commands.entity(e).despawn();
    }

    let Ok(road_handle) = handle_q.single() else {
        return;
    };

    // Update base material properties immediately.
    if let Some(mat) = materials.get_mut(&road_handle.0) {
        mat.base_color = Color::srgb(0.15, 0.15, 0.15);
        mat.perceptual_roughness = 0.8;
        mat.depth_bias = 100_000.0;
    }

    commands.spawn((
        PendingTexture::asphalt(config.road_material.clone(), 1024, 1024),
        PendingRoadTexture {
            material_handle: road_handle.0.clone(),
        },
    ));
}

/// Applies completed texture results to the road material.
pub fn apply_road_textures(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &PendingRoadTexture, &TextureReady)>,
) {
    for (entity, pending, ready) in &query {
        if let Some(mat) = materials.get_mut(&pending.material_handle) {
            mat.base_color_texture = Some(ready.0.albedo.clone());
            mat.normal_map_texture = Some(ready.0.normal.clone());
            mat.metallic_roughness_texture = Some(ready.0.roughness.clone());
            mat.occlusion_texture = Some(ready.0.roughness.clone());
        }
        commands.entity(entity).despawn();
    }
}
