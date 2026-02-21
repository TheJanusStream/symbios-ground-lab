# Symbios Ground Lab

An interactive 3D terrain generator and editor built with [Bevy](https://bevyengine.org/) 0.18. Generate procedural landscapes, apply physically-motivated erosion, preview them with GPU-accelerated splat materials, and export the results for use in other tools.

A live WASM build is available at the project's GitHub Pages URL (see the deploy workflow).

---

## Features

### Terrain Generation
Three procedural algorithms are available, each with live-tweakable parameters:

| Algorithm | Description |
|-----------|-------------|
| **FBM Noise** | Fractional Brownian Motion layered over multiple octaves. Controls: octave count, persistence, lacunarity, base frequency. |
| **Diamond Square** | Classic mid-point displacement; produces realistic ridge-and-valley terrain. Control: roughness (0 = smooth, 1 = jagged). |
| **Voronoi Terracing** | Voronoi-cell-based terracing for stylised stepped landscapes. Controls: seed-point count, terrace count. |

All generators share a **grid size** (64–2048), **cell scale** (world units per grid cell), **height scale**, and a **seed** for reproducibility.

### Erosion
- **Hydraulic erosion** — particle-based droplet simulation. Parameters: drop count, inertia, erosion rate, deposition rate, evaporation rate, capacity factor.
- **Thermal erosion** — talus-angle slope relaxation. Parameters: iteration count, talus angle.
- **Erosion visualisation** — watch individual water droplets carve the terrain in real time; each droplet renders as a cyan sphere with a fading trail.

### Splat-Based Materials
Four tiling terrain layers blended in the fragment shader:
- **R — Grass** (low altitude, gentle slope)
- **G — Dirt** (mid altitude)
- **B — Rock** (steep slopes)
- **A — Snow** (high altitude, gentle slope)

Each layer uses a procedurally generated albedo + normal map (configurable resolution 256–4096 px). Blending weights are driven by a per-pixel height/slope weight map computed from the heightmap. All texture and weight-map generation runs asynchronously so the UI stays responsive.

### Export
Files are written to an `exports/` folder on native builds; on WASM a browser download is triggered.

| Format | Description |
|--------|-------------|
| **PNG (16-bit)** | Greyscale heightmap, full dynamic range normalised to `[0, 65535]`. |
| **OBJ** | Full mesh with vertex positions, UVs `[0, 1]`, and per-vertex normals. |
| **JSON** | Current `TerrainConfig` plus metadata (grid size, world extents, height range). |

---

## Building and Running

### Native (desktop)

```bash
cargo run
```

### WebAssembly

The project ships a GitHub Actions workflow (`.github/workflows/deploy.yml`) that builds for `wasm32-unknown-unknown` with `wasm-bindgen` and deploys to GitHub Pages. To build locally:

```bash
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --out-dir out --target web target/wasm32-unknown-unknown/release/symbios-ground-lab.wasm
```

Then serve `index.html` and the generated `out/` directory from a local HTTP server (e.g. `python3 -m http.server`).

---

## Controls

| Input | Action |
|-------|--------|
| **Right-drag** | Orbit camera |
| **Middle-drag** | Pan camera |
| **Scroll wheel** | Zoom |
| **Terraformer panel** | Tweak all generation parameters |
| **Materials panel** | Configure splat layers and texture settings |
| **Regenerate button** | Re-run terrain generation with current parameters |
| **Visualise Erosion button** | Start the real-time droplet erosion visualisation |
| **Stop Viz button** | Abort the erosion visualisation early |

---

## Architecture

```
src/
  main.rs              — App setup: plugins, resources, system schedule
  lib.rs               — Re-exports the four top-level modules
  core/
    config.rs          — All shared resources: TerrainConfig, DirtyFlags,
                         ErosionVizState, CurrentHeightMap, …
    material_config.rs — MaterialConfig (splat rules + texture params) and
                         MaterialState (async pipeline progress)
  logic/
    generation.rs      — Async terrain generation task (FBM / DS / Voronoi +
                         hydraulic + thermal erosion)
    erosion_viz.rs     — Frame-by-frame droplet stepping for visualisation
  ui/
    panel.rs           — Main "Terraformer" egui window
    material_panel.rs  — "Materials" egui window
  visuals/
    scene.rs           — Camera, lighting setup
    terrain.rs         — TerrainMesh spawn and rebuild from CurrentHeightMap
    droplets.rs        — Gizmo rendering for erosion visualisation droplets
    material.rs        — Splat material pipeline (texture tasks, weight map)
    splat_material.rs  — SplatExtension MaterialExtension type and bindings
    export.rs          — PNG / OBJ / JSON export (native + WASM)
assets/
  shaders/
    splat.wgsl         — Fragment shader: splat weight sampling + layer blending
```

### System execution order (Update schedule, chained)

```
start_generation → poll_generation → step_erosion_viz
  → rebuild_terrain → draw_droplet_gizmos → poll_export_task
  → detect_material_dirty → start_texture_tasks
  → collect_texture_results → apply_splat_material
```

### Performance notes

- Terrain generation and OBJ export run on Bevy's `AsyncComputeTaskPool` so they never block the render thread.
- Procedural texture generation is handled by `bevy_symbios_texture` and also runs asynchronously.
- During the erosion visualisation, splat weight-map regeneration and tangent computation are suppressed on intermediate frames and deferred to the final rebuild when the visualisation completes.

---

## Dependencies

| Crate | Role |
|-------|------|
| `bevy` 0.18 | Engine, ECS, rendering |
| `bevy_egui` 0.39 | Immediate-mode UI |
| `bevy_panorbit_camera` 0.34 | Orbit camera |
| `symbios-ground` 0.1 | HeightMap, terrain generators, erosion algorithms |
| `bevy_symbios_ground` 0.1 | HeightMapMeshBuilder, normal methods |
| `bevy_symbios_texture` 0.1 | Async procedural texture generation |
| `rand` / `rand_pcg` 0.9 | Deterministic RNG |
| `serde` / `serde_json` 1.0 | JSON serialisation |
| `image` 0.25 | PNG encoding |
