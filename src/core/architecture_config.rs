// src/core/architecture_config.rs

use bevy::prelude::*;
use bevy::time::Timer;
use serde::{Deserialize, Serialize};
use bevy_symbios_texture::{
    brick::BrickConfig, stucco::StuccoConfig, concrete::ConcreteConfig,
    shingle::ShingleConfig, plank::PlankConfig, window::WindowConfig,
    metal::MetalConfig,
};

/// Runtime state for debounced building-material texture regeneration.
///
/// Mirrors the debounce pattern used by [`MaterialState`] for the terrain splat
/// pipeline: the architecture UI sets `texture_debounce_pending` when a texture
/// parameter is committed, and the timer gates the actual `PendingTexture`
/// spawn to avoid flooding the thread pool during continuous slider drags.
#[derive(Resource)]
pub struct ArchitectureMaterialState {
    /// Set by the UI when a texture parameter changes; cleared once textures
    /// are re-spawned.
    pub textures_dirty: bool,
    /// True while the debounce timer is counting down.
    pub texture_debounce_pending: bool,
    /// Fires once after the debounce delay, triggering `textures_dirty`.
    pub texture_debounce_timer: Timer,
}

impl Default for ArchitectureMaterialState {
    fn default() -> Self {
        Self {
            textures_dirty: false,
            texture_debounce_pending: false,
            texture_debounce_timer: Timer::from_seconds(0.3, TimerMode::Once),
        }
    }
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct ArchitectureConfig {
    pub enabled: bool,
    pub grammar_source: String,
    
    // Texture Configs
    pub brick: BrickConfig,
    pub stucco: StuccoConfig,
    pub concrete: ConcreteConfig,
    pub shingle: ShingleConfig,
    pub wood: PlankConfig,
    pub glass: WindowConfig,
    pub metal: MetalConfig,
}

impl Default for ArchitectureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            // ELASTIC VILLA GRAMMAR
            // Uses floating splits (~) for main masses to fit any lot size.
            grammar_source: r#"
// Massing - Elastic ratio ~7:3 (approx 14:6)
Lot --> Split(X) { ~7: HouseMass | ~3: GarageMass }
HouseMass --> Split(Z) { 3: DeckArea | ~1: MainHouse }
GarageMass --> Split(Z) { 4: Driveway | ~1: GarageStruct }

DeckArea --> Extrude(0.3) Mat("Wood") I("Deck")
Driveway --> Extrude(0.1) Mat("Pavers") I("Drive")

MainHouse --> Extrude(9.5) Split(Y) { 3.5: GroundFloor | 0.3: BeltCourse | 3.2: UpperFloor | 0.3: RoofFascia | 2.2: MainRoof }
GarageStruct --> Extrude(4.0) Split(Y) { 3.5: GarageBody | 0.5: GarageRoof }

MainRoof --> Roof(Gable, 30) { Slope: ShingleSlope | GableEnd: GableWall }
ShingleSlope --> Mat("Shingle") I("RoofTile")
GableWall --> Mat("Stucco") I("Wall")

GarageRoof --> Comp(Faces) { Top: FlatRoof | Side: GarageFascia }
GarageFascia --> Extrude(0.1) Mat("Metal") I("Fascia")
FlatRoof --> Mat("Metal") I("GarageRoofTile")

BeltCourse --> Comp(Faces) { Side: BeltFace }
BeltFace --> Extrude(0.25) Mat("Concrete") I("Trim")
RoofFascia --> Comp(Faces) { Side: FasciaFace }
FasciaFace --> Extrude(0.05) Mat("Metal") I("Fascia")

GroundFloor --> Comp(Faces) { Front: FrontEntryFacade | Back: SideFacade | Left: SideFacade | Right: SideFacade }
// Elastic Facade: scales elements to fit width
FrontEntryFacade --> Split(X) { ~1.5: BrickWall | ~2.5: EntryDoor | ~1.0: BrickWall | ~4.0: PictureWindow | ~1: BrickWall }
SideFacade --> Repeat(X, 4.0) { SideBay }
SideBay --> Split(X) { ~1: BrickWall | 2.0: StandardWindowBrick | ~1: BrickWall }

UpperFloor --> Comp(Faces) { Side: UpperFacade }
UpperFacade --> Repeat(X, 3.5) { UpperBay }
UpperBay --> Split(X) { ~1: StuccoWall | 1.5: StandardWindowStucco | ~1: StuccoWall }

GarageBody --> Comp(Faces) { Front: GarageFront | Back: BrickWall | Left: BrickWall | Right: BrickWall }
GarageFront --> Split(X) { ~1: BrickWall | ~5.0: GarageDoor | ~1: BrickWall }

StandardWindowBrick --> Split(Y) { 0.9: BrickWall | 1.6: WinAssembly | ~1: BrickWall }
StandardWindowStucco --> Split(Y) { 0.9: StuccoWall | 1.6: WinAssembly | ~1: StuccoWall }
PictureWindow --> Split(Y) { 0.8: BrickWall | 2.2: WinAssembly | ~1: BrickWall }

WinAssembly --> Split(X) { 0.15: ConcreteFrame | ~1: WinCenter | 0.15: ConcreteFrame }
WinCenter --> Split(Y) { 0.15: ConcreteFrame | ~1: GlassPane | 0.15: ConcreteFrame }
ConcreteFrame --> Extrude(0.25) Mat("Concrete") I("Frame")
GlassPane --> Extrude(0.05) Mat("Glass") I("Pane")

EntryDoor --> Split(Y) { 2.4: DoorAssembly | ~1: BrickWall }
DoorAssembly --> Split(X) { 0.15: ConcreteFrame | ~1: DoorPanel | 0.15: ConcreteFrame }
DoorPanel --> Split(Y) { ~1: WoodPanel | 0.15: ConcreteFrame }
WoodPanel --> Extrude(0.1) Mat("Wood") I("Door")

GarageDoor --> Split(Y) { 2.5: GaragePanel | ~1: BrickWall }
GaragePanel --> Extrude(0.1) Mat("Metal") I("GDoor")

BrickWall --> Extrude(0.2) Mat("Brick") I("Wall")
StuccoWall --> Extrude(0.2) Mat("Stucco") I("Wall")
            "#.trim().to_string(),
            
            brick: BrickConfig { aspect_ratio: 3.0, color_brick: [0.45, 0.22, 0.15], scale: 8.0, ..default() },
            stucco: StuccoConfig { roughness: 0.35, color_base: [0.87, 0.83, 0.77], ..default() },
            concrete: ConcreteConfig { formwork_lines: 3.0, formwork_depth: 0.1, ..default() },
            shingle: ShingleConfig { color_tile: [0.2, 0.2, 0.25], scale: 8.0, ..default() },
            wood: PlankConfig { color_wood_light: [0.4, 0.24, 0.14], color_wood_dark: [0.22, 0.12, 0.06], ..default() },
            glass: WindowConfig { panes_x: 1, panes_y: 1, glass_opacity: 0.3, ..default() },
            metal: MetalConfig { color_metal: [0.18, 0.18, 0.2], ..default() },
        }
    }
}