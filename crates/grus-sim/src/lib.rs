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
pub mod visibility;
pub mod waves;

pub use buildings::{
    Building, BuildingIndex, ConstructionState, PlacementPlan, step_construction,
    validate_placement,
};
pub use catalog::{
    ATTACK_MOVE_RADIUS, AlienSpec, BASE_GATHER_RATE, BuildingKind, BuildingSpec, CARRY_LIMIT,
    CombatSpec, Cost, MAX_LIVE_ALIENS, MAX_POPULATION, ResourceKind, UnitKind, UnitSpec,
    WAVE_BUDGET_BASE, WAVE_BUDGET_PER_WAVE, WAVE_BUDGET_QUADRATIC_TENTHS, WAVE_INTERVAL_SECONDS,
    WAVE_PREPARATION_SECONDS, WAVE_THREE_ZONE_START, WAVE_TWO_ZONE_START, WAVE_WARNING_SECONDS,
    resource_amount,
};
pub use combat::{
    AttackCooldown, CombatEvent, CombatEventTotal, CombatEvents, CombatOrder, CombatTarget, Health,
    step_combat, target_eligible,
};
pub use commands::{
    CommandResult, PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, UnitIndex,
    apply_player_command, sorted_unit_ids, spawn_unit,
};
pub use economy::{
    Carry, Dropoff, GatherProgress, LastRouteReject, ResourceIndex, ResourceSource,
    ResourceStockpile, TeamEconomy, TeamState, WorkerTask, idle_worker_ids, is_idle_worker,
    step_economy,
};
pub use fixture::{
    ENTRY_ZONES, EntryZone, MapFixture, RUINS, ResourceSpawn, TeamStart,
    extend_with_benchmark_workload, seed_skirmish,
};
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
pub use visibility::{
    CellVisibility, VisibilityMap, VisibilitySubject, explored_by, refresh_visibility, visible_to,
};
pub use waves::{
    WaveRoster, WaveState, live_alien_ids, plan_roster, step_waves, wave_budget, zones_for,
};
