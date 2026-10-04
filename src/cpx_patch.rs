use anyhow::{Context, Result};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

const IMAGE_FILE_RECORD_BYTES: usize = 20;
const STORED_LENGTH_FIELD_OFFSET: usize = 8;

pub(crate) struct FileRecordStoredLengthPatch<'a> {
    pub file_list_offset: usize,
    pub record_index: usize,
    pub expected_stored_length: u32,
    pub replacement_cpx_length: usize,
    pub region_id: &'a str,
    pub region_reason: &'a str,
    pub write_id: &'a str,
    pub owner: &'a str,
    pub purpose: &'a str,
}

pub(crate) struct FileRecordStoredLengthWrite {
    pub file_list_offset: usize,
    pub record_index: usize,
    pub expected_stored_length: u32,
    pub replacement_cpx_length: usize,
    pub region_id: String,
    pub region_reason: String,
    pub write_id: String,
    pub owner: String,
    pub purpose: String,
}

pub(crate) fn patch_file_record_stored_length(
    executable: &[u8],
    patch: FileRecordStoredLengthPatch<'_>,
) -> Result<(Vec<u8>, usize)> {
    patch_file_record_stored_lengths(
        executable,
        &[FileRecordStoredLengthWrite {
            file_list_offset: patch.file_list_offset,
            record_index: patch.record_index,
            expected_stored_length: patch.expected_stored_length,
            replacement_cpx_length: patch.replacement_cpx_length,
            region_id: patch.region_id.to_string(),
            region_reason: patch.region_reason.to_string(),
            write_id: patch.write_id.to_string(),
            owner: patch.owner.to_string(),
            purpose: patch.purpose.to_string(),
        }],
    )
}

pub(crate) fn patch_file_record_stored_lengths(
    executable: &[u8],
    patches: &[FileRecordStoredLengthWrite],
) -> Result<(Vec<u8>, usize)> {
    let mut plan = WritePlan::new();
    for patch in patches {
        let offset = patch
            .file_list_offset
            .checked_add(patch.record_index * IMAGE_FILE_RECORD_BYTES + STORED_LENGTH_FIELD_OFFSET)
            .context("CPX stored-length field offset overflow")?;
        plan = plan.region(ImageRegion {
            id: patch.region_id.clone(),
            range: offset..offset + 4,
            kind: RegionKind::Metadata,
            reason: patch.region_reason.clone(),
        });
        plan = plan.write(ExpectedWrite {
            id: patch.write_id.clone(),
            owner: patch.owner.clone(),
            purpose: patch.purpose.clone(),
            offset,
            expected_original: patch.expected_stored_length.to_le_bytes().to_vec(),
            replacement: u32::try_from(
                patch
                    .replacement_cpx_length
                    .checked_sub(1)
                    .context("replacement CPX is empty")?,
            )?
            .to_le_bytes()
            .to_vec(),
            intent: WriteIntent::Metadata,
        });
    }
    let write_count = plan.writes.len();
    Ok((plan.apply(executable, None)?, write_count))
}

#[cfg(test)]
#[path = "cpx_patch_tests.rs"]
mod cpx_patch_tests;
