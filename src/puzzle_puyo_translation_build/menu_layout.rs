use anyhow::{Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use serde::Deserialize;

use crate::executable::{disassemble_symbol, read_executable_symbol};
use crate::graphics_translation::HashedAsset;
use crate::indexed_texture::sha256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MenuLayout {
    center_x: i16,
    source_positions_hex: String,
    source_descriptor_hex: String,
    consumer_sha256: String,
    caller_sha256: String,
    ink_bounds: [[i16; 2]; 4],
    dependencies: Vec<HashedAsset>,
}

fn centered_positions(source: &[u8], center: i16, bounds: &[[i16; 2]; 4]) -> Result<Vec<u8>> {
    ensure!(
        source.len() == 34 && source[32..] == [255, 255],
        "puzzle menu position table changed"
    );
    let mut result = source.to_vec();
    for (index, [left, right]) in bounds.iter().copied().enumerate() {
        let offset = index * 8;
        ensure!(
            i16::from_le_bytes(source[offset..offset + 2].try_into()?) == index as i16 + 1,
            "puzzle menu pattern order changed"
        );
        ensure!(
            left >= 0 && left < right && right <= 240,
            "puzzle menu ink exceeds its pattern"
        );
        // Bounds are half-open. Nearest integer placement keeps the visible
        // center within half a pixel without scaling any glyph or source art.
        let x = i32::from(center) - (i32::from(left) + i32::from(right)) / 2;
        result[offset + 2..offset + 4].copy_from_slice(&i16::try_from(x)?.to_le_bytes());
    }
    Ok(result)
}

fn replacement(source: &[u8]) -> Result<(usize, Vec<u8>, Vec<u8>)> {
    let layout: MenuLayout =
        serde_json::from_slice(&std::fs::read("config/puzzle-menu-layout.json")?)?;
    for dependency in &layout.dependencies {
        ensure!(
            sha256(&std::fs::read(&dependency.path)?) == dependency.sha256,
            "puzzle menu artwork changed; remeasure complete item ink bounds: {}",
            dependency.path
        );
    }
    for (name, expected) in [
        ("Make_Menu_Item", &layout.consumer_sha256),
        ("Nazo_Menu_Run", &layout.caller_sha256),
    ] {
        disassemble_symbol(source, name)?;
        ensure!(
            sha256(&read_executable_symbol(source, name)?.bytes) == *expected,
            "puzzle menu position consumer changed: {name}"
        );
    }
    let descriptor = read_executable_symbol(source, "Nazo_TopMenu")?;
    ensure!(
        super::executable_patch::decode_hex(&layout.source_descriptor_hex)? == descriptor.bytes,
        "puzzle top menu descriptor changed"
    );
    let positions = read_executable_symbol(source, "Nazo_TopMenuPos")?;
    ensure!(
        super::executable_patch::decode_hex(&layout.source_positions_hex)? == positions.bytes,
        "puzzle top menu source positions changed"
    );
    let result = centered_positions(&positions.bytes, layout.center_x, &layout.ink_bounds)?;
    Ok((positions.file_offset, positions.bytes, result))
}

pub(super) fn add_writes(plan: WritePlan, source: &[u8]) -> Result<WritePlan> {
    let (offset, original, replacement) = replacement(source)?;
    Ok(plan
        .region(ImageRegion {
            id: "puzzle-top-menu-positions".into(),
            range: offset..offset + original.len(),
            kind: RegionKind::Data,
            reason: "verified Nazo_TopMenuPos data, not executable instructions".into(),
        })
        .write(ExpectedWrite {
            id: "center-puzzle-menu-items".into(),
            owner: "puzzle-menu-layout".into(),
            purpose:
                "center complete Korean, numeral and My wordmarks without modifying source glyphs"
                    .into(),
            offset,
            expected_original: original,
            replacement,
            intent: WriteIntent::Data,
        }))
}

pub(super) fn verify(source: &[u8], patched: &[u8]) -> Result<()> {
    let (offset, _, replacement) = replacement(source)?;
    ensure!(
        patched[offset..offset + replacement.len()] == replacement,
        "puzzle menu layout readback differs"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_whole_items_preserving_row_pattern_depth_and_terminator() {
        let mut original = Vec::new();
        for i in 0..4_i16 {
            for v in [i + 1, 200, 200 + i * 48, 1000] {
                original.extend(v.to_le_bytes());
            }
        }
        original.extend((-1_i16).to_le_bytes());
        let bounds = [[86, 193], [84, 199], [86, 153], [24, 174]];
        let result = centered_positions(&original, 320, &bounds).unwrap();
        for (i, [left, right]) in bounds.into_iter().enumerate() {
            let x = i16::from_le_bytes(result[i * 8 + 2..i * 8 + 4].try_into().unwrap());
            assert!((2 * x + left + right - 640).abs() <= 1);
        }
        for i in 0..original.len() {
            if i % 8 != 2 && i % 8 != 3 {
                assert_eq!(result[i], original[i]);
            }
        }
        original[0] = 2;
        assert!(centered_positions(&original, 320, &bounds).is_err());
    }
}
