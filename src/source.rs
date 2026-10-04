use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::cue::CueSheet;
use crate::disc::{RAW_SECTOR_SIZE, RawDisc};
use crate::iso9660::{EntryKind, InventoryEntry, Iso9660, VolumeInfo};
use crate::system_cnf::{BootConfiguration, parse_system_cnf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceManifest {
    pub schema: u32,
    pub game: GameIdentity,
    pub cue: FileIdentity,
    pub bin: FileIdentity,
    pub track: TrackIdentity,
    pub filesystem: FilesystemIdentity,
    pub boot: BootIdentity,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GameIdentity {
    pub title: String,
    pub platform: String,
    pub region: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub filename: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrackIdentity {
    pub number: u8,
    pub mode: String,
    pub index: u8,
    pub start: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilesystemIdentity {
    pub format: String,
    pub raw_sector_size: u16,
    pub logical_block_size: u16,
    pub volume_space_sectors: u32,
    pub system_identifier: String,
    pub volume_identifier: String,
    pub root_directory_lba: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootIdentity {
    pub system_cnf_path: String,
    pub boot2: String,
    pub version: String,
    pub video_mode: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceVerification {
    pub schema: u32,
    pub passed: bool,
    pub game: GameIdentity,
    pub cue: VerifiedFile,
    pub bin: VerifiedFile,
    pub track: CueSheet,
    pub filesystem: VolumeInfo,
    pub boot: BootConfiguration,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerifiedFile {
    pub expected_filename: String,
    pub actual_filename: String,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct InventorySummary {
    pub files: usize,
    pub directories: usize,
    pub file_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscInventory {
    pub schema: u32,
    pub source: SourceVerification,
    pub summary: InventorySummary,
    pub entries: Vec<InventoryEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExtractedFile {
    pub iso_path: String,
    pub output_path: PathBuf,
    pub byte_length: u32,
    pub sha256: String,
    pub reused_existing: bool,
}

pub fn inspect_source(
    manifest_path: &Path,
    cue_path: &Path,
    bin_path: &Path,
) -> Result<DiscInventory> {
    let manifest = load_manifest(manifest_path)?;
    validate_manifest(&manifest)?;
    let cue_hash = hash_file(cue_path)?;
    let bin_hash = hash_file(bin_path)?;
    verify_file("CUE", &manifest.cue, &cue_hash)?;
    verify_file("BIN", &manifest.bin, &bin_hash)?;

    let cue_text = std::fs::read_to_string(cue_path)
        .with_context(|| format!("failed to read CUE {}", cue_path.display()))?;
    let cue = CueSheet::parse(&cue_text)?;
    ensure!(
        cue.track_number == manifest.track.number,
        "CUE track number mismatch"
    );
    ensure!(
        cue.track_mode == manifest.track.mode,
        "CUE track mode mismatch"
    );
    ensure!(
        cue.index_number == manifest.track.index,
        "CUE index number mismatch"
    );
    ensure!(
        cue.index_start == manifest.track.start,
        "CUE index start mismatch"
    );
    let referenced_bin = cue
        .referenced_path(cue_path)
        .canonicalize()
        .with_context(|| format!("failed to resolve BIN referenced by CUE: {}", cue.data_file))?;
    let supplied_bin = bin_path
        .canonicalize()
        .with_context(|| format!("failed to resolve supplied BIN {}", bin_path.display()))?;
    ensure!(
        referenced_bin == supplied_bin,
        "CUE references {}, not supplied BIN {}",
        referenced_bin.display(),
        supplied_bin.display()
    );

    let file = File::open(bin_path)
        .with_context(|| format!("failed to open BIN {}", bin_path.display()))?;
    let mut disc = RawDisc::open(BufReader::new(file))?;
    ensure!(
        RAW_SECTOR_SIZE == manifest.filesystem.raw_sector_size as usize,
        "configured raw sector size is unsupported"
    );
    ensure!(
        disc.sector_count() == manifest.filesystem.volume_space_sectors,
        "raw BIN sector count mismatch: expected {}, got {}",
        manifest.filesystem.volume_space_sectors,
        disc.sector_count()
    );
    let iso = Iso9660::open(&mut disc)?;
    verify_volume(&manifest.filesystem, iso.volume())?;
    let entries = iso.inventory(&mut disc)?;
    let system_cnf = iso.read_file(&mut disc, &entries, &manifest.boot.system_cnf_path)?;
    let boot = parse_system_cnf(&system_cnf)?;
    verify_boot(&manifest.boot, &boot)?;

    let files = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::File)
        .count();
    let directories = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Directory)
        .count();
    let file_bytes = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::File)
        .map(|entry| u64::from(entry.byte_length))
        .sum();
    Ok(DiscInventory {
        schema: 1,
        source: SourceVerification {
            schema: 1,
            passed: true,
            game: manifest.game,
            cue: verified_file(&manifest.cue, cue_path, cue_hash)?,
            bin: verified_file(&manifest.bin, bin_path, bin_hash)?,
            track: cue,
            filesystem: iso.volume().clone(),
            boot,
        },
        summary: InventorySummary {
            files,
            directories,
            file_bytes,
        },
        entries,
    })
}

pub fn write_inventory(path: &Path, inventory: &DiscInventory) -> Result<()> {
    write_json_file(path, inventory)
}

pub fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary output in {}", parent.display()))?;
    serde_json::to_writer_pretty(&mut temporary, value)?;
    temporary.write_all(b"\n")?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to commit JSON output {}", path.display()))?;
    Ok(())
}

pub fn write_new_json_file<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary output in {}", parent.display()))?;
    serde_json::to_writer_pretty(&mut temporary, value)?;
    temporary.write_all(b"\n")?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)
        .with_context(|| format!("refusing to overwrite JSON output {}", path.display()))?;
    Ok(())
}

pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read JSON input {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse JSON input {}", path.display()))
}

pub fn read_inventory_file(
    bin_path: &Path,
    inventory: &DiscInventory,
    iso_path: &str,
) -> Result<Vec<u8>> {
    let file = File::open(bin_path)
        .with_context(|| format!("failed to open BIN {}", bin_path.display()))?;
    let mut disc = RawDisc::open(BufReader::new(file))?;
    read_verified_entry(&mut disc, inventory, iso_path)
}

pub fn extract_files(
    bin_path: &Path,
    inventory: &DiscInventory,
    requested_paths: &[String],
    output_directory: &Path,
) -> Result<Vec<ExtractedFile>> {
    ensure!(
        !requested_paths.is_empty(),
        "at least one ISO path is required"
    );
    let mut unique_paths = std::collections::BTreeSet::new();
    for path in requested_paths {
        ensure!(
            unique_paths.insert(path),
            "duplicate ISO path requested: {path}"
        );
    }

    let file = File::open(bin_path)
        .with_context(|| format!("failed to open BIN {}", bin_path.display()))?;
    let mut disc = RawDisc::open(BufReader::new(file))?;
    let mut extracted = Vec::with_capacity(requested_paths.len());
    for iso_path in requested_paths {
        let bytes = read_verified_entry(&mut disc, inventory, iso_path)?;
        let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
        let entry = find_inventory_file(inventory, iso_path)?;
        let output_path = safe_output_path(output_directory, iso_path)?;
        let reused_existing = write_new_or_verify(&output_path, &bytes, &actual_sha256)?;
        extracted.push(ExtractedFile {
            iso_path: iso_path.clone(),
            output_path,
            byte_length: entry.byte_length,
            sha256: actual_sha256,
            reused_existing,
        });
    }
    Ok(extracted)
}

fn read_verified_entry<R: std::io::Read + std::io::Seek>(
    disc: &mut RawDisc<R>,
    inventory: &DiscInventory,
    iso_path: &str,
) -> Result<Vec<u8>> {
    let entry = find_inventory_file(inventory, iso_path)?;
    let expected_sha256 = entry
        .sha256
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("inventory file has no SHA-256: {iso_path}"))?;
    let bytes = disc.read_extent(entry.extent_lba, entry.byte_length)?;
    let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
    ensure!(
        actual_sha256 == expected_sha256,
        "read bytes do not match inventory for {iso_path}"
    );
    Ok(bytes)
}

pub fn find_inventory_file<'a>(
    inventory: &'a DiscInventory,
    iso_path: &str,
) -> Result<&'a InventoryEntry> {
    inventory
        .entries
        .iter()
        .find(|entry| entry.kind == EntryKind::File && entry.path == iso_path)
        .ok_or_else(|| anyhow::anyhow!("ISO9660 file not found: {iso_path}"))
}

fn safe_output_path(output_directory: &Path, iso_path: &str) -> Result<PathBuf> {
    ensure!(
        iso_path.starts_with('/'),
        "ISO path must be absolute: {iso_path}"
    );
    let relative = Path::new(iso_path.trim_start_matches('/'));
    ensure!(
        !relative.as_os_str().is_empty(),
        "cannot extract the ISO root"
    );
    for component in relative.components() {
        ensure!(
            matches!(component, Component::Normal(_)),
            "unsafe ISO path component in {iso_path}"
        );
    }
    Ok(output_directory.join(relative))
}

fn write_new_or_verify(path: &Path, bytes: &[u8], expected_sha256: &str) -> Result<bool> {
    if path.exists() {
        ensure!(
            path.is_file(),
            "extraction target is not a file: {}",
            path.display()
        );
        let existing = hash_file(path)?;
        ensure!(
            existing.byte_length == bytes.len() as u64 && existing.sha256 == expected_sha256,
            "refusing to overwrite different existing extraction {}",
            path.display()
        );
        return Ok(true);
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create extraction directory {}", parent.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "failed to create temporary extraction in {}",
            parent.display()
        )
    })?;
    temporary.write_all(bytes)?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to commit extraction {}", path.display()))?;
    Ok(false)
}

fn load_manifest(path: &Path) -> Result<SourceManifest> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read source manifest {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse source manifest {}", path.display()))
}

fn validate_manifest(manifest: &SourceManifest) -> Result<()> {
    ensure!(
        manifest.schema == 1,
        "unsupported source manifest schema {}",
        manifest.schema
    );
    validate_identity("CUE", &manifest.cue)?;
    validate_identity("BIN", &manifest.bin)?;
    ensure!(manifest.track.number == 1, "only track 1 is supported");
    ensure!(
        manifest.track.mode == "MODE2/2352",
        "only MODE2/2352 is supported"
    );
    ensure!(manifest.track.index == 1, "only index 1 is supported");
    ensure!(
        manifest.track.start == "00:00:00",
        "only index start 00:00:00 is supported"
    );
    ensure!(
        manifest.filesystem.format == "ISO9660",
        "only ISO9660 is supported"
    );
    Ok(())
}

fn validate_identity(label: &str, identity: &FileIdentity) -> Result<()> {
    ensure!(!identity.filename.is_empty(), "{label} filename is empty");
    ensure!(
        identity.sha256.len() == 64 && identity.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{label} SHA-256 must be 64 hexadecimal characters"
    );
    Ok(())
}

#[derive(Clone, Debug)]
struct FileHash {
    byte_length: u64,
    sha256: String,
}

fn hash_file(path: &Path) -> Result<FileHash> {
    let file =
        File::open(path).with_context(|| format!("failed to open source {}", path.display()))?;
    let expected_length = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    let mut byte_length = 0_u64;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        byte_length += count as u64;
    }
    ensure!(
        byte_length == expected_length,
        "source size changed while hashing {}",
        path.display()
    );
    Ok(FileHash {
        byte_length,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

fn verify_file(label: &str, expected: &FileIdentity, actual: &FileHash) -> Result<()> {
    ensure!(
        actual.byte_length == expected.size,
        "{label} size mismatch: expected {}, got {}",
        expected.size,
        actual.byte_length
    );
    ensure!(
        actual.sha256 == expected.sha256.to_ascii_lowercase(),
        "{label} SHA-256 mismatch: expected {}, got {}",
        expected.sha256,
        actual.sha256
    );
    Ok(())
}

fn verified_file(expected: &FileIdentity, path: &Path, actual: FileHash) -> Result<VerifiedFile> {
    Ok(VerifiedFile {
        expected_filename: expected.filename.clone(),
        actual_filename: path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow::anyhow!("source filename is not UTF-8: {}", path.display()))?
            .to_string(),
        byte_length: actual.byte_length,
        sha256: actual.sha256,
    })
}

fn verify_volume(expected: &FilesystemIdentity, actual: &VolumeInfo) -> Result<()> {
    ensure!(
        actual.format == expected.format,
        "filesystem format mismatch"
    );
    ensure!(
        actual.logical_block_size == expected.logical_block_size,
        "ISO9660 logical block size mismatch"
    );
    ensure!(
        actual.volume_space_sectors == expected.volume_space_sectors,
        "ISO9660 volume space size mismatch"
    );
    ensure!(
        actual.system_identifier == expected.system_identifier,
        "ISO9660 system identifier mismatch"
    );
    ensure!(
        actual.volume_identifier == expected.volume_identifier,
        "ISO9660 volume identifier mismatch"
    );
    ensure!(
        actual.root_directory_lba == expected.root_directory_lba,
        "ISO9660 root directory LBA mismatch"
    );
    Ok(())
}

fn verify_boot(expected: &BootIdentity, actual: &BootConfiguration) -> Result<()> {
    ensure!(actual.boot2 == expected.boot2, "SYSTEM.CNF BOOT2 mismatch");
    ensure!(
        actual.version == expected.version,
        "SYSTEM.CNF VER mismatch"
    );
    ensure!(
        actual.video_mode == expected.video_mode,
        "SYSTEM.CNF VMODE mismatch"
    );
    Ok(())
}

#[cfg(test)]
#[path = "source/source_tests.rs"]
mod source_tests;
