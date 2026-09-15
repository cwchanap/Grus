use super::*;

#[test]
fn allocator_skips_zero_and_increments_per_kind() {
    let mut allocator = IdAllocator::new(0, 3, 1);

    assert_eq!(allocator.allocate_unit(), UnitId(1));
    assert_eq!(allocator.allocate_unit(), UnitId(2));
    assert_eq!(allocator.next_unit, 3);
    assert_eq!(allocator.allocate_building(), BuildingId(3));
    assert_eq!(allocator.next_building, 4);
    assert_eq!(allocator.allocate_resource(), ResourceId(1));
    assert_eq!(allocator.next_resource, 2);
}

#[test]
fn allocator_hands_out_last_id_before_exhaustion() {
    let mut allocator = IdAllocator::new(u32::MAX - 1, u32::MAX - 1, u32::MAX - 1);

    assert_eq!(allocator.allocate_unit(), UnitId(u32::MAX - 1));
    assert_eq!(allocator.next_unit, u32::MAX);
}

#[test]
#[should_panic(expected = "ID space exhausted")]
fn allocator_fails_at_u32_max_instead_of_wrapping() {
    let mut allocator = IdAllocator::new(u32::MAX, u32::MAX, u32::MAX);

    allocator.allocate_unit();
}
