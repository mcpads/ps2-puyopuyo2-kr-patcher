use super::*;

fn pairs(slot_one_image: usize, slot_one_palette: usize) -> Vec<NazoTextureRegistrationPair> {
    vec![
        NazoTextureRegistrationPair {
            image_load_index: 0,
            palette_slot: 0,
        },
        NazoTextureRegistrationPair {
            image_load_index: slot_one_image,
            palette_slot: slot_one_palette,
        },
    ]
}

#[test]
fn every_nazo_registration_set_must_supply_the_main_font_clone_in_slot_one() {
    let sets = EXPECTED_REGISTRATION_SETS.map(|name| (name, pairs(IMAGE_LOAD_INDEX, PALETTE_SLOT)));

    validate_registration_sets(sets.iter().map(|(name, pairs)| (*name, pairs.as_slice()))).unwrap();
}

#[test]
fn one_wrong_nazo_slot_one_mapping_invalidates_the_shared_font_path() {
    let mut sets =
        EXPECTED_REGISTRATION_SETS.map(|name| (name, pairs(IMAGE_LOAD_INDEX, PALETTE_SLOT)));
    sets[3].1[REGISTRATION_SLOT].image_load_index = IMAGE_LOAD_INDEX + 1;

    let error =
        validate_registration_sets(sets.iter().map(|(name, pairs)| (*name, pairs.as_slice())))
            .unwrap_err()
            .to_string();

    assert!(error.contains("RegBuff slot 1"));
}

#[test]
fn an_untracked_nazo_registration_set_invalidates_the_population() {
    let mut sets = EXPECTED_REGISTRATION_SETS
        .map(|name| (name, pairs(IMAGE_LOAD_INDEX, PALETTE_SLOT)))
        .to_vec();
    sets.push((
        "Nazo_TexReg_Unexpected",
        pairs(IMAGE_LOAD_INDEX, PALETTE_SLOT),
    ));

    let error =
        validate_registration_sets(sets.iter().map(|(name, pairs)| (*name, pairs.as_slice())))
            .unwrap_err()
            .to_string();

    assert!(error.contains("population changed"));
}
