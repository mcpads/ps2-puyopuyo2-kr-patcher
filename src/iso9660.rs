use std::collections::BTreeSet;
use std::io::{Read, Seek};

use anyhow::{Result, bail, ensure};
use serde::Serialize;

use crate::disc::{RawDisc, USER_DATA_SIZE};

const PVD_LBA: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VolumeInfo {
    pub format: String,
    pub system_identifier: String,
    pub volume_identifier: String,
    pub logical_block_size: u16,
    pub volume_space_sectors: u32,
    pub root_directory_lba: u32,
    pub root_directory_bytes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Directory,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InventoryEntry {
    pub path: String,
    pub kind: EntryKind,
    pub extent_lba: u32,
    pub byte_length: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Clone, Debug)]
struct DirectoryRecord {
    identifier: String,
    extent_lba: u32,
    byte_length: u32,
    is_directory: bool,
}

#[derive(Debug)]
pub struct Iso9660 {
    volume: VolumeInfo,
}

impl Iso9660 {
    pub fn open<R: Read + Seek>(disc: &mut RawDisc<R>) -> Result<Self> {
        let pvd = disc.read_form1_sector(PVD_LBA)?;
        ensure!(
            pvd[0] == 1 && &pvd[1..6] == b"CD001" && pvd[6] == 1,
            "invalid ISO9660 Primary Volume Descriptor"
        );
        let volume_space_sectors = both_endian_u32(&pvd, 80, "volume space size")?;
        ensure!(
            volume_space_sectors <= disc.sector_count(),
            "ISO9660 volume declares {volume_space_sectors} sectors but BIN has {}",
            disc.sector_count()
        );
        let logical_block_size = both_endian_u16(&pvd, 128, "logical block size")?;
        ensure!(
            logical_block_size as usize == USER_DATA_SIZE,
            "unsupported ISO9660 logical block size {logical_block_size}"
        );
        let root_length = pvd[156] as usize;
        ensure!(root_length >= 34, "invalid ISO9660 root directory record");
        let root = parse_directory_record(&pvd[156..156 + root_length])?;
        ensure!(root.is_directory, "ISO9660 root record is not a directory");
        validate_extent(
            root.extent_lba,
            root.byte_length,
            volume_space_sectors,
            "root directory",
        )?;

        Ok(Self {
            volume: VolumeInfo {
                format: "ISO9660".to_string(),
                system_identifier: padded_ascii(&pvd[8..40], "system identifier")?,
                volume_identifier: padded_ascii(&pvd[40..72], "volume identifier")?,
                logical_block_size,
                volume_space_sectors,
                root_directory_lba: root.extent_lba,
                root_directory_bytes: root.byte_length,
            },
        })
    }

    pub fn volume(&self) -> &VolumeInfo {
        &self.volume
    }

    pub fn inventory<R: Read + Seek>(&self, disc: &mut RawDisc<R>) -> Result<Vec<InventoryEntry>> {
        let mut entries = Vec::new();
        let mut visited = BTreeSet::new();
        self.walk_directory(
            disc,
            "",
            self.volume.root_directory_lba,
            self.volume.root_directory_bytes,
            &mut visited,
            &mut entries,
        )?;
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(entries)
    }

    pub fn read_file<R: Read + Seek>(
        &self,
        disc: &mut RawDisc<R>,
        entries: &[InventoryEntry],
        path: &str,
    ) -> Result<Vec<u8>> {
        let entry = entries
            .iter()
            .find(|entry| entry.kind == EntryKind::File && entry.path == path)
            .ok_or_else(|| anyhow::anyhow!("ISO9660 path not found: {path}"))?;
        disc.read_extent(entry.extent_lba, entry.byte_length)
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_directory<R: Read + Seek>(
        &self,
        disc: &mut RawDisc<R>,
        parent_path: &str,
        extent_lba: u32,
        byte_length: u32,
        visited: &mut BTreeSet<(u32, u32)>,
        output: &mut Vec<InventoryEntry>,
    ) -> Result<()> {
        ensure!(
            visited.insert((extent_lba, byte_length)),
            "ISO9660 directory extent is referenced more than once: LBA {extent_lba}, {byte_length} bytes"
        );
        validate_extent(
            extent_lba,
            byte_length,
            self.volume.volume_space_sectors,
            "directory",
        )?;
        let bytes = disc.read_extent(extent_lba, byte_length)?;
        for record in parse_directory(&bytes)? {
            if record.identifier == "\0" || record.identifier == "\u{1}" {
                continue;
            }
            validate_extent(
                record.extent_lba,
                record.byte_length,
                self.volume.volume_space_sectors,
                &record.identifier,
            )?;
            let path = format!("{parent_path}/{}", record.identifier);
            if record.is_directory {
                output.push(InventoryEntry {
                    path: path.clone(),
                    kind: EntryKind::Directory,
                    extent_lba: record.extent_lba,
                    byte_length: record.byte_length,
                    sha256: None,
                });
                self.walk_directory(
                    disc,
                    &path,
                    record.extent_lba,
                    record.byte_length,
                    visited,
                    output,
                )?;
            } else {
                let sha256 = disc.hash_extent(record.extent_lba, record.byte_length)?;
                output.push(InventoryEntry {
                    path,
                    kind: EntryKind::File,
                    extent_lba: record.extent_lba,
                    byte_length: record.byte_length,
                    sha256: Some(sha256),
                });
            }
        }
        Ok(())
    }
}

fn parse_directory(bytes: &[u8]) -> Result<Vec<DirectoryRecord>> {
    let mut records = Vec::new();
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let record_length = bytes[offset] as usize;
        if record_length == 0 {
            let next_sector = (offset / USER_DATA_SIZE + 1) * USER_DATA_SIZE;
            if next_sector <= offset {
                bail!("ISO9660 directory offset did not advance");
            }
            offset = next_sector.min(bytes.len());
            continue;
        }
        ensure!(
            offset + record_length <= bytes.len(),
            "ISO9660 directory record crosses extent boundary at byte {offset}"
        );
        records.push(parse_directory_record(
            &bytes[offset..offset + record_length],
        )?);
        offset += record_length;
    }
    Ok(records)
}

fn parse_directory_record(bytes: &[u8]) -> Result<DirectoryRecord> {
    ensure!(
        bytes.len() >= 34,
        "ISO9660 directory record is shorter than 34 bytes"
    );
    ensure!(
        bytes[0] as usize == bytes.len(),
        "ISO9660 directory record length mismatch"
    );
    ensure!(bytes[1] == 0, "extended attribute records are unsupported");
    let extent_lba = both_endian_u32(bytes, 2, "extent LBA")?;
    let byte_length = both_endian_u32(bytes, 10, "extent byte length")?;
    let flags = bytes[25];
    ensure!(
        flags & 0x80 == 0,
        "multi-extent ISO9660 records are unsupported"
    );
    let identifier_length = bytes[32] as usize;
    ensure!(
        33 + identifier_length <= bytes.len(),
        "ISO9660 identifier exceeds its directory record"
    );
    let identifier_bytes = &bytes[33..33 + identifier_length];
    ensure!(
        identifier_bytes.is_ascii(),
        "ISO9660 identifier is not ASCII"
    );
    let identifier = String::from_utf8(identifier_bytes.to_vec())?;
    Ok(DirectoryRecord {
        identifier,
        extent_lba,
        byte_length,
        is_directory: flags & 0x02 != 0,
    })
}

fn validate_extent(lba: u32, byte_length: u32, volume_sectors: u32, label: &str) -> Result<()> {
    let sectors = byte_length.div_ceil(USER_DATA_SIZE as u32);
    let end = lba
        .checked_add(sectors)
        .ok_or_else(|| anyhow::anyhow!("ISO9660 extent overflow for {label}"))?;
    ensure!(
        end <= volume_sectors,
        "ISO9660 extent for {label} ends at LBA {end}, past volume size {volume_sectors}"
    );
    Ok(())
}

fn both_endian_u16(bytes: &[u8], offset: usize, label: &str) -> Result<u16> {
    ensure!(offset + 4 <= bytes.len(), "missing ISO9660 {label}");
    let little = u16::from_le_bytes(bytes[offset..offset + 2].try_into()?);
    let big = u16::from_be_bytes(bytes[offset + 2..offset + 4].try_into()?);
    ensure!(little == big, "ISO9660 {label} endian copies disagree");
    Ok(little)
}

fn both_endian_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32> {
    ensure!(offset + 8 <= bytes.len(), "missing ISO9660 {label}");
    let little = u32::from_le_bytes(bytes[offset..offset + 4].try_into()?);
    let big = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into()?);
    ensure!(little == big, "ISO9660 {label} endian copies disagree");
    Ok(little)
}

fn padded_ascii(bytes: &[u8], label: &str) -> Result<String> {
    ensure!(bytes.is_ascii(), "ISO9660 {label} is not ASCII");
    Ok(String::from_utf8(bytes.to_vec())?
        .trim_end_matches([' ', '\0'])
        .to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use sha2::{Digest, Sha256};

    use crate::disc::{RAW_SECTOR_SIZE, form1_sector};

    use super::*;

    #[test]
    fn inventories_nested_files_and_hashes_exact_file_bytes() {
        let fixture = iso_fixture();
        let mut disc = RawDisc::open(Cursor::new(fixture)).unwrap();
        let iso = Iso9660::open(&mut disc).unwrap();
        assert_eq!(iso.volume().system_identifier, "PLAYSTATION");
        assert_eq!(iso.volume().volume_identifier, "PUYO2_TEST");

        let entries = iso.inventory(&mut disc).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(
            entries
                .iter()
                .any(|entry| { entry.path == "/SYSTEM.CNF;1" && entry.kind == EntryKind::File })
        );
        let note = entries
            .iter()
            .find(|entry| entry.path == "/DATA/NOTE.TXT;1")
            .unwrap();
        let expected = vec![0x5a_u8; USER_DATA_SIZE + 17];
        assert_eq!(note.byte_length, expected.len() as u32);
        assert_eq!(
            note.sha256.as_deref(),
            Some(format!("{:x}", Sha256::digest(&expected)).as_str())
        );
    }

    #[test]
    fn rejects_disagreeing_both_endian_fields() {
        let mut fixture = iso_fixture();
        let pvd_offset = PVD_LBA as usize * RAW_SECTOR_SIZE + crate::disc::USER_DATA_OFFSET;
        fixture[pvd_offset + 84] ^= 1;
        let mut disc = RawDisc::open(Cursor::new(fixture)).unwrap();
        let error = Iso9660::open(&mut disc).unwrap_err().to_string();
        assert!(error.contains("endian copies disagree"));
    }

    fn iso_fixture() -> Vec<u8> {
        const SECTOR_COUNT: usize = 24;
        const ROOT_LBA: u32 = 18;
        const SYSTEM_LBA: u32 = 19;
        const DATA_LBA: u32 = 20;
        const NOTE_LBA: u32 = 21;

        let system = b"BOOT2 = cdrom0:\\TEST_000.00;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n";
        let note = vec![0x5a_u8; USER_DATA_SIZE + 17];
        let mut user_sectors = vec![[0_u8; USER_DATA_SIZE]; SECTOR_COUNT];

        let pvd = &mut user_sectors[PVD_LBA as usize];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 1;
        write_padded_ascii(&mut pvd[8..40], "PLAYSTATION");
        write_padded_ascii(&mut pvd[40..72], "PUYO2_TEST");
        write_both_u32(pvd, 80, SECTOR_COUNT as u32);
        write_both_u16(pvd, 128, USER_DATA_SIZE as u16);
        let root_record = directory_record(ROOT_LBA, USER_DATA_SIZE as u32, 0x02, &[0]);
        pvd[156..156 + root_record.len()].copy_from_slice(&root_record);

        let root = &mut user_sectors[ROOT_LBA as usize];
        let records = [
            directory_record(ROOT_LBA, USER_DATA_SIZE as u32, 0x02, &[0]),
            directory_record(ROOT_LBA, USER_DATA_SIZE as u32, 0x02, &[1]),
            directory_record(SYSTEM_LBA, system.len() as u32, 0, b"SYSTEM.CNF;1"),
            directory_record(DATA_LBA, USER_DATA_SIZE as u32, 0x02, b"DATA"),
        ];
        write_records(root, &records);

        user_sectors[SYSTEM_LBA as usize][..system.len()].copy_from_slice(system);
        let data = &mut user_sectors[DATA_LBA as usize];
        let data_records = [
            directory_record(DATA_LBA, USER_DATA_SIZE as u32, 0x02, &[0]),
            directory_record(ROOT_LBA, USER_DATA_SIZE as u32, 0x02, &[1]),
            directory_record(NOTE_LBA, note.len() as u32, 0, b"NOTE.TXT;1"),
        ];
        write_records(data, &data_records);
        user_sectors[NOTE_LBA as usize].copy_from_slice(&note[..USER_DATA_SIZE]);
        user_sectors[NOTE_LBA as usize + 1][..17].copy_from_slice(&note[USER_DATA_SIZE..]);

        user_sectors
            .into_iter()
            .flat_map(|sector| form1_sector(&sector))
            .collect()
    }

    fn directory_record(lba: u32, size: u32, flags: u8, identifier: &[u8]) -> Vec<u8> {
        let length = 33 + identifier.len() + usize::from(identifier.len().is_multiple_of(2));
        let mut record = vec![0_u8; length];
        record[0] = length as u8;
        write_both_u32(&mut record, 2, lba);
        write_both_u32(&mut record, 10, size);
        record[25] = flags;
        record[28..30].copy_from_slice(&1_u16.to_le_bytes());
        record[30..32].copy_from_slice(&1_u16.to_be_bytes());
        record[32] = identifier.len() as u8;
        record[33..33 + identifier.len()].copy_from_slice(identifier);
        record
    }

    fn write_records(target: &mut [u8], records: &[Vec<u8>]) {
        let mut offset = 0;
        for record in records {
            target[offset..offset + record.len()].copy_from_slice(record);
            offset += record.len();
        }
    }

    fn write_both_u16(target: &mut [u8], offset: usize, value: u16) {
        target[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        target[offset + 2..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn write_both_u32(target: &mut [u8], offset: usize, value: u32) {
        target[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        target[offset + 4..offset + 8].copy_from_slice(&value.to_be_bytes());
    }

    fn write_padded_ascii(target: &mut [u8], value: &str) {
        target.fill(b' ');
        target[..value.len()].copy_from_slice(value.as_bytes());
    }
}
