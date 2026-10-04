use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use r5900::{GeneralRegister, InstructionForm, Operands};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cpx::{CPX_BLOCK_ALIGNMENT, encode_cpx_block};
use crate::disc_patch::{
    IsoFileCandidate, plan_sector_patches, verify_iso_readback, write_mode2_cue, write_patched_bin,
};
use crate::executable::{disassemble_symbol, read_executable_symbol, validate_r5900_instruction};
use crate::executable_font_consumers::{
    ExecutableFontConsumerInputs, survey_executable_font_consumers,
};
use crate::font_atlas::{FontAtlasInputs, render_font_atlas};
use crate::glyph_allocation::{GlyphAllocationInputs, plan_glyph_allocation};
use crate::indexed_texture::swizzle_psmt8_uploaded_as_psmct32;
use crate::memory_card_translation::{
    MemoryCardTranslationBundle, MemoryCardTranslationEntry, MemoryCardTranslationInputs,
    TranslationStatus, translation_entry, validate_memory_card_translation_bundle,
};
use crate::memory_card_translation_build::font::{
    FontConfiguration, MemoryCardGlyphAssignment, choose_background_index, choose_foreground_index,
    collect_unique_hangul, decode_hex, encode_translation, load_font, load_font_resources,
    render_hangul, validate_protected_tokens,
};
use crate::message_corpus::{
    MessageCorpusInputs, survey_memory_card_messages, survey_memory_card_save_messages,
    survey_my_nazo_error_messages,
};
use crate::source::{DiscInventory, find_inventory_file};

const EXECUTABLE_ISO_PATH: &str = "/SLPM_624.00;1";
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_D.BIN;1";
const MESSAGE_ID: &str = "memory-card/load/check";
const MESSAGE_SYMBOL: &str = "MemLoadMsg_Check";
const FULLWIDTH_TABLE_SYMBOL: &str = "FontList_Zen";
const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image";
const MESSAGE_CONSUMER_SYMBOL: &str = "MsgPrint_Act";
const LOOKUP_SYMBOL: &str = "GetFontParam";
const IMAGE_FILE_RECORD_BYTES: usize = 20;

pub struct MemoryCardPocInputs<'a> {
    pub inventory: &'a DiscInventory,
    pub source_bin_path: &'a Path,
    pub executable: &'a [u8],
    pub image_container: &'a [u8],
    pub translation_path: &'a Path,
    pub font_config_path: &'a Path,
    pub font_path: &'a Path,
    pub output_bin_path: &'a Path,
    pub output_cue_path: &'a Path,
    pub preview_path: &'a Path,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardPocReport {
    pub schema: u32,
    pub status: &'static str,
    pub release_candidate: bool,
    pub source_bin_sha256: String,
    pub output: PocOutput,
    pub translation: TranslationReport,
    pub font: FontReport,
    pub compression: CompressionReport,
    pub expected_writes: ExpectedWriteReport,
    pub verification: VerificationReport,
}

#[derive(Clone, Debug, Serialize)]
pub struct PocOutput {
    pub bin_path: String,
    pub bin_byte_length: u64,
    pub bin_sha256: String,
    pub cue_path: String,
    pub preview_path: String,
    pub preview_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationReport {
    pub asset_path: String,
    pub message_id: String,
    pub symbol: String,
    pub source_address: String,
    pub source_file_offset: String,
    pub storage_byte_length: usize,
    pub replacement_byte_length_with_nul: usize,
    pub preserved_tail_byte_length: usize,
    pub source_text: String,
    pub korean_text: String,
    pub approval_status: String,
    pub approval_recorded_at: String,
    pub approval_recorded_from: String,
    pub release_gate: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontReport {
    pub configuration_path: String,
    pub id: String,
    pub sha256: String,
    pub license: String,
    pub upstream_repository: String,
    pub pixel_size: f32,
    pub coverage_threshold: u8,
    pub background_palette_index: u8,
    pub foreground_palette_index: u8,
    pub assignments: Vec<MemoryCardGlyphAssignment>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompressionReport {
    pub image_file_record: usize,
    pub slot_byte_length: usize,
    pub source_encoded_byte_length: usize,
    pub patched_encoded_byte_length: usize,
    pub remaining_padding_byte_length: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExpectedWriteReport {
    pub executable_writes: usize,
    pub image_container_writes: usize,
    pub raw_sector_plans: usize,
    pub raw_sector_data_writes: usize,
    pub raw_sector_integrity_writes: usize,
    pub modified_lbas: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerificationReport {
    pub source_font_consumer: &'static str,
    pub fullwidth_lookup: &'static str,
    pub message_consumer: &'static str,
    pub cpx_roundtrip: &'static str,
    pub output_iso_readback: &'static str,
    pub english_scope: &'static str,
    pub runtime_status: &'static str,
}

pub fn build_memory_card_poc(inputs: MemoryCardPocInputs<'_>) -> Result<MemoryCardPocReport> {
    let source_hash = &inputs.inventory.source.bin.sha256;
    let translation_bundle: MemoryCardTranslationBundle = read_json(inputs.translation_path)?;
    let font_config: FontConfiguration = read_json(inputs.font_config_path)?;
    let font = load_font(&font_config, inputs.font_path)?;

    let executable_entry = find_inventory_file(inputs.inventory, EXECUTABLE_ISO_PATH)?;
    let image_entry = find_inventory_file(inputs.inventory, IMAGE_CONTAINER_ISO_PATH)?;
    ensure!(
        executable_entry.byte_length as usize == inputs.executable.len()
            && image_entry.byte_length as usize == inputs.image_container.len(),
        "verified ISO file lengths changed before PoC build"
    );
    let executable_hash = executable_entry
        .sha256
        .clone()
        .context("executable inventory entry has no SHA-256")?;
    let image_hash = image_entry
        .sha256
        .clone()
        .context("image inventory entry has no SHA-256")?;

    let temporary = tempfile::tempdir().context("failed to create PoC validation directory")?;
    let source_atlas_path = temporary.path().join("source-font-atlas.png");
    let source_atlas = render_font_atlas(
        FontAtlasInputs {
            source_bin_sha256: source_hash.clone(),
            executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
            executable_sha256: executable_hash.clone(),
            executable: inputs.executable,
            image_container_iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
            image_container_sha256: image_hash.clone(),
            image_container: inputs.image_container,
        },
        &source_atlas_path,
    )?;
    let load_messages = survey_memory_card_messages(MessageCorpusInputs {
        source_bin_sha256: source_hash.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_hash.clone(),
        executable: inputs.executable,
    })?;
    let save_messages = survey_memory_card_save_messages(MessageCorpusInputs {
        source_bin_sha256: source_hash.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_hash.clone(),
        executable: inputs.executable,
    })?;
    let my_nazo_error_messages = survey_my_nazo_error_messages(MessageCorpusInputs {
        source_bin_sha256: source_hash.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_hash.clone(),
        executable: inputs.executable,
    })?;
    validate_memory_card_translation_bundle(
        &translation_bundle,
        MemoryCardTranslationInputs {
            source_bin_sha256: source_hash,
            executable_iso_path: EXECUTABLE_ISO_PATH,
            executable_sha256: &executable_hash,
            load: &load_messages,
            save: &save_messages,
            my_nazo_error: &my_nazo_error_messages,
        },
    )?;
    let translation = translation_entry(&translation_bundle, MESSAGE_ID)?;
    validate_poc_translation(translation)?;
    validate_message_asset(translation, &load_messages, inputs.executable)?;
    let message_corpora = [load_messages, save_messages, my_nazo_error_messages];
    let font_consumers = survey_executable_font_consumers(ExecutableFontConsumerInputs {
        source_bin_sha256: source_hash.to_string(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_hash.clone(),
        executable: inputs.executable,
    })?;
    let allocation = plan_glyph_allocation(GlyphAllocationInputs {
        executable: inputs.executable,
        font_atlas: &source_atlas,
        message_corpora: &message_corpora,
        font_consumers: &font_consumers,
    })?;
    validate_message_consumer(inputs.executable)?;

    let hangul = collect_unique_hangul([translation.translation.text.as_str()])?;
    let candidates: Vec<_> = allocation
        .slots
        .iter()
        .filter(|slot| slot.classification == "shadowed-duplicate-candidate")
        .take(hangul.len())
        .collect();
    ensure!(
        candidates.len() == hangul.len(),
        "approved translation needs {} Hangul glyphs but only {} non-displacing slots are available",
        hangul.len(),
        candidates.len()
    );

    let mut font_resources =
        load_font_resources(inputs.executable, inputs.image_container, &source_atlas)?;
    ensure!(
        source_atlas.atlas.image_file_record == font_resources.image_record_index,
        "font resource selection differs from the validated source atlas"
    );
    let atlas_width = usize::try_from(font_resources.image_record.width)?;
    let background =
        choose_background_index(&font_resources.linear_indices, atlas_width, &candidates)?;
    let foreground = choose_foreground_index(
        &font_resources.linear_indices,
        &font_resources.palette,
        &source_atlas,
        atlas_width,
        background,
    )?;
    let assignments = render_hangul(
        &font,
        &hangul,
        &candidates,
        &font_config.rasterization,
        background,
        foreground,
        atlas_width,
        &mut font_resources.linear_indices,
    )?;

    let encoded_message = encode_translation(&translation.translation.text, &assignments)?;
    ensure!(
        encoded_message.len() <= translation.source.storage_byte_length,
        "encoded Korean message does not fit its fixed storage"
    );
    validate_protected_tokens(translation, &encoded_message)?;

    let swizzled = swizzle_psmt8_uploaded_as_psmct32(
        &font_resources.linear_indices,
        usize::try_from(font_resources.image_record.width)?,
        usize::try_from(font_resources.image_record.height)?,
    )?;
    let encoded_cpx = encode_cpx_block(&swizzled)?;
    let slot_offset = usize::try_from(font_resources.image_record.sector_offset)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("font slot offset overflow")?;
    let slot_length = usize::try_from(font_resources.image_record.sector_count)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("font slot length overflow")?;
    ensure!(
        encoded_cpx.len() <= slot_length,
        "patched font CPX is {} bytes but its fixed slot is only {slot_length} bytes",
        encoded_cpx.len()
    );
    let source_encoded_length =
        usize::try_from(font_resources.image_record.stored_byte_length)? + 1;
    let mut replacement_slot = encoded_cpx.clone();
    replacement_slot.resize(slot_length, 0);
    let image_plan = WritePlan::new()
        .region(ImageRegion {
            id: "font-cpx-slot".to_string(),
            range: slot_offset..slot_offset + slot_length,
            kind: RegionKind::Data,
            reason: "consumer-linked font atlas CPX slot".to_string(),
        })
        .write(ExpectedWrite {
            id: "replace-font-cpx-slot".to_string(),
            owner: "memory-card-poc-font".to_string(),
            purpose: "insert the approved development-PoC Hangul glyphs".to_string(),
            offset: slot_offset,
            expected_original: inputs.image_container[slot_offset..slot_offset + slot_length]
                .to_vec(),
            replacement: replacement_slot,
            intent: WriteIntent::Data,
        });
    let patched_image = image_plan.apply(inputs.image_container, None)?;

    let (patched_executable, executable_write_count) = patch_executable(
        inputs.executable,
        translation,
        &assignments,
        &encoded_message,
        font_resources.image_record_index,
        encoded_cpx.len(),
    )?;
    validate_patched_executable(
        inputs.executable,
        &patched_executable,
        translation,
        &assignments,
        &encoded_message,
    )?;

    let file_candidates = vec![
        IsoFileCandidate {
            iso_path: EXECUTABLE_ISO_PATH.to_string(),
            extent_lba: executable_entry.extent_lba,
            original: inputs.executable.to_vec(),
            replacement: patched_executable.clone(),
        },
        IsoFileCandidate {
            iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
            extent_lba: image_entry.extent_lba,
            original: inputs.image_container.to_vec(),
            replacement: patched_image.clone(),
        },
    ];
    let sector_patches = plan_sector_patches(inputs.source_bin_path, &file_candidates)?;
    let output_hash = write_patched_bin(
        inputs.source_bin_path,
        inputs.output_bin_path,
        &sector_patches,
    )?;
    write_mode2_cue(inputs.output_cue_path, inputs.output_bin_path)?;
    verify_output_readback(
        inputs.output_bin_path,
        &file_candidates,
        &patched_executable,
        &patched_image,
        inputs.preview_path,
        source_hash,
        &output_hash,
    )?;
    let preview_bytes = fs::read(inputs.preview_path)
        .with_context(|| format!("failed to read preview {}", inputs.preview_path.display()))?;
    let approval = translation
        .translation
        .human_approval
        .as_ref()
        .context("development PoC translation lost its human approval")?;

    let modified_lbas = sector_patches.iter().map(|patch| patch.lba).collect();
    let raw_sector_data_writes = sector_patches.iter().map(|patch| patch.data_writes).sum();
    Ok(MemoryCardPocReport {
        schema: 1,
        status: "development-poc-built; runtime verification pending",
        release_candidate: false,
        source_bin_sha256: source_hash.clone(),
        output: PocOutput {
            bin_path: path_string(inputs.output_bin_path)?,
            bin_byte_length: inputs.inventory.source.bin.byte_length,
            bin_sha256: output_hash,
            cue_path: path_string(inputs.output_cue_path)?,
            preview_path: path_string(inputs.preview_path)?,
            preview_sha256: sha256(&preview_bytes),
        },
        translation: TranslationReport {
            asset_path: path_string(inputs.translation_path)?,
            message_id: translation.source.message_id.clone(),
            symbol: translation.source.symbol.clone(),
            source_address: translation.source.source_address.clone(),
            source_file_offset: translation.source.source_file_offset.clone(),
            storage_byte_length: translation.source.storage_byte_length,
            replacement_byte_length_with_nul: encoded_message.len(),
            preserved_tail_byte_length: translation.source.storage_byte_length
                - encoded_message.len(),
            source_text: translation.source.source_text.clone(),
            korean_text: translation.translation.text.clone(),
            approval_status: translation.translation.status.as_str().to_string(),
            approval_recorded_at: approval.recorded_at.clone(),
            approval_recorded_from: approval.recorded_from.clone(),
            release_gate: translation.translation.translator_notes.join(" "),
        },
        font: FontReport {
            configuration_path: path_string(inputs.font_config_path)?,
            id: font_config.id,
            sha256: font_config.sha256,
            license: font_config.license,
            upstream_repository: font_config.upstream_repository,
            pixel_size: font_config.rasterization.pixel_size,
            coverage_threshold: font_config.rasterization.coverage_threshold,
            background_palette_index: background,
            foreground_palette_index: foreground,
            assignments,
        },
        compression: CompressionReport {
            image_file_record: font_resources.image_record_index,
            slot_byte_length: slot_length,
            source_encoded_byte_length: source_encoded_length,
            patched_encoded_byte_length: encoded_cpx.len(),
            remaining_padding_byte_length: slot_length - encoded_cpx.len(),
        },
        expected_writes: ExpectedWriteReport {
            executable_writes: executable_write_count,
            image_container_writes: image_plan.writes.len(),
            raw_sector_plans: sector_patches.len(),
            raw_sector_data_writes,
            raw_sector_integrity_writes: sector_patches.len(),
            modified_lbas,
        },
        verification: VerificationReport {
            source_font_consumer: "MemLoadTexTbl slot 1 -> IMAGE_D record 21 -> FontPrint_Act",
            fullwidth_lookup: "GetFontParam typed decode/encode and first-match custom-code validation passed",
            message_consumer: "MsgPrint_Act typed fullwidth call, 24-pixel advance, and two-byte pointer advance passed",
            cpx_roundtrip: "patched swizzle -> CPX encode -> CPX decode is exact",
            output_iso_readback: "patched executable and IMAGE_D extents match the planned file candidates",
            english_scope: "MEMORY CARD and both (PS2) runs are preserved byte-for-byte; only Japanese content was replaced",
            runtime_status: "pending PCSX2 visible-consumer verification",
        },
    })
}

fn validate_poc_translation(asset: &MemoryCardTranslationEntry) -> Result<()> {
    ensure!(
        matches!(
            asset.translation.status,
            TranslationStatus::NeedsReview | TranslationStatus::Complete
        ) && asset.translation.human_approval.is_some()
            && !asset.translation.text.is_empty(),
        "bounded development PoC translation is no longer human-approved"
    );
    Ok(())
}

fn validate_message_asset(
    asset: &MemoryCardTranslationEntry,
    report: &crate::message_corpus::MessageCorpusReport,
    executable: &[u8],
) -> Result<()> {
    ensure!(
        asset.source.message_id == MESSAGE_ID && asset.source.symbol == MESSAGE_SYMBOL,
        "translation asset identifies a different source message"
    );
    let entry = report
        .messages
        .iter()
        .find(|entry| entry.message_id == MESSAGE_ID)
        .context("validated message corpus no longer contains the PoC message")?;
    ensure!(
        entry.symbol == asset.source.symbol
            && entry.source_address == asset.source.source_address
            && entry.source_file_offset == asset.source.source_file_offset
            && entry.storage_byte_length == asset.source.storage_byte_length
            && entry.source_sha256 == asset.source.source_sha256
            && entry.source_hex == asset.source.source_hex,
        "translation asset source identity differs from the fresh message census"
    );
    let symbol = read_executable_symbol(executable, MESSAGE_SYMBOL)?;
    ensure!(
        format!("{:#010x}", symbol.address) == asset.source.source_address
            && format!("{:#010x}", symbol.file_offset) == asset.source.source_file_offset
            && symbol.bytes == decode_hex(&asset.source.source_hex)?,
        "translation asset raw bytes differ from the executable symbol"
    );
    ensure!(
        sha256(&symbol.bytes) == asset.source.source_sha256,
        "translation asset source SHA-256 differs from the executable symbol"
    );
    Ok(())
}

fn validate_message_consumer(executable: &[u8]) -> Result<()> {
    let disassembly = disassemble_symbol(executable, MESSAGE_CONSUMER_SYMBOL)?;
    ensure!(
        disassembly.instructions.len() == 231,
        "MsgPrint_Act instruction population changed"
    );
    let consumer = read_executable_symbol(executable, MESSAGE_CONSUMER_SYMBOL)?;
    let lookup = read_executable_symbol(executable, LOOKUP_SYMBOL)?;
    let call = decode_instruction(&consumer.bytes, consumer.address, 0x1e8)?;
    ensure!(
        call.form() == InstructionForm::Jal
            && matches!(call.operands(), Operands::Jump { target } if u64::from(*target) == lookup.address),
        "MsgPrint_Act no longer calls GetFontParam on its fullwidth path"
    );
    require_addiu(
        &consumer,
        0x278,
        GeneralRegister::S2,
        GeneralRegister::S2,
        24,
    )?;
    require_addiu(
        &consumer,
        0x280,
        GeneralRegister::S0,
        GeneralRegister::S0,
        2,
    )?;
    Ok(())
}

fn require_addiu(
    symbol: &crate::executable::ExecutableSymbolBytes,
    offset: usize,
    destination: GeneralRegister,
    source: GeneralRegister,
    immediate: i16,
) -> Result<()> {
    let instruction = decode_instruction(&symbol.bytes, symbol.address, offset)?;
    ensure!(
        instruction.form() == InstructionForm::Addiu
            && matches!(
                instruction.operands(),
                Operands::SignedImmediate {
                    destination: actual_destination,
                    source: actual_source,
                    immediate: actual_immediate,
                } if *actual_destination == destination
                    && *actual_source == source
                    && *actual_immediate == immediate
            ),
        "{} instruction changed at +{offset:#x}",
        symbol_name_for_diagnostic(symbol)
    );
    Ok(())
}

fn symbol_name_for_diagnostic(_symbol: &crate::executable::ExecutableSymbolBytes) -> &'static str {
    MESSAGE_CONSUMER_SYMBOL
}

fn decode_instruction(source: &[u8], address: u64, offset: usize) -> Result<r5900::Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .context("typed instruction offset exceeds symbol")?;
    validate_r5900_instruction(bytes, address + u64::try_from(offset)?)
}

fn patch_executable(
    executable: &[u8],
    asset: &MemoryCardTranslationEntry,
    assignments: &[MemoryCardGlyphAssignment],
    encoded_message: &[u8],
    image_file_record: usize,
    encoded_cpx_length: usize,
) -> Result<(Vec<u8>, usize)> {
    let font_table = read_executable_symbol(executable, FULLWIDTH_TABLE_SYMBOL)?;
    let file_list = read_executable_symbol(executable, IMAGE_FILE_LIST_SYMBOL)?;
    let message = read_executable_symbol(executable, MESSAGE_SYMBOL)?;
    let stored_length_offset = file_list
        .file_offset
        .checked_add(image_file_record * IMAGE_FILE_RECORD_BYTES + 8)
        .context("image file-list stored length offset overflow")?;
    let new_stored_length = u32::try_from(encoded_cpx_length - 1)?.to_le_bytes();

    let mut plan = WritePlan::new()
        .region(ImageRegion {
            id: "fullwidth-code-table".to_string(),
            range: font_table.file_offset..font_table.file_offset + font_table.bytes.len(),
            kind: RegionKind::Metadata,
            reason: "GetFontParam fullwidth two-byte lookup table".to_string(),
        })
        .region(ImageRegion {
            id: "font-cpx-stored-length".to_string(),
            range: stored_length_offset..stored_length_offset + 4,
            kind: RegionKind::Metadata,
            reason: "FileList_Image record length for the recompressed font CPX".to_string(),
        })
        .region(ImageRegion {
            id: "memory-card-check-message".to_string(),
            range: message.file_offset..message.file_offset + encoded_message.len(),
            kind: RegionKind::Data,
            reason: "human-approved Japanese-to-Korean development PoC message".to_string(),
        });
    for assignment in assignments {
        let offset = font_table.file_offset + assignment.slot_index * 2;
        let replacement = decode_hex(&assignment.code_hex)?;
        plan = plan.write(ExpectedWrite {
            id: format!("assign-fullwidth-slot-{}", assignment.slot_index),
            owner: "memory-card-poc-encoding".to_string(),
            purpose: format!(
                "assign {} to a shadowed duplicate slot",
                assignment.character
            ),
            offset,
            expected_original: executable[offset..offset + 2].to_vec(),
            replacement,
            intent: WriteIntent::Metadata,
        });
    }
    plan = plan
        .write(ExpectedWrite {
            id: "update-font-cpx-stored-length".to_string(),
            owner: "memory-card-poc-compressor".to_string(),
            purpose: "record the exact recompressed CPX length minus its terminator byte"
                .to_string(),
            offset: stored_length_offset,
            expected_original: executable[stored_length_offset..stored_length_offset + 4].to_vec(),
            replacement: new_stored_length.to_vec(),
            intent: WriteIntent::Metadata,
        })
        .write(ExpectedWrite {
            id: "replace-memory-card-check-message".to_string(),
            owner: "approved-memory-card-translation".to_string(),
            purpose: "replace only the approved Japanese message while preserving English and @P"
                .to_string(),
            offset: message.file_offset,
            expected_original: decode_hex(&asset.source.source_hex)?[..encoded_message.len()]
                .to_vec(),
            replacement: encoded_message.to_vec(),
            intent: WriteIntent::Data,
        });
    let write_count = plan.writes.len();
    Ok((plan.apply(executable, None)?, write_count))
}

fn validate_patched_executable(
    source: &[u8],
    patched: &[u8],
    asset: &MemoryCardTranslationEntry,
    assignments: &[MemoryCardGlyphAssignment],
    encoded_message: &[u8],
) -> Result<()> {
    let table = read_executable_symbol(patched, FULLWIDTH_TABLE_SYMBOL)?;
    for assignment in assignments {
        let code = decode_hex(&assignment.code_hex)?;
        let first_match = table.bytes[..table.bytes.len() - 1]
            .chunks_exact(2)
            .position(|entry| entry == code)
            .with_context(|| {
                format!("custom code is absent after patch: {}", assignment.code_hex)
            })?;
        ensure!(
            first_match == assignment.slot_index,
            "custom code first match differs for {}",
            assignment.character
        );
    }
    ensure!(
        table.bytes.last() == Some(&0),
        "fullwidth table terminator changed"
    );
    let source_message = read_executable_symbol(source, MESSAGE_SYMBOL)?;
    let patched_message = read_executable_symbol(patched, MESSAGE_SYMBOL)?;
    ensure!(
        patched_message.bytes[..encoded_message.len()] == *encoded_message,
        "patched message bytes differ from the approved encoding"
    );
    ensure!(
        patched_message.bytes[encoded_message.len()..]
            == source_message.bytes[encoded_message.len()..],
        "bytes after the early NUL changed"
    );
    ensure!(
        patched_message.bytes[encoded_message.len() - 1] == 0
            && asset.source.storage_byte_length == patched_message.bytes.len(),
        "patched fixed-storage boundary changed"
    );
    validate_message_consumer(patched)
}

#[allow(clippy::too_many_arguments)]
fn verify_output_readback(
    output_path: &Path,
    files: &[IsoFileCandidate],
    patched_executable: &[u8],
    patched_image: &[u8],
    preview_path: &Path,
    source_hash: &str,
    output_hash: &str,
) -> Result<()> {
    verify_iso_readback(output_path, files)?;
    let report = render_font_atlas(
        FontAtlasInputs {
            source_bin_sha256: output_hash.to_string(),
            executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
            executable_sha256: sha256(patched_executable),
            executable: patched_executable,
            image_container_iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
            image_container_sha256: sha256(patched_image),
            image_container: patched_image,
        },
        preview_path,
    )?;
    ensure!(
        report.source_bin_sha256 != source_hash
            && report.atlas.png_sha256 == hash_file(preview_path)?,
        "patched font preview identity check failed"
    );
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("path is not UTF-8: {}", path.display()))
}

#[cfg(test)]
#[path = "memory_card_poc_tests.rs"]
mod memory_card_poc_tests;
