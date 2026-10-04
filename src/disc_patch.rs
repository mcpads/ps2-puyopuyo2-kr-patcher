use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
#[cfg(target_os = "macos")]
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use sha2::{Digest, Sha256};

use crate::disc::{
    FORM1_INTEGRITY_OFFSET, FORM1_INTEGRITY_SIZE, RAW_SECTOR_SIZE, RawDisc, USER_DATA_OFFSET,
    USER_DATA_SIZE, regenerate_form1_integrity, validate_form1_integrity,
};

pub(crate) struct IsoFileCandidate {
    pub(crate) iso_path: String,
    pub(crate) extent_lba: u32,
    pub(crate) original: Vec<u8>,
    pub(crate) replacement: Vec<u8>,
}

pub(crate) struct SectorPatch {
    pub(crate) lba: u32,
    pub(crate) source_bytes: [u8; RAW_SECTOR_SIZE],
    pub(crate) bytes: [u8; RAW_SECTOR_SIZE],
    pub(crate) data_writes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComponentBuildMode {
    WriteArtifact,
    ReturnSectorPatches,
}

pub(crate) enum ComponentBuildOutput<T> {
    Artifact(T),
    SectorPatches(Vec<SectorPatch>),
}

#[derive(Debug)]
struct AccumulatedSectorPatch {
    source_raw: [u8; RAW_SECTOR_SIZE],
    merged_user_data: [u8; USER_DATA_SIZE],
    owner_by_offset: Box<[Option<usize>]>,
}

#[derive(Debug, Default)]
pub(crate) struct SectorPatchAccumulator {
    owners: Vec<String>,
    sectors: BTreeMap<u32, AccumulatedSectorPatch>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SectorPatchMergeReport {
    pub(crate) declared_sectors: usize,
    pub(crate) sectors_already_touched: usize,
    pub(crate) changed_user_data_bytes: usize,
    pub(crate) added_user_data_bytes: usize,
    pub(crate) repeated_identical_user_data_bytes: usize,
}

impl SectorPatchAccumulator {
    pub(crate) fn merge_sector_patches(
        &mut self,
        patches: &[SectorPatch],
        owner: &str,
    ) -> Result<SectorPatchMergeReport> {
        ensure!(!owner.is_empty(), "sector-patch owner is empty");
        ensure!(
            !self.owners.iter().any(|known| known == owner),
            "sector-patch owner is duplicated: {owner}"
        );
        ensure!(!patches.is_empty(), "component {owner} planned no sectors");
        ensure!(
            patches.windows(2).all(|pair| pair[0].lba < pair[1].lba),
            "component {owner} LBAs are not strictly increasing"
        );

        let owner_index = self.owners.len();
        self.owners.push(owner.to_string());
        let mut report = SectorPatchMergeReport {
            declared_sectors: patches.len(),
            sectors_already_touched: 0,
            changed_user_data_bytes: 0,
            added_user_data_bytes: 0,
            repeated_identical_user_data_bytes: 0,
        };

        for patch in patches {
            let lba = patch.lba;
            let source_raw = patch.source_bytes;
            let candidate_raw = patch.bytes;
            ensure!(
                patch.data_writes > 0,
                "component {owner} declared LBA {lba} without data writes"
            );
            validate_form1_integrity(&source_raw)
                .with_context(|| format!("source integrity failed at component LBA {lba}"))?;
            validate_form1_integrity(&candidate_raw).with_context(|| {
                format!("component {owner} integrity failed at planned LBA {lba}")
            })?;
            ensure!(
                source_raw[..USER_DATA_OFFSET] == candidate_raw[..USER_DATA_OFFSET],
                "component {owner} changed raw-sector framing at LBA {lba}"
            );

            let existed = self.sectors.contains_key(&lba);
            let entry = self.sectors.entry(lba).or_insert_with(|| {
                let merged_user_data = source_raw
                    [USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE]
                    .try_into()
                    .expect("Form1 user data has fixed length");
                AccumulatedSectorPatch {
                    source_raw,
                    merged_user_data,
                    owner_by_offset: vec![None; USER_DATA_SIZE].into_boxed_slice(),
                }
            });
            ensure!(
                entry.source_raw == source_raw,
                "source sector changed while composing component {owner} at LBA {lba}"
            );
            if existed {
                report.sectors_already_touched += 1;
            }
            let source_user_data = &source_raw[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE];
            let component_user_data =
                &candidate_raw[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE];
            let sector_report = merge_component_user_data(
                source_user_data,
                &mut entry.merged_user_data,
                &mut entry.owner_by_offset,
                component_user_data,
                owner_index,
                &self.owners,
                lba,
            )?;
            ensure!(
                sector_report.changed_user_data_bytes > 0,
                "component {owner} declared unchanged LBA {lba}"
            );
            report.changed_user_data_bytes += sector_report.changed_user_data_bytes;
            report.added_user_data_bytes += sector_report.added_user_data_bytes;
            report.repeated_identical_user_data_bytes +=
                sector_report.repeated_identical_user_data_bytes;
        }
        Ok(report)
    }

    pub(crate) fn merged_sector_count(&self) -> usize {
        self.sectors.len()
    }

    pub(crate) fn merged_user_data_bytes(&self) -> usize {
        self.sectors
            .values()
            .map(|sector| {
                sector.source_raw[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE]
                    .iter()
                    .zip(&sector.merged_user_data)
                    .filter(|(source, merged)| source != merged)
                    .count()
            })
            .sum()
    }

    pub(crate) fn into_sector_patches(self) -> Result<Vec<SectorPatch>> {
        let mut patches = Vec::with_capacity(self.sectors.len());
        for (lba, sector) in self.sectors {
            let mut candidate = sector.source_raw;
            candidate[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE]
                .copy_from_slice(&sector.merged_user_data);
            regenerate_form1_integrity(&mut candidate)?;

            let source_user_data =
                &sector.source_raw[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE];
            let mut data_writes = Vec::new();
            let mut cursor = 0_usize;
            while cursor < USER_DATA_SIZE {
                if source_user_data[cursor] == sector.merged_user_data[cursor] {
                    cursor += 1;
                    continue;
                }
                let owner_index =
                    sector.owner_by_offset[cursor].context("merged byte has no candidate owner")?;
                let start = cursor;
                while cursor < USER_DATA_SIZE
                    && source_user_data[cursor] != sector.merged_user_data[cursor]
                    && sector.owner_by_offset[cursor] == Some(owner_index)
                {
                    cursor += 1;
                }
                let owner = &self.owners[owner_index];
                data_writes.push(ExpectedWrite {
                    id: format!("complete-{owner}-lba-{lba}-offset-{start:#x}"),
                    owner: format!("complete-translation-{owner}"),
                    purpose: "compose one independently verified component change against the immutable source"
                        .to_string(),
                    offset: USER_DATA_OFFSET + start,
                    expected_original: source_user_data[start..cursor].to_vec(),
                    replacement: sector.merged_user_data[start..cursor].to_vec(),
                    intent: WriteIntent::Data,
                });
            }
            ensure!(
                !data_writes.is_empty(),
                "composed LBA {lba} has no data writes"
            );

            let mut plan = WritePlan::new();
            for write in &data_writes {
                plan = plan.region(ImageRegion {
                    id: format!("{}-region", write.id),
                    range: write.offset..write.offset + write.replacement.len(),
                    kind: RegionKind::Data,
                    reason: write.purpose.clone(),
                });
            }
            plan = plan.region(ImageRegion {
                id: format!("complete-lba-{lba}-integrity"),
                range: FORM1_INTEGRITY_OFFSET..FORM1_INTEGRITY_OFFSET + FORM1_INTEGRITY_SIZE,
                kind: RegionKind::Metadata,
                reason: "Mode2/Form1 EDC and P/Q ECC for composed user data".to_string(),
            });
            for write in data_writes.iter().cloned() {
                plan = plan.write(write);
            }
            plan = plan.write(ExpectedWrite {
                id: format!("complete-lba-{lba}-regenerate-integrity"),
                owner: "complete-translation-form1-integrity".to_string(),
                purpose: "regenerate EDC and P/Q ECC after all component writes".to_string(),
                offset: FORM1_INTEGRITY_OFFSET,
                expected_original: sector.source_raw
                    [FORM1_INTEGRITY_OFFSET..FORM1_INTEGRITY_OFFSET + FORM1_INTEGRITY_SIZE]
                    .to_vec(),
                replacement: candidate
                    [FORM1_INTEGRITY_OFFSET..FORM1_INTEGRITY_OFFSET + FORM1_INTEGRITY_SIZE]
                    .to_vec(),
                intent: WriteIntent::Metadata,
            });
            let final_sector: [u8; RAW_SECTOR_SIZE] = plan
                .apply(&sector.source_raw, None)?
                .try_into()
                .expect("sector plan preserves the fixed length");
            validate_form1_integrity(&final_sector)?;
            patches.push(SectorPatch {
                lba,
                source_bytes: sector.source_raw,
                bytes: final_sector,
                data_writes: data_writes.len(),
            });
        }
        Ok(patches)
    }
}

fn merge_component_user_data(
    source: &[u8],
    merged: &mut [u8],
    owner_by_offset: &mut [Option<usize>],
    component: &[u8],
    component_owner: usize,
    owners: &[String],
    lba: u32,
) -> Result<SectorPatchMergeReport> {
    ensure!(
        source.len() == merged.len()
            && source.len() == component.len()
            && source.len() == owner_by_offset.len(),
        "component user-data buffers have different lengths"
    );
    let mut report = SectorPatchMergeReport {
        declared_sectors: 1,
        sectors_already_touched: 0,
        changed_user_data_bytes: 0,
        added_user_data_bytes: 0,
        repeated_identical_user_data_bytes: 0,
    };
    for offset in 0..source.len() {
        if component[offset] == source[offset] {
            continue;
        }
        report.changed_user_data_bytes += 1;
        match owner_by_offset[offset] {
            None => {
                merged[offset] = component[offset];
                owner_by_offset[offset] = Some(component_owner);
                report.added_user_data_bytes += 1;
            }
            Some(_) if merged[offset] == component[offset] => {
                report.repeated_identical_user_data_bytes += 1;
            }
            Some(previous_owner) => {
                bail!(
                    "component {} conflicts with {} at LBA {lba} user-data offset {offset:#x}: source {:#04x}, previous {:#04x}, component {:#04x}",
                    owners[component_owner],
                    owners[previous_owner],
                    source[offset],
                    merged[offset],
                    component[offset]
                );
            }
        }
    }
    Ok(report)
}

pub(crate) fn plan_sector_patches(
    source_path: &Path,
    files: &[IsoFileCandidate],
) -> Result<Vec<SectorPatch>> {
    let mut raw_by_lba: BTreeMap<u32, ([u8; RAW_SECTOR_SIZE], Vec<ExpectedWrite>)> =
        BTreeMap::new();
    let mut source = BufReader::new(
        File::open(source_path)
            .with_context(|| format!("failed to open source BIN {}", source_path.display()))?,
    );
    for file in files {
        ensure!(
            file.original.len() == file.replacement.len(),
            "ISO file length changed: {}",
            file.iso_path
        );
        for range in changed_ranges(&file.original, &file.replacement) {
            let mut file_offset = range.start;
            while file_offset < range.end {
                let sector_index = file_offset / USER_DATA_SIZE;
                let within_sector = file_offset % USER_DATA_SIZE;
                let take = (range.end - file_offset).min(USER_DATA_SIZE - within_sector);
                let lba = file
                    .extent_lba
                    .checked_add(u32::try_from(sector_index)?)
                    .context("ISO extent LBA overflow")?;
                let entry = if let Some(entry) = raw_by_lba.get_mut(&lba) {
                    entry
                } else {
                    let mut raw = [0; RAW_SECTOR_SIZE];
                    source.seek(SeekFrom::Start(u64::from(lba) * RAW_SECTOR_SIZE as u64))?;
                    source.read_exact(&mut raw)?;
                    validate_form1_integrity(&raw)
                        .with_context(|| format!("source integrity failed at touched LBA {lba}"))?;
                    raw_by_lba.entry(lba).or_insert((raw, Vec::new()))
                };
                let raw_offset = USER_DATA_OFFSET + within_sector;
                let replacement = file.replacement[file_offset..file_offset + take].to_vec();
                let expected_original = file.original[file_offset..file_offset + take].to_vec();
                ensure!(
                    entry.0[raw_offset..raw_offset + take] == expected_original,
                    "ISO extent bytes differ from raw sector at {} LBA {lba}",
                    file.iso_path
                );
                entry.1.push(ExpectedWrite {
                    id: format!(
                        "{}-{file_offset:#x}",
                        file.iso_path.trim_matches('/').replace(['/', ';'], "-")
                    ),
                    owner: format!("{}-file-candidate", file.iso_path),
                    purpose: "map one planned ISO file diff into its raw Form1 user-data range"
                        .to_string(),
                    offset: raw_offset,
                    expected_original,
                    replacement,
                    intent: WriteIntent::Data,
                });
                file_offset += take;
            }
        }
    }

    let mut patches = Vec::with_capacity(raw_by_lba.len());
    for (lba, (source_raw, data_writes)) in raw_by_lba {
        let mut candidate = source_raw;
        for write in &data_writes {
            candidate[write.offset..write.offset + write.replacement.len()]
                .copy_from_slice(&write.replacement);
        }
        regenerate_form1_integrity(&mut candidate)?;
        let mut plan = WritePlan::new();
        for write in &data_writes {
            plan = plan.region(ImageRegion {
                id: format!("{}-region", write.id),
                range: write.offset..write.offset + write.replacement.len(),
                kind: RegionKind::Data,
                reason: write.purpose.clone(),
            });
        }
        plan = plan.region(ImageRegion {
            id: format!("lba-{lba}-integrity"),
            range: FORM1_INTEGRITY_OFFSET..FORM1_INTEGRITY_OFFSET + FORM1_INTEGRITY_SIZE,
            kind: RegionKind::Metadata,
            reason: "Mode2/Form1 EDC and P/Q ECC for the changed user data".to_string(),
        });
        for write in data_writes.iter().cloned() {
            plan = plan.write(write);
        }
        plan = plan.write(ExpectedWrite {
            id: format!("lba-{lba}-regenerate-integrity"),
            owner: "mode2-form1-integrity".to_string(),
            purpose: "regenerate EDC and P/Q ECC after planned user-data writes".to_string(),
            offset: FORM1_INTEGRITY_OFFSET,
            expected_original: source_raw
                [FORM1_INTEGRITY_OFFSET..FORM1_INTEGRITY_OFFSET + FORM1_INTEGRITY_SIZE]
                .to_vec(),
            replacement: candidate
                [FORM1_INTEGRITY_OFFSET..FORM1_INTEGRITY_OFFSET + FORM1_INTEGRITY_SIZE]
                .to_vec(),
            intent: WriteIntent::Metadata,
        });
        let final_sector: [u8; RAW_SECTOR_SIZE] = plan
            .apply(&source_raw, None)?
            .try_into()
            .expect("sector plan preserves the fixed length");
        validate_form1_integrity(&final_sector)?;
        patches.push(SectorPatch {
            lba,
            source_bytes: source_raw,
            bytes: final_sector,
            data_writes: data_writes.len(),
        });
    }
    Ok(patches)
}

pub(crate) fn write_patched_bin(
    source_path: &Path,
    output_path: &Path,
    patches: &[SectorPatch],
) -> Result<String> {
    ensure!(!patches.is_empty(), "build produced no raw sector changes");
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    #[cfg(target_os = "macos")]
    if let Some(output_hash) =
        write_patched_bin_from_clone(source_path, output_path, patches, parent)?
    {
        return Ok(output_hash);
    }
    let mut source = BufReader::new(File::open(source_path)?);
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut writer = BufWriter::new(temporary.as_file_mut());
    let patch_map: BTreeMap<u32, &[u8; RAW_SECTOR_SIZE]> = patches
        .iter()
        .map(|patch| (patch.lba, &patch.bytes))
        .collect();
    let mut hasher = Sha256::new();
    let mut lba = 0_u32;
    let mut raw = [0; RAW_SECTOR_SIZE];
    loop {
        match source.read_exact(&mut raw) {
            Ok(()) => {
                let bytes = patch_map.get(&lba).copied().unwrap_or(&raw);
                writer.write_all(bytes)?;
                hasher.update(bytes);
                lba = lba.checked_add(1).context("output LBA overflow")?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }
    }
    writer.flush()?;
    drop(writer);
    temporary.as_file_mut().sync_all()?;
    let output_hash = format!("{:x}", hasher.finalize());
    if output_path.exists() {
        ensure!(
            hash_file(output_path)? == output_hash,
            "output BIN exists with different content: {}",
            output_path.display()
        );
        return Ok(output_hash);
    }
    temporary
        .persist_noclobber(output_path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to commit output BIN {}", output_path.display()))?;
    Ok(output_hash)
}

pub(crate) fn hash_sector_patches(patches: &[SectorPatch]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"puyo2-ps2-sector-patches-v1\0");
    for patch in patches {
        hasher.update(patch.lba.to_le_bytes());
        hasher.update(patch.source_bytes);
        hasher.update(patch.bytes);
        hasher.update(
            u64::try_from(patch.data_writes)
                .expect("sector data-write count fits u64")
                .to_le_bytes(),
        );
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(target_os = "macos")]
fn write_patched_bin_from_clone(
    source_path: &Path,
    output_path: &Path,
    patches: &[SectorPatch],
    parent: &Path,
) -> Result<Option<String>> {
    let temporary_path = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
    fs::remove_file(&temporary_path)?;
    let cloned = Command::new("cp")
        .arg("-c")
        .arg(source_path)
        .arg(&temporary_path)
        .status();
    let Ok(status) = cloned else {
        return Ok(None);
    };
    if !status.success() {
        if temporary_path.exists() {
            fs::remove_file(&temporary_path)?;
        }
        return Ok(None);
    }

    let mut output = OpenOptions::new().write(true).open(&temporary_path)?;
    for patch in patches {
        output.seek(SeekFrom::Start(
            u64::from(patch.lba) * RAW_SECTOR_SIZE as u64,
        ))?;
        output.write_all(&patch.bytes)?;
    }
    output.sync_all()?;
    drop(output);
    let output_hash = hash_file(&temporary_path)?;
    if output_path.exists() {
        ensure!(
            hash_file(output_path)? == output_hash,
            "output BIN exists with different content: {}",
            output_path.display()
        );
        return Ok(Some(output_hash));
    }
    temporary_path
        .persist_noclobber(output_path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to commit output BIN {}", output_path.display()))?;
    Ok(Some(output_hash))
}

pub(crate) fn write_mode2_cue(path: &Path, bin_path: &Path) -> Result<()> {
    let filename = bin_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("output BIN filename is not UTF-8")?;
    let cue = format!("FILE \"{filename}\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n");
    if path.exists() {
        ensure!(
            fs::read_to_string(path)? == cue,
            "output CUE exists with different content: {}",
            path.display()
        );
        return Ok(());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(cue.as_bytes())?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to commit output CUE {}", path.display()))?;
    Ok(())
}

pub(crate) fn verify_iso_readback(output_path: &Path, files: &[IsoFileCandidate]) -> Result<()> {
    let mut disc = RawDisc::open(BufReader::new(File::open(output_path)?))?;
    for file in files {
        ensure!(
            disc.read_extent(file.extent_lba, u32::try_from(file.replacement.len())?)?
                == file.replacement,
            "output ISO readback differs for {}",
            file.iso_path
        );
    }
    Ok(())
}

pub(crate) fn verify_sector_patch_readback(
    output_path: &Path,
    patches: &[SectorPatch],
) -> Result<()> {
    let mut output = BufReader::new(File::open(output_path)?);
    for patch in patches {
        output.seek(SeekFrom::Start(
            u64::from(patch.lba) * RAW_SECTOR_SIZE as u64,
        ))?;
        let mut raw = [0_u8; RAW_SECTOR_SIZE];
        output.read_exact(&mut raw)?;
        ensure!(
            raw == patch.bytes,
            "output readback differs at patched LBA {}",
            patch.lba
        );
        validate_form1_integrity(&raw)
            .with_context(|| format!("output integrity failed at patched LBA {}", patch.lba))?;
    }
    Ok(())
}

fn changed_ranges(source: &[u8], replacement: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = 0_usize;
    while cursor < source.len() {
        if source[cursor] == replacement[cursor] {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < source.len() && source[cursor] != replacement[cursor] {
            cursor += 1;
        }
        ranges.push(start..cursor);
    }
    ranges
}

fn hash_file(path: &Path) -> Result<String> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = [0; 1024 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
#[path = "disc_patch_tests.rs"]
mod disc_patch_tests;
