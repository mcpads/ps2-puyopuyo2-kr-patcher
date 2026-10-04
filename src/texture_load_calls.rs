use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use r5900::{InstructionForm, Operands};
use serde::Serialize;

use self::analysis::{
    AbstractValue, AnalysisContext, FILE_ID_REGISTER, LOAD_TABLE_REGISTER, find_calls,
    resolve_register, typed_evidence_at, validate_instruction_at,
};
use self::record_coverage::audit_file_record_coverage;
use crate::indexed_texture::sha256;
use crate::texture_load_census::{TextureLoadCensusInputs, survey_texture_load_tables};
use crate::typed_evidence::{TypedInstructionEvidence, symbol_address};

mod analysis;
mod record_coverage;

const READ_FILE_SYMBOL: &str = "aqCdvd_ReadFile";

#[derive(Clone, Debug)]
pub struct TextureLoadCallInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureLoadCallSurvey {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub read_file_symbol: String,
    pub read_file_address: String,
    pub summary: TextureLoadCallSummary,
    pub calls: Vec<TextureLoadCall>,
    pub source_coverage: Vec<TextureCallSourceCoverage>,
    pub invalid_record_references: Vec<InvalidFileRecordReference>,
    pub unreachable_load_tables: Vec<String>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureLoadCallSummary {
    pub read_file_calls: usize,
    pub texture_load_calls: usize,
    pub other_load_calls: usize,
    pub direct_load_table_calls: usize,
    pub pointer_table_calls: usize,
    pub source_compatible_texture_calls: usize,
    pub source_incompatible_texture_calls: usize,
    pub load_tables: usize,
    pub reachable_load_tables: usize,
    pub source_images: usize,
    pub referenced_images: usize,
    pub unreferenced_images: usize,
    pub invalid_record_references: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureLoadCall {
    pub owner_symbol: String,
    pub call_address: String,
    pub file_id: usize,
    pub source_compatible: Option<bool>,
    pub source: TextureLoadArgumentSource,
    pub possible_load_tables: Vec<String>,
    pub argument_instructions: Vec<TypedInstructionEvidence>,
    pub call_instruction: TypedInstructionEvidence,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureCallSourceCoverage {
    pub file_id: usize,
    pub iso_path: String,
    pub image_records: usize,
    pub referenced_image_records: usize,
    pub unreferenced_image_records: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InvalidFileRecordReference {
    pub owner_symbol: String,
    pub call_address: String,
    pub file_id: usize,
    pub table_symbol: String,
    pub operation: u32,
    pub file_record: usize,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextureLoadArgumentSource {
    DirectLoadTable {
        symbol: String,
    },
    PointerTable {
        symbol: String,
    },
    OtherStaticTable {
        symbol: Option<String>,
        address: String,
    },
    OtherDynamic,
}

pub fn survey_texture_load_calls(
    inputs: TextureLoadCallInputs<'_>,
) -> Result<TextureLoadCallSurvey> {
    ensure!(
        sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips,
        "ELF architecture is not 32-bit MIPS"
    );
    ensure!(
        file.endianness() == Endianness::Little,
        "ELF is not little-endian"
    );
    let census = survey_texture_load_tables(TextureLoadCensusInputs {
        source_bin_sha256: inputs.source_bin_sha256.clone(),
        executable_iso_path: inputs.executable_iso_path.clone(),
        executable_sha256: inputs.executable_sha256.clone(),
        executable: inputs.executable,
    })?;
    let context = AnalysisContext::from_census(&file, &census)?;
    let read_file_address = symbol_address(&file, READ_FILE_SYMBOL)?;
    let raw_calls = find_calls(&file, read_file_address)?;
    ensure!(!raw_calls.is_empty(), "ELF has no aqCdvd_ReadFile calls");

    let mut calls = Vec::with_capacity(raw_calls.len());
    let mut reachable_addresses = BTreeSet::new();
    let mut texture_load_calls = 0_usize;
    let mut other_load_calls = 0_usize;
    let mut direct_load_table_calls = 0_usize;
    let mut pointer_table_calls = 0_usize;
    let mut source_compatible_texture_calls = 0_usize;
    for raw_call in raw_calls {
        let file_id_value = resolve_register(
            &context,
            &raw_call.owner,
            FILE_ID_REGISTER,
            raw_call.instruction_index + 1,
            0,
        )?;
        let AbstractValue::Constant(file_id) = file_id_value.value else {
            anyhow::bail!(
                "{} call at {:#x} has no static file id",
                READ_FILE_SYMBOL,
                raw_call.owner.address + u64::try_from(raw_call.instruction_index * 4)?
            );
        };
        let file_id = usize::try_from(file_id)?;
        let table_value = resolve_register(
            &context,
            &raw_call.owner,
            LOAD_TABLE_REGISTER,
            raw_call.instruction_index + 1,
            0,
        )?;
        let (source, possible_addresses, source_compatible) = match &table_value.value {
            AbstractValue::Constant(address) if context.load_tables.contains_key(address) => {
                texture_load_calls += 1;
                direct_load_table_calls += 1;
                let symbol = context.load_table_symbols[address].clone();
                (
                    TextureLoadArgumentSource::DirectLoadTable { symbol },
                    BTreeSet::from([*address]),
                    Some(context.load_tables[address].file_id == file_id),
                )
            }
            AbstractValue::LoadTableSet {
                pointer_table_symbol,
                addresses,
            } => {
                ensure!(!addresses.is_empty(), "pointer table has no load tables");
                texture_load_calls += 1;
                pointer_table_calls += 1;
                let source_compatible = addresses
                    .iter()
                    .all(|address| context.load_tables[address].file_id == file_id);
                (
                    TextureLoadArgumentSource::PointerTable {
                        symbol: pointer_table_symbol.clone(),
                    },
                    addresses.clone(),
                    Some(source_compatible),
                )
            }
            AbstractValue::Constant(address) => {
                other_load_calls += 1;
                (
                    TextureLoadArgumentSource::OtherStaticTable {
                        symbol: context.symbols_by_address.get(address).cloned(),
                        address: format!("{address:#010x}"),
                    },
                    BTreeSet::new(),
                    None,
                )
            }
            _ if file_id > 2 => {
                other_load_calls += 1;
                (
                    TextureLoadArgumentSource::OtherDynamic,
                    BTreeSet::new(),
                    None,
                )
            }
            _ => anyhow::bail!(
                "{} call at {:#x} for image file id {} has no resolved table source",
                READ_FILE_SYMBOL,
                raw_call.owner.address + u64::try_from(raw_call.instruction_index * 4)?,
                file_id
            ),
        };
        if source_compatible == Some(true) {
            source_compatible_texture_calls += 1;
        }
        reachable_addresses.extend(possible_addresses.iter().copied());
        let instruction_indices = file_id_value
            .instruction_indices
            .union(&table_value.instruction_indices)
            .copied()
            .collect::<BTreeSet<_>>();
        let possible_load_tables = possible_addresses
            .iter()
            .map(|address| context.load_table_symbols[address].clone())
            .collect();
        let call_address = raw_call.owner.address
            + u64::try_from(
                raw_call
                    .instruction_index
                    .checked_mul(4)
                    .context("call overflow")?,
            )?;
        let call_instruction =
            validate_instruction_at(&raw_call.owner, raw_call.instruction_index)?;
        ensure!(
            call_instruction.form() == InstructionForm::Jal
                && matches!(call_instruction.operands(), Operands::Jump { target } if u64::from(*target) == read_file_address),
            "read-file call changed during typed validation"
        );
        calls.push(TextureLoadCall {
            owner_symbol: raw_call.owner.name.clone(),
            call_address: format!("{call_address:#010x}"),
            file_id,
            source_compatible,
            source,
            possible_load_tables,
            argument_instructions: instruction_indices
                .into_iter()
                .map(|index| typed_evidence_at(&raw_call.owner, index))
                .collect::<Result<_>>()?,
            call_instruction: typed_evidence_at(&raw_call.owner, raw_call.instruction_index)?,
        });
    }
    calls.sort_by(|left, right| left.call_address.cmp(&right.call_address));

    let unreachable_load_tables = context
        .load_table_symbols
        .iter()
        .filter(|(address, _)| !reachable_addresses.contains(address))
        .map(|(_, symbol)| symbol.clone())
        .collect::<Vec<_>>();
    ensure!(
        unreachable_load_tables.is_empty(),
        "some texture load tables are unreachable from the resolved call population"
    );
    ensure!(
        direct_load_table_calls + pointer_table_calls == texture_load_calls,
        "texture read-file calls are not fully classified"
    );
    ensure!(
        texture_load_calls + other_load_calls == calls.len(),
        "read-file calls are not fully classified"
    );
    let record_audit = audit_file_record_coverage(&file, &census, &calls)?;
    ensure!(
        record_audit
            .source_coverage
            .iter()
            .map(|source| source.unreferenced_image_records.clone())
            .eq(census
                .source_coverage
                .iter()
                .map(|source| source.unloaded_image_records.clone())),
        "call-resolved and structural texture image coverage differ"
    );

    Ok(TextureLoadCallSurvey {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        read_file_symbol: READ_FILE_SYMBOL.to_string(),
        read_file_address: format!("{read_file_address:#010x}"),
        summary: TextureLoadCallSummary {
            read_file_calls: calls.len(),
            texture_load_calls,
            other_load_calls,
            direct_load_table_calls,
            pointer_table_calls,
            source_compatible_texture_calls,
            source_incompatible_texture_calls: texture_load_calls - source_compatible_texture_calls,
            load_tables: context.load_tables.len(),
            reachable_load_tables: reachable_addresses.len(),
            source_images: record_audit.source_images,
            referenced_images: record_audit.referenced_images,
            unreferenced_images: record_audit.unreferenced_images,
            invalid_record_references: record_audit.invalid_record_references.len(),
        },
        calls,
        source_coverage: record_audit.source_coverage,
        invalid_record_references: record_audit.invalid_record_references,
        unreachable_load_tables,
        evidence_boundary: "Static typed census of every aqCdvd_ReadFile call and its resolved direct or pointer-table load argument. Pointer-table entries are possible targets, not path-specific runtime reachability. Source-incompatible calls remain explicit instead of being admitted as valid loads. The result excludes file records absent from every resolved table under its actual file id, but does not prove that each reachable table executes at runtime.",
    })
}
