//! Camera and lighting setup.
//!
//! [`setup_scene`] runs once at [`Startup`] and spawns a warm directional sun,
//! ambient fill light, and a [`PanOrbitCamera`] centred on the world origin.

use bevy::color::palettes::css::WHITE;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

/// Spawn the initial scene: a warm directional sun, ambient fill light, and
/// an orbit camera centred on the world origin.
///
/// Runs once at [`Startup`] before [`spawn_terrain`](super::terrain::spawn_terrain).
/// The camera focus is `Vec3::ZERO`; the terrain mesh is always centred there
/// regardless of its grid dimensions.
pub fn setup_scene(mut commands: Commands) {
    // Directional light (warm sun from the upper-right)
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            // Shadows disabled: each shadow cascade consumes texture units.
            // WebGL 2 guarantees only 16 texture image units per draw call;
            // keeping shadows off ensures we stay well under that limit.
            shadows_enabled: false,
            color: Color::srgb(1.0, 0.96, 0.88),
            ..default()
        },
        Transform::from_rotation(
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)
                .mul_quat(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_6)),
        ),
    ));

    // ambient light
    commands.insert_resource(GlobalAmbientLight {
        color: WHITE.into(),
        brightness: 300.0,
        ..default()
    });

    // Camera – orbit centred on the terrain, tilted 30° down
    commands.spawn((
        PanOrbitCamera {
            focus: Vec3::ZERO,
            radius: Some(300.0),
            pitch: Some(std::f32::consts::FRAC_PI_6),
            yaw: Some(std::f32::consts::FRAC_PI_4),
            button_orbit: MouseButton::Right,
            button_pan: MouseButton::Middle,
            ..default()
        },
        Camera3d::default(),
        // Bloom disabled: uses additional render passes that can exceed
        // WebGL 2 resource limits on low-end devices.
    ));
}
