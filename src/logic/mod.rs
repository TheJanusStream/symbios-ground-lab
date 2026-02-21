//! Terrain generation and erosion simulation.
//!
//! - [`generation`] — spawns an async task to generate a [`HeightMap`](symbios_ground::HeightMap)
//!   using the configured algorithm (FBM Noise, Diamond Square, or Voronoi Terracing)
//!   followed by optional hydraulic and thermal erosion passes.
//! - [`erosion_viz`] — frame-by-frame droplet stepping for the real-time erosion
//!   visualisation; publishes periodic heightmap snapshots so the terrain mesh
//!   updates while the simulation runs.

pub mod erosion_viz;
pub mod generation;
