# Symbios Ground Lab

An interactive 3D terrain generator and editor built with [Bevy](https://bevyengine.org/) 0.18. Generate procedural landscapes, apply physically-motivated erosion, lay out tensor-field urban road networks with procedural buildings, preview everything with GPU-accelerated splat materials, and export the results for use in other tools.

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

All generators share a **grid size** (64–2048), **cell scale** (world units per grid cell), **height scale**, **water level**, and a **seed** for reproducibility.

### Water
A translucent water volume is rendered at the configurable water level. The surface uses a custom WGSL fragment shader (`water.wgsl`) for animated wave effects. Erosion respects the water level — hydraulic droplets skip submerged cells, and thermal erosion uses a gentler underwater talus angle.

### Erosion
- **Hydraulic erosion** — particle-based droplet simulation. Parameters: drop count, inertia, erosion rate, deposition rate, evaporation rate, capacity factor.
- **Thermal erosion** — talus-angle slope relaxation. Parameters: iteration count, talus angle.
- **Erosion visualisation** — watch individual water droplets carve the terrain in real time; each droplet renders as a cyan sphere with a fading trail.

### Splat-Based Materials
Four tiling terrain layers blended in the fragment shader:
- **R — Grass** (low altitude, gentle slope)
- **G — Dirt** (mid altitude)
- **B — Rock** (steep slopes, triplanar projected)
- **A — Snow** (high altitude, gentle slope)

Each layer uses a procedurally generated albedo + normal map (configurable resolution 256–4096 px). Blending weights are driven by a per-pixel height/slope weight map computed from the heightmap. All texture and weight-map generation runs asynchronously so the UI stays responsive.

### Urban Generation
Tensor-field-based road network generation with water-aware tracing:
- **Major and minor roads** traced from a configurable tensor field (step size, road distances, snap radius).
- **City blocks** extracted from the road graph with perimeter detection.
- **Building lots** subdivided from blocks with configurable area limits, setbacks, and minimum dimensions.
- **Synaptic pruning** removes roads that serve no lots, keeping the network efficient.
- **Terrain carving** flattens the heightmap under roads and lots with smooth blend radii before erosion runs.
- **Graph rationalisation** straightens edges via Ramer–Douglas–Peucker simplification and smooths intersections with configurable fillet arcs.
- **3D road meshes** rendered as intersection hub polygons, spline-sampled ribbons with procedural asphalt textures, and embankment skirts with a dirt material.
- **Debug gizmos** for road edges (yellow=major, cyan=minor), block outlines (green), block centroids (magenta), and lot footprints (orange).

### Procedural Architecture
CGA (Computer Generated Architecture) grammar-driven building generation:
- A **live grammar editor** in the Architect panel lets you modify shape derivation rules in real time.
- Buildings are derived from building lots using the `Lot` axiom and placed at terrain height.
- **Seven procedural material types**: Brick, Stucco, Concrete, Shingle, Wood, Glass, and Metal — each with configurable texture parameters.
- Texture generation runs asynchronously with debounced regeneration on parameter changes.
- A **max-buildings** slider caps the number of buildings for performance control.

### Export
Files are written to an `exports/` folder on native builds; on WASM a browser download is triggered.

| Format | Description |
|--------|-------------|
| **PNG (16-bit)** | Greyscale heightmap, full dynamic range normalised to `[0, 65535]`. |
| **OBJ** | Full mesh with vertex positions, UVs `[0, 1]`, and per-vertex normals. |
| **JSON** | Current `TerrainConfig` and `MaterialConfig` plus metadata (grid size, world extents, height range). |

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
| **Terraformer panel** | Tweak terrain generation, erosion, and export parameters |
| **Materials panel** | Configure splat layers and texture settings |
| **Urban Planner panel** | Configure road generation, block/lot subdivision, and road materials |
| **Architect panel** | Edit CGA grammar, cap building count, and tune building material textures |
| **Regenerate button** | Re-run terrain generation with current parameters |
| **Visualise Erosion button** | Start the real-time droplet erosion visualisation |
| **Stop Viz button** | Abort the erosion visualisation early |

---

## Architecture

```
src/
  main.rs                — App setup: plugins (incl. BevySymbiosShapePlugin,
                           SymbiosTexturePlugin), resources, system schedule
  lib.rs                 — Declares the four top-level modules
  core/
    config.rs            — TerrainConfig, DirtyFlags, ErosionVizState,
                           CurrentHeightMap, GenerationTask, ExportStatus, …
    material_config.rs   — MaterialConfig (splat rules + texture params) and
                           MaterialState (async pipeline progress)
    urban_config.rs      — UrbanConfig (tensor-field roads, lot subdivision),
                           CurrentRoadGraph, CurrentBuildingLots, RoadMaterialState
    architecture_config.rs — ArchitectureConfig (CGA grammar, building material
                           configs) and ArchitectureMaterialState
  logic/
    generation.rs        — Async terrain generation task (FBM / DS / Voronoi +
                           urban road carving + hydraulic + thermal erosion)
    erosion_viz.rs       — Frame-by-frame droplet stepping for visualisation
  ui/
    panel.rs             — Main "Terraformer" egui window
    material_panel.rs    — "Materials" egui window
    urban_panel.rs       — "Urban Planner" egui window
    architecture_panel.rs — "Architect" egui window
  visuals/
    scene.rs             — Camera, lighting setup
    terrain.rs           — TerrainMesh + WaterVolume spawn and rebuild
    droplets.rs          — Gizmo rendering for erosion visualisation droplets
    material.rs          — Splat material pipeline (texture tasks, weight map)
    splat_material.rs    — SplatExtension MaterialExtension type and bindings
    water_material.rs    — WaterExtension MaterialExtension for animated water
    buildings.rs         — CGA grammar-driven procedural building generation
    building_materials.rs — Async texture pipeline for building facade materials
    roads.rs             — 3D road mesh generation (hubs + spline ribbons)
    road_materials.rs    — Async asphalt texture pipeline for road surfaces
    urban_gizmos.rs      — Debug gizmos for roads, blocks, and lots
    export.rs            — PNG / OBJ / JSON export (native + WASM)
assets/
  shaders/
    splat.wgsl           — Fragment shader: splat weight sampling + layer blending
    water.wgsl           — Fragment shader: animated water surface
```

### System execution order (Update schedule, chained)

```
start_generation → poll_generation → poll_viz_init → step_erosion_viz
  → rebuild_terrain → draw_droplet_gizmos
  → draw_road_gizmos → draw_block_gizmos → draw_lot_gizmos
  → poll_export_task
  → detect_material_dirty → start_texture_tasks
  → collect_texture_results → apply_splat_material
  → regenerate_building_textures → rebuild_buildings → apply_building_textures
  → regenerate_road_textures → rebuild_roads → apply_road_textures
```

### Performance notes

- On **native** builds, terrain generation and PNG/OBJ export run on Bevy's `AsyncComputeTaskPool` across background threads so they never block the render thread.
- On **WASM**, Bevy's executor multiplexes async tasks onto the single main thread. Because the CPU-bound generation and export functions contain no `.await` yield points, they run to completion the moment the task is polled and will block the browser tab during that window. The UI and camera remain frozen for the duration; this is a known limitation of single-threaded WASM.
- Procedural texture generation (terrain splat, building materials, road asphalt) is handled by `bevy_symbios_texture` and also runs asynchronously (same caveat applies on WASM).
- During the erosion visualisation, splat weight-map regeneration and tangent computation are suppressed on intermediate frames and deferred to the final rebuild when the visualisation completes.
- Texture regeneration from UI slider drags is debounced (400 ms for terrain, 300 ms for buildings and roads) to prevent saturating the thread pool with abandoned tasks.
