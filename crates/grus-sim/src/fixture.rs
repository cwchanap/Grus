use bevy::math::Vec2;
use bevy::prelude::World;

use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{
    Age, BuildingKind, ResourceKind, UnitKind, building_spec, resource_amount, unit_spec,
};
use crate::combat::Health;
use crate::commands::spawn_unit;
use crate::economy::{
    Carry, Dropoff, GatherProgress, ResourceStockpile, TeamEconomy, WorkerTask,
    spawn_resource_source,
};
use crate::ids::{BuildingId, IdAllocator, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};

#[derive(Clone, Copy, Debug)]
pub struct UnitSpawn {
    pub id: UnitId,
    pub team: TeamId,
    pub position: Vec2,
}

/// Authored starting position of one team: Town Center anchor plus the four
/// villager spawn cells. All values are cells on the 128×96 battlefield.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamStart {
    pub team: TeamId,
    pub town_center_anchor: GridPos,
    pub villagers: [GridPos; 4],
}

/// One authored finite resource node with a deterministic `ResourceId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSpawn {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub cell: GridPos,
    pub amount: u32,
}

/// Coordinate-only authored AI base layout for one team: building slots and
/// the scout/attack route, mirrored between the two team starts. Deliberately
/// no live `UnitId`/`BuildingId`/`ResourceId` references — the AI policy
/// resolves everything through the indexes and real command validation at
/// apply time. Not exposed through any bridge snapshot; GDScript smoke build
/// constants stay independent human-UI test choreography.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiMapPlan {
    pub town_center_anchor: GridPos,
    pub house_slots: Vec<GridPos>,
    pub farm_slots: Vec<GridPos>,
    pub safe_storehouse_slots: Vec<GridPos>,
    pub expansion_storehouse_slots: Vec<GridPos>,
    pub barracks_anchor: GridPos,
    pub archery_range_anchor: GridPos,
    pub stable_anchor: GridPos,
    pub scout_route: Vec<GridPos>,
}

#[derive(Clone, Debug)]
pub struct MapFixture {
    pub map: GridMap,
    pub left_spawn: Vec2,
    pub right_spawn: Vec2,
    /// Authored finite resource nodes for the skirmish seed.
    pub resources: Vec<ResourceSpawn>,
}

impl MapFixture {
    /// The single authored team-start table. Anchors and villager cells come
    /// from the HPA-471 design; Town Centers are 4×4 footprints.
    pub fn team_starts() -> [TeamStart; 2] {
        [
            TeamStart {
                team: TeamId(1),
                town_center_anchor: GridPos::new(12, 46),
                villagers: [
                    GridPos::new(11, 45),
                    GridPos::new(11, 50),
                    GridPos::new(16, 45),
                    GridPos::new(16, 50),
                ],
            },
            TeamStart {
                team: TeamId(2),
                town_center_anchor: GridPos::new(112, 46),
                villagers: [
                    GridPos::new(116, 45),
                    GridPos::new(116, 50),
                    GridPos::new(111, 45),
                    GridPos::new(111, 50),
                ],
            },
        ]
    }

    /// Safe resources around each Town Center, mirrored between teams.
    /// The only resource-position table in the codebase; IDs are
    /// deterministic (team 1 safe, then team 2 safe).
    pub fn starting_resources() -> Vec<ResourceSpawn> {
        vec![
            spawn(ResourceId(1), ResourceKind::Food, 22, 42),
            spawn(ResourceId(2), ResourceKind::Food, 22, 54),
            spawn(ResourceId(3), ResourceKind::Wood, 20, 45),
            spawn(ResourceId(4), ResourceKind::Wood, 20, 48),
            spawn(ResourceId(5), ResourceKind::Wood, 20, 51),
            spawn(ResourceId(6), ResourceKind::Gold, 25, 48),
            spawn(ResourceId(7), ResourceKind::Food, 105, 42),
            spawn(ResourceId(8), ResourceKind::Food, 105, 54),
            spawn(ResourceId(9), ResourceKind::Wood, 107, 45),
            spawn(ResourceId(10), ResourceKind::Wood, 107, 48),
            spawn(ResourceId(11), ResourceKind::Wood, 107, 51),
            spawn(ResourceId(12), ResourceKind::Gold, 102, 48),
        ]
    }

    /// Southwest and northeast expansion resources; IDs continue after the
    /// safe sources.
    pub fn expansion_resources() -> Vec<ResourceSpawn> {
        vec![
            spawn(ResourceId(13), ResourceKind::Gold, 45, 16),
            spawn(ResourceId(14), ResourceKind::Wood, 43, 18),
            spawn(ResourceId(15), ResourceKind::Wood, 47, 18),
            spawn(ResourceId(16), ResourceKind::Gold, 82, 79),
            spawn(ResourceId(17), ResourceKind::Wood, 84, 77),
            spawn(ResourceId(18), ResourceKind::Wood, 80, 77),
        ]
    }

    pub fn battlefield() -> Self {
        let mut map = GridMap::new(128, 96);

        // Mirrored central blockers leave a northern and southern route.
        map.set_blocked_rect(GridPos::new(54, 20), GridPos::new(61, 75));
        map.set_blocked_rect(GridPos::new(66, 20), GridPos::new(73, 75));
        map.set_blocked_rect(GridPos::new(62, 38), GridPos::new(65, 57));

        Self {
            map,
            left_spawn: Vec2::new(14.5, 48.5),
            right_spawn: Vec2::new(113.5, 48.5),
            resources: Self::starting_resources()
                .into_iter()
                .chain(Self::expansion_resources())
                .collect(),
        }
    }

    /// The one authored **Rust** base-coordinate table: the team-1 layout
    /// below, mirrored for team 2. Base slots flip on the vertical center
    /// line (footprint-width aware) and the scout route plus expansion
    /// storehouse point-reflect through the map center, where the authored
    /// blockers and expansion resources are symmetric. Session/system-order
    /// tests anchor their choreography here so there is exactly one table.
    pub fn team_plan(team: TeamId) -> AiMapPlan {
        let plan = AiMapPlan {
            town_center_anchor: GridPos::new(12, 46),
            house_slots: vec![
                GridPos::new(8, 44),
                GridPos::new(8, 48),
                GridPos::new(8, 52),
            ],
            farm_slots: vec![
                GridPos::new(22, 44),
                GridPos::new(22, 50),
                GridPos::new(22, 56),
            ],
            safe_storehouse_slots: vec![GridPos::new(21, 46)],
            expansion_storehouse_slots: vec![GridPos::new(45, 20)],
            barracks_anchor: GridPos::new(17, 51),
            archery_range_anchor: GridPos::new(17, 38),
            stable_anchor: GridPos::new(17, 55),
            scout_route: vec![
                GridPos::new(30, 36),
                GridPos::new(45, 17),
                GridPos::new(64, 12),
                GridPos::new(85, 25),
                GridPos::new(106, 44),
            ],
        };
        if team == TeamId(1) {
            return plan;
        }
        AiMapPlan {
            town_center_anchor: mirror_anchor(
                plan.town_center_anchor,
                building_spec(BuildingKind::TownCenter).width,
            ),
            house_slots: mirrored(&plan.house_slots, building_spec(BuildingKind::House).width),
            farm_slots: mirrored(&plan.farm_slots, building_spec(BuildingKind::Farm).width),
            safe_storehouse_slots: mirrored(
                &plan.safe_storehouse_slots,
                building_spec(BuildingKind::Storehouse).width,
            ),
            expansion_storehouse_slots: plan
                .expansion_storehouse_slots
                .iter()
                .map(|slot| point_reflect_anchor(*slot, BuildingKind::Storehouse))
                .collect(),
            barracks_anchor: mirror_anchor(
                plan.barracks_anchor,
                building_spec(BuildingKind::Barracks).width,
            ),
            archery_range_anchor: mirror_anchor(
                plan.archery_range_anchor,
                building_spec(BuildingKind::ArcheryRange).width,
            ),
            stable_anchor: mirror_anchor(
                plan.stable_anchor,
                building_spec(BuildingKind::Stable).width,
            ),
            scout_route: plan
                .scout_route
                .iter()
                .map(|cell| point_reflect(*cell))
                .collect(),
        }
    }

    pub fn units_200(&self) -> Vec<UnitSpawn> {
        let mut units = Vec::with_capacity(200);

        for row in 0..10 {
            for column in 0..10 {
                let offset = (row * 10 + column) as u32;
                units.push(UnitSpawn {
                    id: UnitId(offset + 1),
                    team: TeamId(1),
                    position: Vec2::new(
                        self.left_spawn.x - 4.0 + column as f32,
                        self.left_spawn.y - 4.0 + row as f32,
                    ),
                });
                units.push(UnitSpawn {
                    id: UnitId(offset + 101),
                    team: TeamId(2),
                    position: Vec2::new(
                        self.right_spawn.x + 4.0 - column as f32,
                        self.right_spawn.y - 4.0 + row as f32,
                    ),
                });
            }
        }

        units
    }
}

/// The pure simulation skirmish seed for one normal match: real completed
/// Town Center Buildings (4×4 Footprints + `Dropoff` from birth), four
/// Villagers per team with fresh worker components, both teams' economies
/// (200 Food / 300 Wood / 100 Gold, Age 1), Town Center map occupancy, the
/// authored finite resource sources, and `UnitIndex`/`BuildingIndex`
/// `IdAllocator` counters above the authored maxima. Every starting entity
/// flows through the production constructors — there is no temporary
/// Dropoff-only Town Center.
pub fn seed_skirmish(world: &mut World, map: &mut GridMap, fixture: &MapFixture) {
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile {
            food: 200,
            wood: 300,
            gold: 100,
        },
        Age::Age1,
    );
    economy.insert_team(
        TeamId(2),
        ResourceStockpile {
            food: 200,
            wood: 300,
            gold: 100,
        },
        Age::Age1,
    );
    world.insert_resource(economy);

    let mut unit_counter: u32 = 0;
    let town_center_health = building_spec(BuildingKind::TownCenter).max_health;
    for (index, start) in MapFixture::team_starts().into_iter().enumerate() {
        let building_id = BuildingId(index as u32 + 1);
        let footprint = Footprint::new(start.town_center_anchor, 4, 4);
        for cell in footprint.cells() {
            map.set_blocked(cell, true);
        }
        let entity = world
            .spawn((
                Building {
                    id: building_id,
                    team: start.team,
                    kind: BuildingKind::TownCenter,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                footprint,
                Dropoff { team: start.team },
                Health {
                    current: town_center_health,
                    max: town_center_health,
                },
            ))
            .id();
        let mut buildings = world.get_resource_or_insert_with(BuildingIndex::default);
        buildings.insert(building_id, entity);

        for cell in start.villagers {
            unit_counter += 1;
            let villager = spawn_unit(
                world,
                UnitId(unit_counter),
                start.team,
                map.cell_center(cell),
                UnitKind::Villager,
                unit_spec(UnitKind::Villager).speed,
            );
            world.entity_mut(villager).insert((
                Carry::Empty,
                GatherProgress::default(),
                WorkerTask::Idle,
            ));
        }
    }

    // Authored finite sources (deterministic IDs 1..=18) block their cells
    // and register in the resource index.
    for resource in &fixture.resources {
        spawn_resource_source(
            world,
            map,
            resource.id,
            resource.kind,
            resource.cell,
            resource.amount,
        );
    }

    world.insert_resource(IdAllocator::new(unit_counter + 1, 3, 19));
}

/// Horizontal mirror of a `width`-wide footprint anchor on the 128-wide
/// battlefield: cells `x..x+w` map to `127-(x+w-1)..127-x`.
fn mirror_anchor(anchor: GridPos, width: u8) -> GridPos {
    GridPos::new(127 - anchor.x - i32::from(width) + 1, anchor.y)
}

/// Point reflection through the map center — the axis pair the battlefield
/// blockers and the expansion resources are symmetric under.
fn point_reflect(cell: GridPos) -> GridPos {
    GridPos::new(127 - cell.x, 95 - cell.y)
}

/// Point reflection of a footprint *anchor*, footprint-size aware on both
/// axes: reflecting only the anchor point would shift a w×h footprint by
/// (w-1, h-1) cells and break the authored symmetry (the team-2 expansion
/// Storehouse landed on a tree's gathering cells before this).
fn point_reflect_anchor(anchor: GridPos, kind: BuildingKind) -> GridPos {
    let spec = building_spec(kind);
    GridPos::new(
        127 - anchor.x - i32::from(spec.width) + 1,
        95 - anchor.y - i32::from(spec.height) + 1,
    )
}

fn mirrored(anchors: &[GridPos], width: u8) -> Vec<GridPos> {
    anchors
        .iter()
        .map(|anchor| mirror_anchor(*anchor, width))
        .collect()
}

fn spawn(id: ResourceId, kind: ResourceKind, x: i32, y: i32) -> ResourceSpawn {
    ResourceSpawn {
        id,
        kind,
        cell: GridPos::new(x, y),
        amount: resource_amount(kind),
    }
}

#[cfg(test)]
mod tests;
