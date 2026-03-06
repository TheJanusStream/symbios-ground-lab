use crate::core::architecture_config::ArchitectureConfig;
use crate::core::config::{CurrentHeightMap, DirtyFlags};
use crate::core::urban_config::CurrentBuildingLots;
use bevy::prelude::*;
use bevy_symbios_shape::{ShapeRegistry, SpawnShapeExt};
use symbios_shape::{Interpreter, Quat as DQuat, Scope, Vec3 as DVec3, grammar::parse_rule};

#[derive(Component)]
pub struct BuildingRoot;

pub fn rebuild_buildings(
    mut commands: Commands,
    lots: Res<CurrentBuildingLots>,
    arch_config: Res<ArchitectureConfig>,
    registry: Res<ShapeRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut _dirty: ResMut<DirtyFlags>, // Check specific arch dirty flag if we add one, or repurpose
    existing_q: Query<Entity, With<BuildingRoot>>,
    hm: Res<CurrentHeightMap>,
) {
    // Only run if architecture is enabled and lots exist
    if !arch_config.enabled || lots.0.is_empty() {
        return;
    }

    // We need a trigger. Let's say if lots changed OR arch_config changed.
    // Ideally we add `DirtyFlags::architecture`.
    // For now, let's assume we run when `lots` is changed.
    if !lots.is_changed() && !arch_config.is_changed() {
        return;
    }

    // 1. Cleanup
    for e in &existing_q {
        commands.entity(e).despawn();
    }

    // 2. Parse Grammar
    let mut interp = Interpreter::new();
    for line in arch_config.grammar_source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if let Ok(rule) = parse_rule(line) {
            interp.add_weighted_rules(&rule.name, rule.variants).ok();
        } else {
            warn!("Failed to parse rule: {}", line);
        }
    }

    // 3. Spawn Buildings
    let Some(ref heightmap) = hm.0 else { return };

    // Optimization: Cap at 50 buildings for stability during dev
    let max_buildings = 100;

    for (i, lot) in lots.0.iter().take(max_buildings).enumerate() {
        // Set seed for stability
        interp.seed = (i as u64) * 12345 + 99;

        // Coordinate Conversion
        // Lot.position is CENTER. Scope.position is CORNER (local (0,0,0)).
        // Bevy Y is Up. Lot is X/Y (2D). Shape is X/Y/Z (3D).
        // Lot Rot is around Y.

        let center_x = lot.position.x;
        let center_z = lot.position.y; // 2D Y is 3D Z

        // Get terrain height at center
        let ground_y = heightmap.get_height_at(center_x, center_z);

        // Construct Rotation quaternion
        let rot = DQuat::from_rotation_y(-lot.rotation as f64);

        // Calculate Corner Offset: -Width/2, 0, -Depth/2
        // Note: Lot width/depth are f32, Scope needs f64
        let half_size = DVec3::new(lot.width as f64 * 0.5, 0.0, lot.depth as f64 * 0.5);
        let center = DVec3::new(center_x as f64, ground_y as f64, center_z as f64);

        // Corner = Center - (Rotation * HalfSize)
        let corner = center - (rot * half_size);

        let scope = Scope::new(
            corner
                + DVec3::new(
                    -heightmap.world_width() as f64 * 0.5,
                    0.0,
                    -heightmap.world_depth() as f64 * 0.5,
                ), // Shift to world origin
            rot,
            DVec3::new(lot.width as f64, 0.0, lot.depth as f64), // Y=0, Extrude sets height
        );

        match commands.spawn_shape(
            &interp,
            scope,
            "Lot",
            &registry,
            &mut meshes,
            &mut materials,
        ) {
            Ok(e) => {
                commands.entity(e).insert(BuildingRoot);
            }
            Err(e) => warn!("Building derivation failed: {:?}", e),
        }
    }
}
