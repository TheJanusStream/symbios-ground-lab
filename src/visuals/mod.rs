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
//! - [`export`] — PNG heightmap, OBJ mesh, and JSON config export (native + WASM).

pub mod droplets;
pub mod export;
pub mod material;
pub mod scene;
pub mod splat_material;
pub mod terrain;
pub mod urban_gizmos;
