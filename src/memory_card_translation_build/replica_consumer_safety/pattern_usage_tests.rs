use super::*;
use crate::executable::validate_r5900_instruction;

fn pattern(registration_slot: usize) -> TexturePatternRecord {
    TexturePatternRecord {
        image_load_index: registration_slot,
        source_x: 0,
        source_y: 0,
        width: 24,
        height: 24,
    }
}

#[test]
fn named_owner_patterns_without_slot_one_are_admitted() {
    validate_no_slot_one_patterns("owner-patterns", &[pattern(0), pattern(8)]).unwrap();
}

#[test]
fn a_fixed_slot_one_pattern_blocks_g122_reassignment() {
    let error = validate_no_slot_one_patterns("owner-patterns", &[pattern(1)])
        .unwrap_err()
        .to_string();

    assert!(error.contains("G122 glyph reassignment is unsafe"));
}

#[test]
fn direct_pattern_slot_accepts_typed_addiu_immediates() {
    let instruction = validate_r5900_instruction(&[0x02, 0x00, 0x05, 0x24], 0x0040_0000)
        .expect("addiu a1, zero, 2 is valid R5900");

    assert_eq!(immediate_a1_value(&instruction), Some(2));
}

#[test]
fn direct_pattern_slot_accepts_the_typed_zero_move_form() {
    let instruction = validate_r5900_instruction(&[0x2d, 0x28, 0x00, 0x00], 0x0040_0000)
        .expect("daddu a1, zero, zero is valid R5900");

    assert_eq!(immediate_a1_value(&instruction), Some(0));
}
