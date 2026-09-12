pub mod buildings;
pub mod catalog;
pub mod commands;
pub mod economy;
pub mod fixture;
pub mod ids;
pub mod map;
pub mod movement;

pub use buildings::{
    Building, BuildingIndex, ConstructionState, PlacementPlan, step_construction,
    validate_placement,
};
pub use catalog::{
    AGE_TWO_COST, AGE_TWO_GATHER_RATE, AGE_TWO_SECONDS, Age, BASE_GATHER_RATE, BuildingKind,
    BuildingSpec, CARRY_LIMIT, Cost, MAX_POPULATION, ResourceKind, UnitKind, UnitSpec,
    resource_amount,
};
pub use commands::{
    CommandResult, PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, UnitIndex,
    apply_player_command, spawn_unit,
};
pub use economy::{
    Carry, Dropoff, GatherProgress, ResourceStockpile, TeamEconomy, TeamState, WorkerTask,
};
pub use fixture::{MapFixture, ResourceSpawn, TeamStart, seed_skirmish};
pub use ids::{BuildingId, IdAllocator, ResourceId, TeamId, UnitId};
pub use map::{Footprint, GridMap, GridPos};
pub use movement::{MoveOrder, SIM_STEP_SECONDS, SimPosition, Unit, step_movement};
