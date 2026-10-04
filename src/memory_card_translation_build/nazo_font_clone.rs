use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::font::MemoryCardFontResources;
use crate::cpx::read_symbol_bytes;
use crate::executable::read_executable_symbol;
use crate::executable_references::find_direct_calls;
use crate::font_atlas::nazo::{NazoFontAtlasReport, NazoTextureRegistrationPair};
use crate::indexed_texture::{
    parse_file_records, parse_load_records, read_decoded_image, read_palette,
    unswizzle_psmt8_uploaded_as_psmct32,
};

pub(super) const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_G.BIN;1";
pub(super) const FILE_LIST_SYMBOL: &str = "FileList_Image2";
const LOAD_TABLE_SYMBOL: &str = "NazoTexTbl";
const ERROR_CALLER_SYMBOL: &str = "Nazo_Menu_Run";
const ERROR_RENDERER_SYMBOL: &str = "MyNazo_ErrMsg";
const REGISTRATION_BUFFER_SYMBOL: &str = "RegBuff";
const REGISTRAR_SYMBOL: &str = "ActRegList_Regist3";
const REGISTRATION_RECORD_BYTES: u64 = 0x90;
const REGISTRATION_SLOT: usize = 1;
const IMAGE_LOAD_INDEX: usize = 9;
const PALETTE_SLOT: usize = 9;
const IMAGE_FILE_RECORD: usize = 122;
const PALETTE_FILE_RECORD: usize = 242;
const EXPECTED_REGISTRATION_SETS: [&str; 8] = [
    "Nazo_TexReg_Menu",
    "Nazo_TexReg_Nazo",
    "Nazo_TexReg_Game",
    "Nazo_TexReg_Edit_0",
    "Nazo_TexReg_Edit_1",
    "Nazo_TexReg_Edit_2",
    "Nazo_TexReg_Edit_4",
    "Nazo_TexReg_Present",
];

pub(super) fn load_nazo_menu_font_clone(
    executable: &[u8],
    container: &[u8],
    nazo_font: &NazoFontAtlasReport,
) -> Result<MemoryCardFontResources> {
    ensure!(
        nazo_font.source.load_table_symbol == LOAD_TABLE_SYMBOL
            && nazo_font.source.file_list_symbol == FILE_LIST_SYMBOL
            && nazo_font.source.mapped_iso_path == IMAGE_CONTAINER_ISO_PATH,
        "Nazo font evidence no longer owns the IMAGE_G load path"
    );
    validate_nazo_registration_path(nazo_font)?;

    let file = object::File::parse(executable)
        .context("failed to parse executable for Nazo menu font clone")?;
    validate_my_nazo_error_call(&file, executable)?;
    let (_, load_bytes) = read_symbol_bytes(&file, LOAD_TABLE_SYMBOL)?;
    let loads = parse_load_records(&load_bytes)?;
    let image_count = loads
        .iter()
        .take_while(|record| record.operation == 1)
        .count();
    ensure!(
        image_count == 29
            && loads.len() == 45
            && loads[..image_count]
                .iter()
                .all(|record| record.operation == 1)
            && loads[image_count..]
                .iter()
                .all(|record| record.operation == 2),
        "NazoTexTbl image/palette load population changed"
    );
    let image_load = loads[IMAGE_LOAD_INDEX];
    let palette_load = loads[image_count + PALETTE_SLOT];
    ensure!(
        image_load.file_record == IMAGE_FILE_RECORD
            && palette_load.file_record == PALETTE_FILE_RECORD,
        "NazoTexTbl slot-1 main-font clone file records changed"
    );

    let (_, file_list_bytes) = read_symbol_bytes(&file, FILE_LIST_SYMBOL)?;
    let records = parse_file_records(&file_list_bytes)?;
    let image_record = *records
        .get(image_load.file_record)
        .context("Nazo menu font clone image record is out of range")?;
    let palette_record = *records
        .get(palette_load.file_record)
        .context("Nazo menu font clone palette record is out of range")?;
    let decoded = read_decoded_image(container, image_record)?;
    ensure!(
        decoded.len()
            == usize::try_from(u64::from(image_record.width) * u64::from(image_record.height))?,
        "Nazo menu font clone CPX has unexpected trailing decoded bytes"
    );
    let linear_indices = unswizzle_psmt8_uploaded_as_psmct32(
        &decoded,
        usize::try_from(image_record.width)?,
        usize::try_from(image_record.height)?,
    )?;
    let palette = read_palette(container, palette_record, palette_load.flags & 2 != 0)?;
    Ok(MemoryCardFontResources {
        image_record_index: image_load.file_record,
        image_record,
        palette_record_index: palette_load.file_record,
        palette,
        linear_indices,
    })
}

fn validate_nazo_registration_path(report: &NazoFontAtlasReport) -> Result<()> {
    ensure!(
        report.registrations.registrar_symbol == REGISTRAR_SYMBOL
            && report.registrations.registration_buffer_symbol == REGISTRATION_BUFFER_SYMBOL
            && report.registrations.record_byte_length == REGISTRATION_RECORD_BYTES,
        "Nazo registration no longer writes 0x90-byte RegBuff records"
    );
    validate_registration_sets(
        report
            .registrations
            .registration_sets
            .iter()
            .map(|set| (set.symbol.as_str(), set.pairs.as_slice())),
    )?;
    let called_sets = report
        .registrations
        .call_sites
        .iter()
        .map(|call| call.registration_set_symbol.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        called_sets == EXPECTED_REGISTRATION_SETS.into_iter().collect(),
        "not every Nazo registration set reaches ActRegList_Regist3"
    );
    Ok(())
}

fn validate_registration_sets<'a>(
    sets: impl IntoIterator<Item = (&'a str, &'a [NazoTextureRegistrationPair])>,
) -> Result<()> {
    let mut names = BTreeSet::new();
    for (name, pairs) in sets {
        ensure!(
            names.insert(name),
            "duplicate Nazo registration set: {name}"
        );
        let pair = pairs
            .get(REGISTRATION_SLOT)
            .with_context(|| format!("{name} has no RegBuff slot 1"))?;
        ensure!(
            pair.image_load_index == IMAGE_LOAD_INDEX && pair.palette_slot == PALETTE_SLOT,
            "{name} no longer maps RegBuff slot 1 to NazoTexTbl image 9/palette 9"
        );
    }
    ensure!(
        names == EXPECTED_REGISTRATION_SETS.into_iter().collect(),
        "Nazo registration-set population changed"
    );
    Ok(())
}

fn validate_my_nazo_error_call(file: &object::File<'_>, executable: &[u8]) -> Result<()> {
    let renderer = read_executable_symbol(executable, ERROR_RENDERER_SYMBOL)?;
    let targets = BTreeMap::from([(
        u32::try_from(renderer.address)?,
        ERROR_RENDERER_SYMBOL.to_string(),
    )]);
    let calls = find_direct_calls(file, &targets)?;
    ensure!(
        calls.len() == 1
            && calls[0].target_symbol == ERROR_RENDERER_SYMBOL
            && calls[0]
                .owner_symbols
                .iter()
                .any(|owner| owner == ERROR_CALLER_SYMBOL),
        "MyNazo_ErrMsg is no longer called exactly once from Nazo_Menu_Run"
    );
    Ok(())
}

#[cfg(test)]
#[path = "nazo_font_clone_tests.rs"]
mod nazo_font_clone_tests;
