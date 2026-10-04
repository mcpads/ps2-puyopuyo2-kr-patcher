use super::validation::validate_texture_target_partitions;

#[test]
fn target_partition_accepts_explicit_consumer_exclusions() {
    validate_texture_target_partitions(145, 139, 3, 3, 3, 0).unwrap();
}

#[test]
fn target_partition_rejects_unclassified_linked_candidate() {
    assert!(validate_texture_target_partitions(145, 139, 2, 3, 3, 0).is_err());
}

#[test]
fn target_partition_rejects_unlinked_admission() {
    assert!(validate_texture_target_partitions(145, 139, 3, 3, 3, 1).is_err());
}
