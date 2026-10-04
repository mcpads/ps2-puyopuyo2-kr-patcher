use super::catalog::{
    LOAD_DIRECT_REFERENCES, LOAD_TABLE_REFERENCES, MY_NAZO_ERROR_DIRECT_REFERENCES,
    SAVE_DIRECT_REFERENCES, SAVE_TABLE_REFERENCES, TUTORIAL_MESSAGES, actor,
};
use super::parsing::parse_tutorial_font_table;
use super::survey::find_actor_creation_specs;
use super::*;

#[test]
fn load_catalog_covers_the_declared_consumer_population() {
    let sites = LOAD_DIRECT_REFERENCES.len() + LOAD_TABLE_REFERENCES.len();
    let messages: BTreeSet<&str> = LOAD_DIRECT_REFERENCES
        .iter()
        .map(|reference| reference.message.symbol)
        .chain(
            LOAD_TABLE_REFERENCES
                .iter()
                .flat_map(|reference| reference.messages.iter().map(|message| message.symbol)),
        )
        .collect();
    assert_eq!(sites, 15);
    assert_eq!(messages.len(), 29);
}

#[test]
fn save_catalog_covers_the_declared_consumer_population() {
    let sites = SAVE_DIRECT_REFERENCES.len() + SAVE_TABLE_REFERENCES.len();
    let messages: BTreeSet<&str> = SAVE_DIRECT_REFERENCES
        .iter()
        .map(|reference| reference.message.symbol)
        .chain(
            SAVE_TABLE_REFERENCES
                .iter()
                .flat_map(|reference| reference.messages.iter().map(|message| message.symbol)),
        )
        .collect();
    let physical_edges = SAVE_DIRECT_REFERENCES.len()
        + SAVE_TABLE_REFERENCES
            .iter()
            .map(|reference| reference.messages.len())
            .sum::<usize>();
    assert_eq!(sites, 32);
    assert_eq!(physical_edges, 59);
    assert_eq!(messages.len(), 49);
}

#[test]
fn my_nazo_error_catalog_covers_the_declared_consumer_population() {
    let sites = MY_NAZO_ERROR_DIRECT_REFERENCES.len();
    let messages: BTreeSet<&str> = MY_NAZO_ERROR_DIRECT_REFERENCES
        .iter()
        .map(|reference| reference.message.symbol)
        .collect();
    assert_eq!(sites, 2);
    assert_eq!(messages.len(), 2);
}

#[test]
fn tutorial_catalog_preserves_each_pointer_table_entry() {
    let ids: BTreeSet<&str> = TUTORIAL_MESSAGES.iter().map(|message| message.id).collect();
    let symbols: BTreeSet<&str> = TUTORIAL_MESSAGES
        .iter()
        .map(|message| message.symbol)
        .collect();

    assert_eq!(ids.len(), TUTORIAL_MESSAGES.len());
    assert_eq!(symbols.len(), TUTORIAL_MESSAGES.len());
    assert_eq!(TUTORIAL_MESSAGES.len(), 12);
}

#[test]
fn global_membership_reuses_every_declared_message_catalog() {
    let memberships = declared_message_corpus_memberships();

    assert_eq!(memberships.len(), 123);
    assert_eq!(memberships["ExpMsg_Data_0"], ["tutorial"]);
    assert_eq!(memberships["NazoQes_Str_1"], ["nazo"]);
    assert_eq!(
        memberships["MemLoadMsg_BtnOK"],
        ["memory_card_load", "my_nazo_error"]
    );
}

#[test]
fn tutorial_font_table_preserves_repeated_physical_slots() {
    let table =
        parse_tutorial_font_table(&[0x81, 0x40, 0x82, 0xa0, 0x81, 0x40, 0x81, 0x40, 0]).unwrap();

    assert_eq!(table.code_slots.len(), 4);
    assert_eq!(table.unique_code_units.len(), 2);
    assert_eq!(table.repeated_code_units.len(), 1);
    assert_eq!(table.repeated_code_units[0].raw_hex, "8140");
    assert_eq!(table.repeated_code_units[0].occurrences, 3);
    assert_eq!(table.repeated_code_units[0].first_slot, 0);
}

#[test]
fn renderer_actor_scan_accepts_low_address_before_or_after_the_setter_call() {
    let adjacent_low = [
        0x45, 0x00, 0x04, 0x3c, 0x60, 0xf0, 0x84, 0x24, 0xd8, 0xc9, 0x10, 0x0c, 0x0f, 0x00, 0x05,
        0x24,
    ];
    let adjacent =
        find_actor_creation_specs(&adjacent_low, 0x0045_91b8, 0x0044_f060, 0x0043_2760).unwrap();
    assert_eq!(adjacent, [actor(0, 4, 8)]);

    let delay_slot_low = [
        0x46, 0x00, 0x04, 0x3c, 0x0f, 0x00, 0x05, 0x24, 0xd8, 0xc9, 0x10, 0x0c, 0x20, 0xec, 0x84,
        0x24,
    ];
    let delay_slot =
        find_actor_creation_specs(&delay_slot_low, 0x0045_f154, 0x0045_ec20, 0x0043_2760).unwrap();
    assert_eq!(delay_slot, [actor(0, 12, 8)]);

    let separated_setup = [
        0x46, 0x00, 0x04, 0x3c, 0x30, 0x6a, 0x42, 0x24, 0x20, 0xec, 0x84, 0x24, 0x0f, 0x00, 0x05,
        0x24, 0x00, 0x89, 0x10, 0x00, 0xd8, 0xc9, 0x10, 0x0c, 0x21, 0x98, 0x51, 0x00,
    ];
    let separated =
        find_actor_creation_specs(&separated_setup, 0x0046_4988, 0x0045_ec20, 0x0043_2760).unwrap();
    assert_eq!(separated, [actor(0, 8, 20)]);

    let mut wrong_setter = adjacent_low;
    wrong_setter[11] = 0;
    assert!(
        find_actor_creation_specs(&wrong_setter, 0x0045_91b8, 0x0044_f060, 0x0043_2760,)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn message_tokens_roundtrip_text_layout_and_controls() {
    let source = [
        b'M', b'E', b'M', b'O', b'R', b'Y', b' ', b'@', b'P', 0x82, 0xa0, b'\n', b'@', b'A', b'@',
        b'0', 0,
    ];
    let tokens = parse_message(&source).unwrap();
    assert_eq!(reconstruct_message(&tokens), source);
    assert_eq!(
        display_message(&tokens),
        "MEMORY {dynamic-port}あ\n{button:A}{color:0}"
    );
}

#[test]
fn message_parser_rejects_unknown_or_truncated_controls() {
    assert!(
        parse_message(b"@?\0")
            .unwrap_err()
            .to_string()
            .contains("unsupported")
    );
    assert!(
        parse_message(b"@\0")
            .unwrap_err()
            .to_string()
            .contains("truncated")
    );
}

#[test]
fn message_parser_rejects_noncanonical_shift_jis_and_missing_nul() {
    assert!(parse_message(&[0x82, 0]).is_err());
    assert!(parse_message(&[0x82, 0xa0]).is_err());
}
