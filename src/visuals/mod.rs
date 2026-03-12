//! Bevy scene, mesh management, material pipeline, gizmos, and export.
//!
//! - [`scene`] — camera and lighting setup.
//! - [`terrain`] — spawns the [`TerrainMesh`](terrain::TerrainMesh) entity and
//!   rebuilds its mesh whenever [`CurrentHeightMap`](crate::core::config::CurrentHeightMap) changes.
//! - [`droplets`] — gizmo rendering for active erosion-visualisation droplets.
//! - [`material`] — splat material pipeline: detects dirty flags, spawns async
//!   texture tasks, collects results, and uploads the weight map to the GPU.
//! - [`splat_material`] — the [`SplatExtension`](splat_material::SplatExtension)
//!   `MaterialExtension` type and its GPU bindings.
//! - [`water_material`] — the [`WaterExtension`](water_material::WaterExtension)
//!   `MaterialExtension` type for the animated water surface shader.
//! - [`export`] — PNG heightmap, OBJ mesh, and JSON config export (native + WASM).
//! - [`buildings`] — CGA grammar-driven procedural building generation on
//!   subdivided lots; spawns hierarchical shape entities via `bevy_symbios_shape`.
//! - [`building_materials`] — async procedural texture pipeline for building
//!   facades (brick, stucco, concrete, shingle, wood, glass, metal).
//! - [`roads`] — 3D road mesh generation (hub polygons + spline ribbons) from
//!   the tensor-field road graph.
//! - [`road_materials`] — async asphalt texture pipeline for road surfaces.
//! - [`urban_gizmos`] — debug gizmo overlays for road edges, city block
//!   perimeters/centroids, and building lot footprints.

pub mod building_materials;
pub mod buildings;
pub mod droplets;
pub mod export;
pub mod material;
pub mod road_materials;
pub mod roads;
pub mod scene;
pub mod splat_material;
pub mod terrain;
pub mod urban_gizmos;
pub mod water_material;
