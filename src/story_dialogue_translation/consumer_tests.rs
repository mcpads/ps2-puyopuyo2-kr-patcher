use super::consumer::validate_story_dialogue_texture_cell_instructions;

const PRODUCER_ADDRESS: u64 = 0x0045_9ed0;
const PACKET_CONSUMER_ADDRESS: u64 = 0x0042_5f40;
const ALLOCATOR_ADDRESS: u64 = 0x0042_5990;
const MEMORY_COPY_ADDRESS: u64 = 0x0042_01a8;

#[test]
fn accepts_story_texture_rectangles_measured_in_twenty_four_pixel_cells() {
    let (producer, packet_consumer) = fixed_story_texture_consumer();

    validate_story_dialogue_texture_cell_instructions(
        &producer,
        PRODUCER_ADDRESS,
        &packet_consumer,
        PACKET_CONSUMER_ADDRESS,
        ALLOCATOR_ADDRESS,
        MEMORY_COPY_ADDRESS,
    )
    .unwrap();
}

#[test]
fn rejects_a_story_texture_width_not_scaled_by_twenty_four() {
    let (mut producer, packet_consumer) = fixed_story_texture_consumer();
    producer[0x394] = 0x80;

    let error = validate_story_dialogue_texture_cell_instructions(
        &producer,
        PRODUCER_ADDRESS,
        &packet_consumer,
        PACKET_CONSUMER_ADDRESS,
        ALLOCATOR_ADDRESS,
        MEMORY_COPY_ADDRESS,
    )
    .unwrap_err();

    assert!(error.to_string().contains("story texture width"));
}

#[test]
fn rejects_a_width_field_not_consumed_as_the_source_rectangle_extent() {
    let (producer, mut packet_consumer) = fixed_story_texture_consumer();
    packet_consumer[0x22c..0x230].copy_from_slice(&[0, 0, 0, 0]);

    let error = validate_story_dialogue_texture_cell_instructions(
        &producer,
        PRODUCER_ADDRESS,
        &packet_consumer,
        PACKET_CONSUMER_ADDRESS,
        ALLOCATOR_ADDRESS,
        MEMORY_COPY_ADDRESS,
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("sprite packet source rectangle right edge")
    );
}

fn fixed_story_texture_consumer() -> (Vec<u8>, Vec<u8>) {
    let mut producer = vec![0; 0x430];
    for (offset, instruction) in [
        (0x104, [0x24, 0x00, 0x72, 0x8e]),
        (0x388, [0xd0, 0x00, 0x63, 0x8e]),
        (0x38c, [0x40, 0x10, 0x03, 0x00]),
        (0x390, [0x21, 0x10, 0x43, 0x00]),
        (0x394, [0xc0, 0x10, 0x02, 0x00]),
        (0x398, [0x0c, 0x00, 0x42, 0xae]),
        (0x39c, [0xd4, 0x00, 0x63, 0x8e]),
        (0x3a0, [0x40, 0x10, 0x03, 0x00]),
        (0x3a4, [0x21, 0x10, 0x43, 0x00]),
        (0x3a8, [0xc0, 0x10, 0x02, 0x00]),
        (0x3ac, [0x10, 0x00, 0x42, 0xae]),
        (0x3b0, [0x14, 0x00, 0x40, 0xae]),
        (0x3b4, [0x20, 0x01, 0x63, 0x8e]),
        (0x3b8, [0x40, 0x10, 0x03, 0x00]),
        (0x3bc, [0x21, 0x10, 0x43, 0x00]),
        (0x3c0, [0xc0, 0x10, 0x02, 0x00]),
        (0x3c4, [0x64, 0x96, 0x10, 0x0c]),
        (0x3c8, [0x18, 0x00, 0x42, 0xae]),
        (0x424, [0x24, 0x00, 0x65, 0x8e]),
        (0x428, [0x6a, 0x80, 0x10, 0x0c]),
        (0x42c, [0x48, 0x00, 0x06, 0x24]),
    ] {
        producer[offset..offset + 4].copy_from_slice(&instruction);
    }

    let mut packet_consumer = vec![0; 0x238];
    for (offset, instruction) in [
        (0x1c8, [0x14, 0x00, 0x83, 0x8e]),
        (0x1cc, [0x18, 0x00, 0x82, 0x8e]),
        (0x218, [0x14, 0x00, 0x86, 0x8e]),
        (0x21c, [0x0c, 0x00, 0x85, 0x8e]),
        (0x220, [0x18, 0x00, 0x83, 0x8e]),
        (0x224, [0x10, 0x00, 0x82, 0x8e]),
        (0x22c, [0x21, 0x28, 0xc5, 0x00]),
        (0x234, [0x21, 0x10, 0x62, 0x00]),
    ] {
        packet_consumer[offset..offset + 4].copy_from_slice(&instruction);
    }
    (producer, packet_consumer)
}
