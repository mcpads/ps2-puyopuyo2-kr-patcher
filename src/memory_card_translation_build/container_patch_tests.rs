use super::*;
use crate::indexed_texture::FileRecord;

fn resources(sector_offset: u32, indices: &[u8], palette: &[u8]) -> MemoryCardFontResources {
    MemoryCardFontResources {
        image_record_index: 0,
        image_record: FileRecord {
            sector_offset,
            sector_count: 1,
            stored_byte_length: 3,
            width: 2,
            height: 2,
        },
        palette_record_index: 1,
        palette: palette.to_vec(),
        linear_indices: indices.to_vec(),
    }
}

#[test]
fn shared_replacement_is_written_to_both_consumer_font_slots() {
    let indices = [0, 1, 2, 3];
    let palette = [0, 0, 0, 0];
    let primary_resources = resources(1, &indices, &palette);
    let nazo_resources = resources(2, &indices, &palette);
    let mut primary_source = vec![0x11; CPX_BLOCK_ALIGNMENT * 3];
    let mut nazo_source = vec![0x22; CPX_BLOCK_ALIGNMENT * 4];
    let source_cpx = [0x43, 0x50, 0x58, 0x00];
    primary_source[CPX_BLOCK_ALIGNMENT..CPX_BLOCK_ALIGNMENT + source_cpx.len()]
        .copy_from_slice(&source_cpx);
    nazo_source[CPX_BLOCK_ALIGNMENT * 2..CPX_BLOCK_ALIGNMENT * 2 + source_cpx.len()]
        .copy_from_slice(&source_cpx);
    let replacement = [0xaa, 0xbb, 0xcc, 0xdd];

    let patched = replace_shared_font_clones(
        FontContainerTarget {
            container_iso_path: "/IMAGE_D.BIN;1",
            region_id: "primary",
            region_reason: "primary font",
            write_id: "write-primary",
            source: &primary_source,
            resources: &primary_resources,
        },
        FontContainerTarget {
            container_iso_path: "/IMAGE_G.BIN;1",
            region_id: "nazo",
            region_reason: "Nazo menu font clone",
            write_id: "write-nazo",
            source: &nazo_source,
            resources: &nazo_resources,
        },
        &replacement,
    )
    .unwrap();

    assert_eq!(
        &patched.primary.bytes[CPX_BLOCK_ALIGNMENT..CPX_BLOCK_ALIGNMENT + replacement.len()],
        replacement
    );
    assert_eq!(
        &patched.nazo_menu.bytes
            [CPX_BLOCK_ALIGNMENT * 2..CPX_BLOCK_ALIGNMENT * 2 + replacement.len()],
        replacement
    );
    assert_eq!(patched.primary.expected_writes, 1);
    assert_eq!(patched.nazo_menu.expected_writes, 1);
    assert_eq!(
        patched.primary.source_cpx_sha256,
        patched.nazo_menu.source_cpx_sha256
    );
    assert_eq!(
        patched.primary.replacement_cpx_sha256,
        patched.nazo_menu.replacement_cpx_sha256
    );
    assert!(
        patched.primary.bytes[..CPX_BLOCK_ALIGNMENT]
            .iter()
            .all(|byte| *byte == 0x11)
    );
    assert!(
        patched.nazo_menu.bytes[..CPX_BLOCK_ALIGNMENT * 2]
            .iter()
            .all(|byte| *byte == 0x22)
    );
}

#[test]
fn differing_source_cpx_bytes_are_rejected_before_any_write() {
    let indices = [0, 1, 2, 3];
    let palette = [0, 0, 0, 0];
    let primary_resources = resources(0, &indices, &palette);
    let nazo_resources = resources(0, &indices, &palette);
    let primary_source = vec![0x11; CPX_BLOCK_ALIGNMENT];
    let nazo_source = vec![0x22; CPX_BLOCK_ALIGNMENT];

    let error = replace_shared_font_clones(
        FontContainerTarget {
            container_iso_path: "/IMAGE_D.BIN;1",
            region_id: "primary",
            region_reason: "primary font",
            write_id: "write-primary",
            source: &primary_source,
            resources: &primary_resources,
        },
        FontContainerTarget {
            container_iso_path: "/IMAGE_G.BIN;1",
            region_id: "nazo",
            region_reason: "Nazo menu font clone",
            write_id: "write-nazo",
            source: &nazo_source,
            resources: &nazo_resources,
        },
        &[1],
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("CPX byte clones"));
}

#[test]
fn differing_source_clone_indices_are_rejected_before_any_write() {
    let palette = [0, 0, 0, 0];
    let primary_resources = resources(0, &[0, 1, 2, 3], &palette);
    let nazo_resources = resources(0, &[0, 1, 2, 4], &palette);
    let source = vec![0; CPX_BLOCK_ALIGNMENT];

    let error = replace_shared_font_clones(
        FontContainerTarget {
            container_iso_path: "/IMAGE_D.BIN;1",
            region_id: "primary",
            region_reason: "primary font",
            write_id: "write-primary",
            source: &source,
            resources: &primary_resources,
        },
        FontContainerTarget {
            container_iso_path: "/IMAGE_G.BIN;1",
            region_id: "nazo",
            region_reason: "Nazo menu font clone",
            write_id: "write-nazo",
            source: &source,
            resources: &nazo_resources,
        },
        &[1],
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("not exact index clones"));
}
