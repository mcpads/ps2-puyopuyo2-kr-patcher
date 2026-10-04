use super::*;
use control_flow::{control_flow_boundaries_for_words, propagate_control_flow_words};
use propagation::{block_starts_for_words, propagate_straight_line_words};

const ZERO: u32 = 0;
const A0: u32 = 4;
const A1: u32 = 5;
const T0: u32 = 8;
const T1: u32 = 9;
const GP: u32 = 28;

#[test]
fn register_copy_preserves_a_computed_candidate_address() {
    let candidate = 0x0048_1234;
    let words = [lui(T0, 0x0048), addiu(T0, T0, 0x1234), addu(A0, T0, ZERO)];

    let productions = propagate_straight_line_words(
        &words,
        0x004f_8370,
        &BTreeMap::new(),
        &BTreeSet::from([candidate]),
    )
    .unwrap();

    assert_eq!(productions.len(), 2);
    assert_eq!(productions[1].candidate_address, candidate);
    assert_eq!(productions[1].evidence.len(), 3);
    assert!(!productions[1].uses_gp_initial_value);
}

#[test]
fn unknown_register_write_breaks_a_stale_constant_chain() {
    let candidate = 0x0048_1234;
    let words = [lui(T0, 0x0048), xor(T0, A0, A1), addiu(T0, T0, 0x1234)];

    let productions = propagate_straight_line_words(
        &words,
        0x004f_8370,
        &BTreeMap::new(),
        &BTreeSet::from([candidate]),
    )
    .unwrap();

    assert!(productions.is_empty());
}

#[test]
fn file_backed_pointer_load_records_its_slot() {
    let candidate = 0x0048_ca50;
    let words = [lui(T0, 0x0040), lw(T1, T0, 0x0100)];

    let productions = propagate_straight_line_words(
        &words,
        0x004f_8370,
        &BTreeMap::from([(0x0040_0100, candidate)]),
        &BTreeSet::from([candidate]),
    )
    .unwrap();

    assert_eq!(productions.len(), 1);
    assert_eq!(productions[0].pointer_slot_addresses, vec![0x0040_0100]);
}

#[test]
fn gp_copy_and_offset_keep_the_gp_origin() {
    let gp = 0x004f_8370;
    let candidate = gp - 0x100;
    let words = [or(T0, GP, ZERO), addiu(T0, T0, -0x100)];

    let productions =
        propagate_straight_line_words(&words, gp, &BTreeMap::new(), &BTreeSet::from([candidate]))
            .unwrap();

    assert_eq!(productions.len(), 1);
    assert!(productions[0].uses_gp_initial_value);
}

#[test]
fn branch_target_and_post_delay_slot_begin_separate_basic_blocks() {
    let words = [nop(), beq(ZERO, ZERO, 2), nop(), nop(), nop()];

    let starts = block_starts_for_words(&words).unwrap();

    assert_eq!(starts, BTreeSet::from([0, 3, 4]));
}

#[test]
fn matching_values_from_both_branch_paths_survive_a_control_flow_merge() {
    let candidate = 0x0048_0000;
    let words = [
        lui(T0, 0x0048),
        beq(A0, A1, 2),
        nop(),
        nop(),
        addu(A0, T0, ZERO),
    ];

    let productions = propagate_control_flow_words(
        &words,
        0x004f_8370,
        &BTreeMap::new(),
        &BTreeSet::from([candidate]),
    )
    .unwrap();

    assert_eq!(productions.len(), 2);
    assert_eq!(productions[1].result_instruction.address, "0x00400010");
    assert_eq!(productions[1].evidence.len(), 2);
}

#[test]
fn differing_values_from_branch_paths_are_dropped_at_the_merge() {
    let candidate = 0x0048_0000;
    let words = [
        lui(T0, 0x0048),
        beq(A0, A1, 4),
        nop(),
        addiu(T0, T0, 1),
        jump(0x0040_001c),
        nop(),
        nop(),
        addu(A0, T0, ZERO),
    ];

    let productions = propagate_control_flow_words(
        &words,
        0x004f_8370,
        &BTreeMap::new(),
        &BTreeSet::from([candidate]),
    )
    .unwrap();

    assert_eq!(productions.len(), 1);
    assert_eq!(productions[0].result_instruction.address, "0x00400000");
}

#[test]
fn branch_likely_delay_slot_does_not_feed_the_fallthrough_path() {
    let candidate = 0x0048_1234;
    let words = [
        beql(A0, A1, 2),
        lui(T0, 0x0048),
        addiu(A0, T0, 0x1234),
        nop(),
    ];

    let productions = propagate_control_flow_words(
        &words,
        0x004f_8370,
        &BTreeMap::new(),
        &BTreeSet::from([candidate]),
    )
    .unwrap();

    assert!(productions.is_empty());
}

#[test]
fn call_return_site_starts_with_a_conservative_register_state() {
    let candidate = 0x0048_1234;
    let words = [
        lui(T0, 0x0048),
        jal(0x0040_0010),
        addiu(T0, T0, 0x1234),
        addu(A0, T0, ZERO),
        nop(),
    ];

    let productions = propagate_control_flow_words(
        &words,
        0x004f_8370,
        &BTreeMap::new(),
        &BTreeSet::from([candidate]),
    )
    .unwrap();

    assert_eq!(productions.len(), 1);
    assert_eq!(productions[0].result_instruction.address, "0x00400008");
}

#[test]
fn direct_entry_into_another_branch_delay_slot_breaks_value_propagation() {
    let candidate = 0x0048_1234;
    let words = [
        lui(T0, 0x0048),
        beq(A0, A1, 2),
        nop(),
        beq(A0, A1, 2),
        addiu(A0, T0, 0x1234),
        nop(),
        nop(),
    ];

    let productions = propagate_control_flow_words(
        &words,
        0x004f_8370,
        &BTreeMap::new(),
        &BTreeSet::from([candidate]),
    )
    .unwrap();

    assert!(productions.is_empty());

    let boundaries = control_flow_boundaries_for_words(&words).unwrap();
    assert_eq!(boundaries.len(), 1);
    assert_eq!(boundaries[0].instruction_address, 0x0040_0004);
    assert_eq!(boundaries[0].delay_slot_address, 0x0040_0010);
    assert_eq!(
        boundaries[0].boundary_kind,
        "direct_control_target_is_delay_slot"
    );
}

#[test]
fn delay_slot_outside_the_symbol_is_recorded_as_a_conservative_boundary() {
    let boundaries = control_flow_boundaries_for_words(&[beq(A0, A1, 0)]).unwrap();

    assert_eq!(boundaries.len(), 1);
    assert_eq!(boundaries[0].instruction_address, 0x0040_0000);
    assert_eq!(boundaries[0].delay_slot_address, 0x0040_0004);
    assert_eq!(boundaries[0].boundary_kind, "delay_slot_outside_symbol");
}

fn lui(destination: u32, immediate: u16) -> u32 {
    (0x0f << 26) | (destination << 16) | u32::from(immediate)
}

fn addiu(destination: u32, source: u32, immediate: i16) -> u32 {
    (0x09 << 26) | (source << 21) | (destination << 16) | u32::from(immediate as u16)
}

fn addu(destination: u32, left: u32, right: u32) -> u32 {
    (left << 21) | (right << 16) | (destination << 11) | 0x21
}

fn xor(destination: u32, left: u32, right: u32) -> u32 {
    (left << 21) | (right << 16) | (destination << 11) | 0x26
}

fn or(destination: u32, left: u32, right: u32) -> u32 {
    (left << 21) | (right << 16) | (destination << 11) | 0x25
}

fn lw(destination: u32, base: u32, offset: i16) -> u32 {
    (0x23 << 26) | (base << 21) | (destination << 16) | u32::from(offset as u16)
}

fn beq(left: u32, right: u32, relative_words: i16) -> u32 {
    (0x04 << 26) | (left << 21) | (right << 16) | u32::from(relative_words as u16)
}

fn beql(left: u32, right: u32, relative_words: i16) -> u32 {
    (0x14 << 26) | (left << 21) | (right << 16) | u32::from(relative_words as u16)
}

fn jump(target: u32) -> u32 {
    (0x02 << 26) | ((target >> 2) & 0x03ff_ffff)
}

fn jal(target: u32) -> u32 {
    (0x03 << 26) | ((target >> 2) & 0x03ff_ffff)
}

fn nop() -> u32 {
    0
}
