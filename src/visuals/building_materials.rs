use crate::core::architecture_config::ArchitectureConfig;
use bevy::prelude::*;
use bevy_symbios_shape::ShapeRegistry;
use bevy_symbios_texture::async_gen::PendingTexture;

/// Component to link a pending building texture to a ShapeRegistry key
#[derive(Component)]
pub struct PendingBuildingTexture {
    pub key: String,
    pub material_handle: Handle<StandardMaterial>,
}

pub fn setup_building_materials(
    mut commands: Commands,
    mut registry: ResMut<ShapeRegistry>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<ArchitectureConfig>,
) {
    // Helper to spawn a texture task and register the material
    let mut spawn_mat = |key: &str, material: StandardMaterial, pending: PendingTexture| {
        let handle = materials.add(material);
        registry.register_material(key, handle.clone());
        commands.spawn((
            pending,
            PendingBuildingTexture {
                key: key.to_string(),
                material_handle: handle,
            },
        ));
    };

    // Brick
    spawn_mat(
        "Brick",
        StandardMaterial {
            base_color: Color::srgb_from_array(config.brick.color_brick),
            perceptual_roughness: 0.9,
            ..default()
        },
        PendingTexture::brick(config.brick.clone(), 1024, 1024),
    );

    // Stucco
    spawn_mat(
        "Stucco",
        StandardMaterial {
            base_color: Color::srgb_from_array(config.stucco.color_base),
            perceptual_roughness: 0.8,
            ..default()
        },
        PendingTexture::stucco(config.stucco.clone(), 1024, 1024),
    );

    // Concrete
    spawn_mat(
        "Concrete",
        StandardMaterial {
            base_color: Color::srgb(0.6, 0.6, 0.6),
            perceptual_roughness: 0.85,
            ..default()
        },
        PendingTexture::concrete(config.concrete.clone(), 512, 512),
    );

    // Shingle
    spawn_mat(
        "Shingle",
        StandardMaterial {
            base_color: Color::srgb_from_array(config.shingle.color_tile),
            perceptual_roughness: 0.8,
            ..default()
        },
        PendingTexture::shingle(config.shingle.clone(), 1024, 1024),
    );

    // Wood
    spawn_mat(
        "Wood",
        StandardMaterial {
            base_color: Color::srgb_from_array(config.wood.color_wood_light),
            perceptual_roughness: 0.6,
            ..default()
        },
        PendingTexture::plank(config.wood.clone(), 512, 512),
    );

    // Metal
    spawn_mat(
        "Metal",
        StandardMaterial {
            base_color: Color::srgb_from_array(config.metal.color_metal),
            metallic: 0.85,
            perceptual_roughness: 0.3,
            ..default()
        },
        PendingTexture::metal(config.metal.clone(), 512, 512),
    );

    // Glass (Windows)
    spawn_mat(
        "Glass",
        StandardMaterial {
            base_color: Color::srgba(0.1, 0.2, 0.3, 0.3),
            metallic: 0.9,
            perceptual_roughness: 0.05,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        PendingTexture::window(config.glass.clone(), 512, 512),
    );

    // Pavers (Driveway) -- reuse concrete gen for now or add pavers if available
    // Assuming PaversConfig is available or we substitute. Let's use concrete for simplicity
    // or if PaversConfig is in ArchitectureConfig, use it.
    // (Skipped PaversConfig in ArchitectureConfig struct above for brevity, let's substitute Concrete)
    spawn_mat(
        "Pavers",
        StandardMaterial {
            base_color: Color::srgb(0.5, 0.48, 0.45),
            perceptual_roughness: 0.85,
            ..default()
        },
        PendingTexture::concrete(ConcreteConfig::default(), 512, 512),
    );

    // Register Stretch Meshes
    registry.register_stretch_mesh("Pane");
    registry.register_stretch_mesh("Door");
    registry.register_stretch_mesh("GDoor");
}

// System to apply textures when ready
use bevy_symbios_texture::async_gen::TextureReady;
use bevy_symbios_texture::concrete::ConcreteConfig;

pub fn apply_building_textures(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &PendingBuildingTexture, &TextureReady)>,
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
