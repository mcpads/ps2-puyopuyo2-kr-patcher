use super::*;

#[test]
fn residual_descriptors_preserve_the_exact_parallel_file_list_pairs() {
    let pairs = RESIDUAL_PAIR_DESCRIPTORS
        .iter()
        .map(|descriptor| {
            (
                descriptor.file_id,
                descriptor.image_file_record,
                descriptor.palette_file_record,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pairs,
        [
            (0, 2, 61),
            (0, 23, 83),
            (0, 24, 84),
            (0, 33, 93),
            (0, 42, 102),
            (1, 92, 215),
            (1, 99, 222),
            (1, 106, 229),
            (2, 71, 401),
        ]
    );
}

#[test]
fn absent_palette_load_flags_render_both_alpha_interpretations() {
    assert_eq!(
        alpha_variants_for_load_flags(&BTreeSet::new()),
        [false, true]
    );
}

#[test]
fn observed_palette_load_flags_preserve_only_observed_alpha_modes() {
    assert_eq!(
        alpha_variants_for_load_flags(&BTreeSet::from([0, 1, 2, 3])),
        [false, true]
    );
    assert_eq!(
        alpha_variants_for_load_flags(&BTreeSet::from([2, 3])),
        [true]
    );
}
