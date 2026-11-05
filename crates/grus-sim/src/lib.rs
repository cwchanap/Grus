pub mod buildings;
pub mod catalog;
pub mod combat;
pub mod commands;
pub mod economy;
pub mod fixture;
pub mod heat;
pub mod ids;
pub mod map;
pub mod movement;
pub mod production;
pub mod research;
pub mod session;
pub mod tick;
pub mod underground;
pub mod visibility;
pub mod waves;

pub use buildings::{
    Building, BuildingIndex, ConstructionState, PlacementPlan, spawn_completed_building,
    step_construction, validate_placement,
};
pub use catalog::{
    ANALYZE_COST, ANALYZE_RESEARCH_GAIN, ATTACK_MOVE_RADIUS, AlienSpec, BASE_GATHER_RATE,
    BuildingKind, BuildingSpec, CARRY_LIMIT, CombatSpec, Cost, EXCAVATION_BASE_SECONDS,
    EXCAVATION_COAL_DENOMINATOR, EXCAVATION_DEPTH_MULTIPLIER, EXCAVATION_INITIAL_SCRAP,
    EXCAVATION_SECONDS_PER_LEVEL, FURNACE_COAL_PER_SECOND, MAX_LIVE_ALIENS, MAX_POPULATION,
    MIN_SURFACE_EFFICIENCY, RESEARCH_BASE_PER_SECOND, RESEARCH_TARGET_POINTS,
    RESEARCH_WAVE_BOOST_DENOMINATOR, RESEARCH_WAVE_BOOST_NUMERATOR, RESEARCH_WAVE_BOOST_PERCENT,
    ROOM_SLOTS_PER_LEVEL, ResourceKind, RoomKind, RoomSpec, UnitKind, UnitSpec, WAVE_BUDGET_BASE,
    WAVE_BUDGET_PER_WAVE, WAVE_BUDGET_QUADRATIC_TENTHS, WAVE_INTERVAL_SECONDS,
    WAVE_PREPARATION_SECONDS, WAVE_THREE_ZONE_START, WAVE_TWO_ZONE_START, WAVE_WARNING_SECONDS,
    excavation_cost_and_seconds, resource_amount, room_spec,
};
pub use combat::{
    AttackCooldown, CombatEvent, CombatEventTotal, CombatEvents, CombatOrder, CombatTarget, Health,
    resolve_target, step_combat, target_eligible,
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
    ENTRY_ZONES, EntryZone, MapFixture, RUINS, ResourceSpawn, TeamStart, benchmark_building_count,
    clear_match_world, extend_with_benchmark_workload, seed_skirmish,
};
pub use heat::{HeatState, step_heat, surface_efficiency};
pub use ids::{BuildingId, IdAllocator, ResourceId, TeamId, UnitId};
pub use map::{Footprint, GridMap, GridPos};
pub use movement::{MoveOrder, SIM_STEP_SECONDS, SimPosition, Unit, step_movement};
pub use production::{
    ProductionJob, ProductionKind, ProductionQueue, RallyPoint, population_cap, population_used,
    produces, step_production,
};
pub use research::{
    ResearchState, add_research, building_unlocked, living_research_core, research_points,
    research_rate, step_research, unlock_points,
};
pub use session::{
    MatchPhase, MatchResult, MatchSession, active_phase, gameplay_active, resolve_result,
    set_paused, start_match,
};
pub use tick::step_tick;
pub use underground::{Excavation, Room, Underground, UndergroundLevel, step_underground};
pub use visibility::{
    CellVisibility, VisibilityMap, VisibilitySubject, explored_by, refresh_visibility, visible_to,
};
pub use waves::{
    TIMER_BOUNDARY_EPSILON_SECONDS, WaveRoster, WaveState, live_alien_ids, plan_roster, step_waves,
    wave_budget, wave_deadline, zones_for,
};
