use super::*;

const NOP: [u8; 4] = [0, 0, 0, 0];
const IMAGE_G_FILE_ID: [u8; 4] = [0x01, 0x00, 0x04, 0x24];

#[test]
fn regist2_accepts_image_g_file_id_immediately_before_the_call() {
    let mut instructions = Vec::new();
    instructions.extend_from_slice(&IMAGE_G_FILE_ID);
    instructions.extend_from_slice(&NOP);
    instructions.extend_from_slice(&NOP);

    validate_regist2_image_file_id(&instructions, 0x0040_0000, 4, "owner").unwrap();
}

#[test]
fn regist2_accepts_image_g_file_id_in_the_delay_slot() {
    let mut instructions = Vec::new();
    instructions.extend_from_slice(&NOP);
    instructions.extend_from_slice(&NOP);
    instructions.extend_from_slice(&IMAGE_G_FILE_ID);

    validate_regist2_image_file_id(&instructions, 0x0040_0000, 4, "owner").unwrap();
}

#[test]
fn regist2_rejects_a_different_image_file_id() {
    let instructions = [NOP, NOP, NOP].concat();

    let error = validate_regist2_image_file_id(&instructions, 0x0040_0000, 4, "owner")
        .unwrap_err()
        .to_string();

    assert!(error.contains("IMAGE_G file id 1"));
}
