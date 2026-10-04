use super::*;

#[test]
fn palette_slot_requires_exact_zero_padding() {
    let mut slot = vec![0_u8; CPX_BLOCK_ALIGNMENT];
    slot[PALETTE_BYTES - 1] = 0x80;
    assert_eq!(split_palette_slot(&slot).unwrap().len(), PALETTE_BYTES);

    slot[PALETTE_BYTES] = 1;
    assert!(split_palette_slot(&slot).is_err());
}

#[test]
fn linked_dimensions_take_precedence_over_width_inference() {
    let known_dimensions = BTreeMap::from([(393_216, BTreeSet::from([(768, 512), (1_024, 384)]))]);
    let known_widths = BTreeSet::from([512, 640, 768, 1_024]);
    let (source, dimensions) =
        dimension_candidates(393_216, &known_dimensions, &known_widths).unwrap();

    assert!(matches!(source, DimensionCandidateSource::LinkedPixelCount));
    assert_eq!(dimensions, known_dimensions[&393_216]);
}

#[test]
fn unseen_pixel_count_uses_only_valid_linked_widths() {
    let known_widths = BTreeSet::from([512, 640, 768, 1_024]);
    let (source, dimensions) =
        dimension_candidates(307_200, &BTreeMap::new(), &known_widths).unwrap();

    assert!(matches!(
        source,
        DimensionCandidateSource::LinkedWidthInference
    ));
    assert_eq!(
        dimensions,
        BTreeSet::from([(512, 600), (640, 480), (768, 400), (1_024, 300)])
    );
}
