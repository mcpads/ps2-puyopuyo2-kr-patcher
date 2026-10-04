use std::io::{Read, Seek, SeekFrom};
use std::sync::OnceLock;

use anyhow::{Result, bail, ensure};
use sha2::{Digest, Sha256};

pub const RAW_SECTOR_SIZE: usize = 2352;
pub const USER_DATA_OFFSET: usize = 24;
pub const USER_DATA_SIZE: usize = 2048;
const SYNC: [u8; 12] = [
    0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00,
];
const FORM1_EDC_OFFSET: usize = 0x818;
const ECC_P_OFFSET: usize = 0x81c;
const ECC_Q_OFFSET: usize = 0x8c8;
const ECC_END: usize = 0x930;

pub const FORM1_INTEGRITY_OFFSET: usize = FORM1_EDC_OFFSET;
pub const FORM1_INTEGRITY_SIZE: usize = ECC_END - FORM1_EDC_OFFSET;

#[derive(Debug)]
struct IntegrityTables {
    forward: [u8; 256],
    backward: [u8; 256],
    edc: [u32; 256],
}

fn integrity_tables() -> &'static IntegrityTables {
    static TABLES: OnceLock<IntegrityTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut forward = [0; 256];
        let mut backward = [0; 256];
        let mut edc = [0; 256];
        for value in 0..256_usize {
            let doubled =
                (((value as u32) << 1) ^ if value & 0x80 != 0 { 0x11d } else { 0 }) & 0xff;
            forward[value] = doubled as u8;
            backward[value ^ doubled as usize] = value as u8;
            let mut remainder = value as u32;
            for _ in 0..8 {
                remainder = if remainder & 1 != 0 {
                    (remainder >> 1) ^ 0xd801_8001
                } else {
                    remainder >> 1
                };
            }
            edc[value] = remainder;
        }
        IntegrityTables {
            forward,
            backward,
            edc,
        }
    })
}

fn validate_form1_framing(raw: &[u8]) -> Result<()> {
    ensure!(
        raw.len() == RAW_SECTOR_SIZE,
        "raw sector length is not {RAW_SECTOR_SIZE} bytes"
    );
    ensure!(raw[..SYNC.len()] == SYNC, "invalid raw-sector sync");
    ensure!(raw[0x0f] == 2, "raw sector is not Mode 2");
    ensure!(
        raw[0x10..0x14] == raw[0x14..0x18],
        "Mode 2 subheader copies disagree"
    );
    ensure!(raw[0x12] & 0x20 == 0, "raw sector is not Form 1");
    Ok(())
}

fn compute_edc(source: &[u8], table: &[u32; 256]) -> u32 {
    let mut value = 0_u32;
    for &byte in source {
        value = (value >> 8) ^ table[((value ^ u32::from(byte)) & 0xff) as usize];
    }
    value
}

fn compute_ecc(
    source: &[u8],
    major_count: usize,
    minor_count: usize,
    major_multiplier: usize,
    minor_increment: usize,
    tables: &IntegrityTables,
) -> Vec<u8> {
    let matrix_size = major_count * minor_count;
    let mut output = vec![0; major_count * 2];
    for major in 0..major_count {
        let mut index = (major >> 1) * major_multiplier + (major & 1);
        let mut parity_a = 0_u8;
        let mut parity_b = 0_u8;
        for _ in 0..minor_count {
            let value = source[index];
            index += minor_increment;
            if index >= matrix_size {
                index -= matrix_size;
            }
            parity_a ^= value;
            parity_b ^= value;
            parity_a = tables.forward[parity_a as usize];
        }
        parity_a = tables.backward[(tables.forward[parity_a as usize] ^ parity_b) as usize];
        output[major] = parity_a;
        output[major + major_count] = parity_a ^ parity_b;
    }
    output
}

fn expected_form1_integrity(raw: &[u8]) -> Result<Vec<u8>> {
    validate_form1_framing(raw)?;
    let tables = integrity_tables();
    let mut work: [u8; RAW_SECTOR_SIZE] = raw.try_into().expect("length was validated");
    let edc = compute_edc(&work[0x10..FORM1_EDC_OFFSET], &tables.edc).to_le_bytes();
    work[FORM1_EDC_OFFSET..ECC_P_OFFSET].copy_from_slice(&edc);
    work[0x0c..0x10].fill(0);
    work[ECC_P_OFFSET..ECC_END].fill(0);
    let p = compute_ecc(&work[0x0c..], 86, 24, 2, 86, tables);
    work[ECC_P_OFFSET..ECC_Q_OFFSET].copy_from_slice(&p);
    let q = compute_ecc(&work[0x0c..], 52, 43, 86, 88, tables);
    let mut integrity = Vec::with_capacity(FORM1_INTEGRITY_SIZE);
    integrity.extend_from_slice(&edc);
    integrity.extend_from_slice(&p);
    integrity.extend_from_slice(&q);
    Ok(integrity)
}

pub fn validate_form1_integrity(raw: &[u8]) -> Result<()> {
    let expected = expected_form1_integrity(raw)?;
    ensure!(
        raw[FORM1_INTEGRITY_OFFSET..ECC_END] == expected,
        "Mode 2/Form 1 EDC/ECC mismatch"
    );
    Ok(())
}

pub fn regenerate_form1_integrity(raw: &mut [u8]) -> Result<()> {
    let expected = expected_form1_integrity(raw)?;
    raw[FORM1_INTEGRITY_OFFSET..ECC_END].copy_from_slice(&expected);
    validate_form1_integrity(raw)
}

pub struct RawDisc<R> {
    reader: R,
    sector_count: u32,
}

impl<R: Read + Seek> RawDisc<R> {
    pub fn open(mut reader: R) -> Result<Self> {
        let byte_length = reader.seek(SeekFrom::End(0))?;
        ensure!(
            byte_length % RAW_SECTOR_SIZE as u64 == 0,
            "raw BIN length {byte_length} is not a multiple of {RAW_SECTOR_SIZE}"
        );
        let sector_count = byte_length / RAW_SECTOR_SIZE as u64;
        ensure!(
            sector_count <= u32::MAX as u64,
            "raw BIN has too many sectors: {sector_count}"
        );
        reader.seek(SeekFrom::Start(0))?;
        Ok(Self {
            reader,
            sector_count: sector_count as u32,
        })
    }

    pub fn sector_count(&self) -> u32 {
        self.sector_count
    }

    pub fn read_form1_sector(&mut self, lba: u32) -> Result<[u8; USER_DATA_SIZE]> {
        ensure!(
            lba < self.sector_count,
            "LBA {lba} is outside {}-sector image",
            self.sector_count
        );
        let offset = u64::from(lba) * RAW_SECTOR_SIZE as u64;
        self.reader.seek(SeekFrom::Start(offset))?;
        let mut raw = [0_u8; RAW_SECTOR_SIZE];
        self.reader.read_exact(&mut raw)?;
        ensure!(
            raw[..SYNC.len()] == SYNC,
            "invalid raw-sector sync at LBA {lba}"
        );
        ensure!(raw[0x0f] == 2, "sector at LBA {lba} is not Mode 2");
        ensure!(
            raw[0x10..0x14] == raw[0x14..0x18],
            "Mode 2 subheader copies disagree at LBA {lba}"
        );
        if raw[0x12] & 0x20 != 0 {
            bail!("sector at LBA {lba} is Mode 2/Form 2, not Form 1");
        }
        Ok(raw[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE]
            .try_into()
            .expect("Mode 2/Form 1 user data has fixed length"))
    }

    pub fn read_extent(&mut self, start_lba: u32, byte_length: u32) -> Result<Vec<u8>> {
        let mut output = Vec::with_capacity(byte_length as usize);
        let mut remaining = byte_length as usize;
        let mut lba = start_lba;
        while remaining > 0 {
            let sector = self.read_form1_sector(lba)?;
            let take = remaining.min(USER_DATA_SIZE);
            output.extend_from_slice(&sector[..take]);
            remaining -= take;
            lba = lba
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("extent LBA overflow"))?;
        }
        Ok(output)
    }

    pub fn hash_extent(&mut self, start_lba: u32, byte_length: u32) -> Result<String> {
        let mut hasher = Sha256::new();
        let mut remaining = byte_length as usize;
        let mut lba = start_lba;
        while remaining > 0 {
            let sector = self.read_form1_sector(lba)?;
            let take = remaining.min(USER_DATA_SIZE);
            hasher.update(&sector[..take]);
            remaining -= take;
            lba = lba
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("extent LBA overflow"))?;
        }
        Ok(format!("{:x}", hasher.finalize()))
    }
}

#[cfg(test)]
pub(crate) fn form1_sector(user_data: &[u8]) -> [u8; RAW_SECTOR_SIZE] {
    let mut raw = [0_u8; RAW_SECTOR_SIZE];
    raw[..SYNC.len()].copy_from_slice(&SYNC);
    raw[0x0f] = 2;
    raw[0x10..0x14].copy_from_slice(&[0, 0, 8, 0]);
    raw[0x14..0x18].copy_from_slice(&[0, 0, 8, 0]);
    raw[USER_DATA_OFFSET..USER_DATA_OFFSET + user_data.len()].copy_from_slice(user_data);
    raw
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn reads_only_mode2_form1_user_data() {
        let mut user_data = [0_u8; USER_DATA_SIZE];
        user_data[..4].copy_from_slice(b"TEST");
        let raw = form1_sector(&user_data);
        let mut disc = RawDisc::open(Cursor::new(raw)).unwrap();
        assert_eq!(&disc.read_form1_sector(0).unwrap()[..4], b"TEST");
    }

    #[test]
    fn rejects_form2_and_partial_images() {
        let mut form2 = form1_sector(&[]);
        form2[0x12] |= 0x20;
        form2[0x16] |= 0x20;
        let mut disc = RawDisc::open(Cursor::new(form2)).unwrap();
        assert!(
            disc.read_form1_sector(0)
                .unwrap_err()
                .to_string()
                .contains("Form 2")
        );

        let partial = vec![0_u8; RAW_SECTOR_SIZE - 1];
        assert!(RawDisc::open(Cursor::new(partial)).is_err());
    }

    #[test]
    fn regenerates_and_detects_form1_integrity() {
        let mut raw = form1_sector(b"protected payload");
        regenerate_form1_integrity(&mut raw).unwrap();
        validate_form1_integrity(&raw).unwrap();
        raw[USER_DATA_OFFSET + 3] ^= 0x80;
        assert!(validate_form1_integrity(&raw).is_err());
    }
}
