pub mod buildings;
pub mod catalog;
pub mod combat;
pub mod commands;
pub mod economy;
pub mod fixture;
pub mod ids;
pub mod map;
pub mod movement;
pub mod production;
pub mod session;

pub use buildings::{
    Building, BuildingIndex, ConstructionState, PlacementPlan, step_construction,
    validate_placement,
};
pub use catalog::{
    AGE_TWO_COST, AGE_TWO_GATHER_RATE, AGE_TWO_SECONDS, ATTACK_MOVE_RADIUS, Age, BASE_GATHER_RATE,
    BuildingKind, BuildingSpec, CARRY_LIMIT, CombatSpec, Cost, MAX_POPULATION, ResourceKind,
    UnitKind, UnitSpec, resource_amount,
};
pub use combat::{
    AttackCooldown, CombatEvent, CombatEvents, CombatOrder, CombatTarget, Health, step_combat,
    target_eligible,
};
pub use commands::{
    CommandResult, PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, UnitIndex,
    apply_player_command, spawn_unit,
};
pub use economy::{
    Carry, Dropoff, GatherProgress, LastRouteReject, ResourceIndex, ResourceSource,
    ResourceStockpile, TeamEconomy, TeamState, WorkerTask, gather_rate_for_age, step_economy,
};
pub use fixture::{MapFixture, ResourceSpawn, TeamStart, seed_skirmish};
pub use ids::{BuildingId, IdAllocator, ResourceId, TeamId, UnitId};
pub use map::{Footprint, GridMap, GridPos};
pub use movement::{MoveOrder, SIM_STEP_SECONDS, SimPosition, Unit, step_movement};
pub use production::{
    ProductionJob, ProductionKind, ProductionQueue, RallyPoint, population_cap, population_used,
    produces, step_production,
};
pub use session::{
    MatchPhase, MatchResult, MatchSession, active_phase, gameplay_active, resolve_result,
    set_paused, start_match,
};
