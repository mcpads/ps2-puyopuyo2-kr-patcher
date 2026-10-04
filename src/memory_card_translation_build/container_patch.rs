use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use super::font::MemoryCardFontResources;
use crate::cpx::CPX_BLOCK_ALIGNMENT;
use crate::indexed_texture::sha256;

pub(super) struct FontContainerTarget<'a> {
    pub(super) container_iso_path: &'static str,
    pub(super) region_id: &'static str,
    pub(super) region_reason: &'static str,
    pub(super) write_id: &'static str,
    pub(super) source: &'a [u8],
    pub(super) resources: &'a MemoryCardFontResources,
}

#[derive(Debug)]
pub(super) struct PatchedFontContainer {
    pub(super) bytes: Vec<u8>,
    pub(super) source_cpx_sha256: String,
    pub(super) replacement_cpx_sha256: String,
    pub(super) source_cpx_byte_length: usize,
    pub(super) replacement_cpx_byte_length: usize,
    pub(super) slot_byte_length: usize,
    pub(super) padding_byte_length: usize,
    pub(super) expected_writes: usize,
}

#[derive(Debug)]
pub(super) struct PatchedFontClones {
    pub(super) primary: PatchedFontContainer,
    pub(super) nazo_menu: PatchedFontContainer,
}

pub(super) fn replace_shared_font_clones(
    primary: FontContainerTarget<'_>,
    nazo_menu: FontContainerTarget<'_>,
    replacement_cpx: &[u8],
) -> Result<PatchedFontClones> {
    ensure!(
        primary.resources.image_record.width == nazo_menu.resources.image_record.width
            && primary.resources.image_record.height == nazo_menu.resources.image_record.height,
        "memory-card source font surfaces do not have identical dimensions"
    );
    ensure!(
        primary.resources.linear_indices == nazo_menu.resources.linear_indices,
        "memory-card source font surfaces are not exact index clones"
    );
    ensure!(
        primary.resources.palette == nazo_menu.resources.palette,
        "memory-card source font surfaces do not share an exact palette"
    );
    let primary_source_cpx = source_cpx_bytes(&primary)?;
    let nazo_source_cpx = source_cpx_bytes(&nazo_menu)?;
    ensure!(
        primary_source_cpx == nazo_source_cpx,
        "memory-card source font surfaces are not exact CPX byte clones"
    );
    Ok(PatchedFontClones {
        primary: replace_font_container(primary, replacement_cpx)?,
        nazo_menu: replace_font_container(nazo_menu, replacement_cpx)?,
    })
}

fn replace_font_container(
    target: FontContainerTarget<'_>,
    replacement_cpx: &[u8],
) -> Result<PatchedFontContainer> {
    let slot_offset = usize::try_from(target.resources.image_record.sector_offset)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .with_context(|| {
            format!(
                "{} memory-card font CPX slot offset overflow",
                target.container_iso_path
            )
        })?;
    let slot_byte_length = usize::try_from(target.resources.image_record.sector_count)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .with_context(|| {
            format!(
                "{} memory-card font CPX slot length overflow",
                target.container_iso_path
            )
        })?;
    ensure!(
        replacement_cpx.len() <= slot_byte_length,
        "replacement memory-card font CPX is {} bytes but {} has only {slot_byte_length} bytes",
        replacement_cpx.len(),
        target.container_iso_path
    );
    let slot_end = slot_offset
        .checked_add(slot_byte_length)
        .context("memory-card font CPX slot range overflow")?;
    let source_slot = target.source.get(slot_offset..slot_end).with_context(|| {
        format!(
            "{} font slot exceeds its container",
            target.container_iso_path
        )
    })?;
    let source_cpx_byte_length =
        usize::try_from(target.resources.image_record.stored_byte_length)? + 1;
    ensure!(
        source_cpx_byte_length <= slot_byte_length,
        "{} source font CPX length exceeds its fixed slot",
        target.container_iso_path
    );

    let mut replacement_slot = replacement_cpx.to_vec();
    replacement_slot.resize(slot_byte_length, 0);
    let plan = WritePlan::new()
        .region(ImageRegion {
            id: target.region_id.to_string(),
            range: slot_offset..slot_end,
            kind: RegionKind::Data,
            reason: target.region_reason.to_string(),
        })
        .write(ExpectedWrite {
            id: target.write_id.to_string(),
            owner: "memory-card-translation-development".to_string(),
            purpose: "insert the complete pre-human-review Korean memory-card glyph atlas"
                .to_string(),
            offset: slot_offset,
            expected_original: source_slot.to_vec(),
            replacement: replacement_slot,
            intent: WriteIntent::Data,
        });
    let expected_writes = plan.writes.len();
    Ok(PatchedFontContainer {
        bytes: plan.apply(target.source, None)?,
        source_cpx_sha256: sha256(&source_slot[..source_cpx_byte_length]),
        replacement_cpx_sha256: sha256(replacement_cpx),
        source_cpx_byte_length,
        replacement_cpx_byte_length: replacement_cpx.len(),
        slot_byte_length,
        padding_byte_length: slot_byte_length - replacement_cpx.len(),
        expected_writes,
    })
}

fn source_cpx_bytes<'a>(target: &'a FontContainerTarget<'_>) -> Result<&'a [u8]> {
    let offset = usize::try_from(target.resources.image_record.sector_offset)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .with_context(|| format!("{} source CPX offset overflow", target.container_iso_path))?;
    let length = usize::try_from(target.resources.image_record.stored_byte_length)?
        .checked_add(1)
        .context("source CPX byte length overflow")?;
    target.source.get(offset..offset + length).with_context(|| {
        format!(
            "{} source CPX exceeds its container",
            target.container_iso_path
        )
    })
}

#[cfg(test)]
#[path = "container_patch_tests.rs"]
mod container_patch_tests;
