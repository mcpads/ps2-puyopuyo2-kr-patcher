use super::*;

#[test]
fn typed_validation_round_trips_machine_code_and_derives_semantics() {
    let source = [0x10, 0x00, 0xa5, 0x24, 0x00, 0x00, 0x00, 0x00];
    let decoded = decode_exact(&source, 0x0042_4fe0).unwrap();
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].bytes, "1000a524");
    assert!(!decoded[0].canonical.is_empty());
}

#[test]
fn typed_decode_rejects_partial_instruction() {
    let error = decode_exact(&[0; 3], 0x0040_0000).unwrap_err().to_string();
    assert!(error.contains("four-byte aligned"));
}

#[test]
fn typed_validation_rejects_locations_outside_the_ee_address_space() {
    let error = validate_r5900_instruction(&[0; 4], 0x1_0000_0000)
        .unwrap_err()
        .to_string();
    assert!(error.contains("exceeds 32 bits"));
}
