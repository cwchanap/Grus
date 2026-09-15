use bevy::prelude::{Component, Resource};

#[derive(Clone, Copy, Component, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnitId(pub u32);

#[derive(Clone, Copy, Component, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuildingId(pub u32);

#[derive(Clone, Copy, Component, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceId(pub u32);

#[derive(Clone, Copy, Component, Debug, Eq, Hash, PartialEq)]
pub struct TeamId(pub u8);

/// Monotonic source of runtime unit/building/resource IDs. Authored fixture
/// entities use deterministic IDs; allocator counters start above authored
/// maxima. Zero is never handed out because the Godot bridge filters it.
#[derive(Debug, Resource)]
pub struct IdAllocator {
    pub next_unit: u32,
    pub next_building: u32,
    pub next_resource: u32,
}

impl IdAllocator {
    pub const fn new(next_unit: u32, next_building: u32, next_resource: u32) -> Self {
        Self {
            next_unit,
            next_building,
            next_resource,
        }
    }

    pub fn allocate_unit(&mut self) -> UnitId {
        UnitId(next_non_zero(&mut self.next_unit))
    }

    pub fn allocate_building(&mut self) -> BuildingId {
        BuildingId(next_non_zero(&mut self.next_building))
    }

    pub fn allocate_resource(&mut self) -> ResourceId {
        ResourceId(next_non_zero(&mut self.next_resource))
    }
}

/// Skips a zero counter, returns the current value, and advances past it.
fn next_non_zero(counter: &mut u32) -> u32 {
    let id = if *counter == 0 { 1 } else { *counter };
    *counter = id.checked_add(1).expect("ID space exhausted");
    id
}

#[cfg(test)]
mod tests;
