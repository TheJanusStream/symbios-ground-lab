use bevy::color::palettes::css::WHITE;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

pub fn setup_scene(mut commands: Commands) {
    // Directional light (warm sun from the upper-right)
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
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
        Bloom::NATURAL, // Enable Bloom
    ));
}
