use bevy::prelude::Component;

#[derive(Clone, Copy, Component, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnitId(pub u32);

#[derive(Clone, Copy, Component, Debug, Eq, Hash, PartialEq)]
pub struct TeamId(pub u8);
