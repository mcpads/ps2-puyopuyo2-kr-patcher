use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::{Deserialize, Serialize};

use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{FileRecord, parse_file_records, render_indexed_texture, sha256};
use crate::texture_load_census::{
    TextureLoadCensusInputs, TextureLoadRecord, TextureTableClassification,
    survey_texture_load_tables,
};

mod consumer_pairs;
mod residual_pairs;

use consumer_pairs::resolve_consumer_pairs;
use residual_pairs::plan_residual_pairs;

#[derive(Clone, Debug)]
pub struct TexturePreviewCorpusInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
    pub containers: Vec<TexturePreviewContainerInput<'a>>,
}

#[derive(Clone, Debug)]
pub struct TexturePreviewContainerInput<'a> {
    pub file_id: usize,
    pub iso_path: String,
    pub sha256: String,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TexturePreviewCorpus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub containers: Vec<TexturePreviewContainerIdentity>,
    pub summary: TexturePreviewSummary,
    pub consumer_pair_tables: Vec<ConsumerPairTable>,
    pub residual_pairs: Vec<ResidualPreviewPair>,
    pub skipped_tables: Vec<SkippedTextureTable>,
    pub previews: Vec<TexturePreview>,
    pub evidence_boundary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TexturePreviewContainerIdentity {
    pub file_id: usize,
    pub iso_path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TexturePreviewSummary {
    pub load_tables: usize,
    pub load_order_tables: usize,
    pub consumer_pair_tables: usize,
    pub skipped_mismatched_tables: usize,
    pub unpaired_image_loads: usize,
    pub pair_instances: usize,
    pub residual_images: usize,
    pub residual_preview_variants: usize,
    pub source_images: usize,
    pub previewed_images: usize,
    pub unique_previews: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumerPairTable {
    pub symbol: String,
    pub pointer_table_symbol: Option<String>,
    pub registration_sets: Vec<ConsumerRegistrationSet>,
    pub image_loads: usize,
    pub palette_loads: usize,
    pub pair_instances: usize,
    pub unique_pairs: usize,
    pub paired_image_loads: usize,
    pub unpaired_image_load_indices: Vec<usize>,
    pub unpaired_image_file_records: Vec<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumerRegistrationSet {
    pub symbol: String,
    pub pairs: Vec<ConsumerRegistrationPair>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumerRegistrationPair {
    pub image_load_index: usize,
    pub palette_slot: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResidualPreviewPair {
    pub file_id: usize,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub reason: ResidualImageReason,
    pub palette_load_flags: Vec<u32>,
    pub preview_alpha_variants: Vec<bool>,
    pub pairing_basis: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResidualImageReason {
    NotInLoadTable,
    LoadedButNotRegistered {
        load_table_symbol: String,
        image_load_index: usize,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkippedTextureTable {
    pub symbol: String,
    pub image_loads: usize,
    pub palette_loads: usize,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TexturePreview {
    pub file_id: usize,
    pub image_container_iso_path: String,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub clear_first_alpha: bool,
    pub owner_tables: Vec<String>,
    pub width: u32,
    pub height: u32,
    pub decoded_trailing_byte_length: u64,
    pub swizzled_indices_sha256: String,
    pub linear_indices_sha256: String,
    pub uploaded_palette_sha256: String,
    pub rgba_sha256: String,
    pub png_path: String,
    pub png_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct PreviewKey {
    file_id: usize,
    image_file_record: usize,
    palette_file_record: usize,
    clear_first_alpha: bool,
}

struct LoadedContainer<'a> {
    identity: TexturePreviewContainerIdentity,
    bytes: &'a [u8],
    file_records: Vec<FileRecord>,
}

pub fn render_texture_preview_corpus(
    inputs: TexturePreviewCorpusInputs<'_>,
    output_directory: &Path,
) -> Result<TexturePreviewCorpus> {
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
    let containers = load_containers(&file, &census.file_sources, inputs.containers)?;
    let mut planned = BTreeMap::<PreviewKey, Vec<String>>::new();
    let mut consumer_pair_tables = Vec::new();
    let mut skipped_tables = Vec::new();
    let mut load_tables = 0_usize;
    let mut load_order_tables = 0_usize;
    let mut pair_instances = 0_usize;
    for candidate in &census.candidates {
        let TextureTableClassification::LoadTable {
            records,
            compatible_sources,
            ..
        } = &candidate.classification
        else {
            continue;
        };
        load_tables += 1;
        let source = compatible_sources
            .as_slice()
            .first()
            .filter(|_| compatible_sources.len() == 1)
            .context("preview load table does not have exactly one source")?;
        let pairs = if let Some(pairs) = plan_load_order_pairs(source.file_id, records) {
            load_order_tables += 1;
            pairs
        } else if let Some(resolution) =
            resolve_consumer_pairs(&file, &candidate.symbol, source.file_id, records)?
        {
            let pairs = resolution.pairs;
            consumer_pair_tables.push(resolution.evidence);
            pairs
        } else {
            skipped_tables.push(SkippedTextureTable {
                symbol: candidate.symbol.clone(),
                image_loads: records
                    .iter()
                    .filter(|record| record.operation == 1)
                    .count(),
                palette_loads: records
                    .iter()
                    .filter(|record| record.operation == 2)
                    .count(),
                reason: "image and palette load counts differ and no consumer pairing is declared"
                    .to_string(),
            });
            continue;
        };
        pair_instances += pairs.len();
        for pair in pairs {
            planned
                .entry(pair)
                .or_default()
                .push(candidate.symbol.clone());
        }
    }

    let residual_plan = plan_residual_pairs(&census, &consumer_pair_tables, &containers)?;
    let paired_preview_images = planned
        .keys()
        .map(|key| (key.file_id, key.image_file_record))
        .collect::<std::collections::BTreeSet<_>>();
    for planned_residual in &residual_plan.planned {
        ensure!(
            !paired_preview_images.contains(&(
                planned_residual.key.file_id,
                planned_residual.key.image_file_record
            )),
            "residual preview image already has a load or registration preview: file {} record {}",
            planned_residual.key.file_id,
            planned_residual.key.image_file_record
        );
        planned
            .entry(planned_residual.key)
            .or_default()
            .extend(planned_residual.owner_tables.iter().cloned());
    }

    let mut previews = Vec::with_capacity(planned.len());
    for (key, mut owner_tables) in planned {
        owner_tables.sort();
        owner_tables.dedup();
        let container = containers
            .get(&key.file_id)
            .with_context(|| format!("preview container is missing for file id {}", key.file_id))?;
        let image_record = *container
            .file_records
            .get(key.image_file_record)
            .context("preview image file record is out of range")?;
        let palette_record = *container
            .file_records
            .get(key.palette_file_record)
            .context("preview palette file record is out of range")?;
        let output_path = output_directory.join(format!(
            "file-{}-image-{:03}-palette-{:03}-alpha-{}.png",
            key.file_id,
            key.image_file_record,
            key.palette_file_record,
            usize::from(key.clear_first_alpha)
        ));
        let rendered = render_indexed_texture(
            container.bytes,
            image_record,
            palette_record,
            key.clear_first_alpha,
            &output_path,
        )?;
        previews.push(TexturePreview {
            file_id: key.file_id,
            image_container_iso_path: container.identity.iso_path.clone(),
            image_file_record: key.image_file_record,
            palette_file_record: key.palette_file_record,
            clear_first_alpha: key.clear_first_alpha,
            owner_tables,
            width: rendered.width,
            height: rendered.height,
            decoded_trailing_byte_length: rendered.decoded_trailing_byte_length,
            swizzled_indices_sha256: rendered.swizzled_indices_sha256,
            linear_indices_sha256: rendered.linear_indices_sha256,
            uploaded_palette_sha256: rendered.uploaded_palette_sha256,
            rgba_sha256: rendered.rgba_sha256,
            png_path: rendered.png_path,
            png_sha256: rendered.png_sha256,
        });
    }

    let previewed_images = previews
        .iter()
        .map(|preview| (preview.file_id, preview.image_file_record))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let source_images = containers
        .values()
        .map(|container| {
            container
                .file_records
                .iter()
                .filter(|record| record.width > 0 && record.height > 0)
                .count()
        })
        .sum();
    ensure!(
        previewed_images == source_images,
        "preview corpus does not cover every source image record"
    );

    Ok(TexturePreviewCorpus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        containers: containers
            .values()
            .map(|container| container.identity.clone())
            .collect(),
        summary: TexturePreviewSummary {
            load_tables,
            load_order_tables,
            consumer_pair_tables: consumer_pair_tables.len(),
            skipped_mismatched_tables: skipped_tables.len(),
            unpaired_image_loads: consumer_pair_tables
                .iter()
                .map(|table| table.unpaired_image_load_indices.len())
                .sum(),
            pair_instances,
            residual_images: residual_plan.evidence.len(),
            residual_preview_variants: residual_plan.planned.len(),
            source_images,
            previewed_images,
            unique_previews: previews.len(),
        },
        consumer_pair_tables,
        residual_pairs: residual_plan.evidence,
        skipped_tables,
        previews,
        evidence_boundary: "Load-order previews cover equal-count tables and declared mismatched tables use exact registration sets. Every remaining D/G/MD image has a separately identified structural residual pair; absent palette load flags produce both alpha variants. Residual pairing is a visual survey aid, not proof of loading or registration. The corpus covers every source image record but does not prove pattern display, runtime reachability, or language classification.".to_string(),
    })
}

fn plan_load_order_pairs(file_id: usize, records: &[TextureLoadRecord]) -> Option<Vec<PreviewKey>> {
    let images = records
        .iter()
        .filter(|record| record.operation == 1)
        .collect::<Vec<_>>();
    let palettes = records
        .iter()
        .filter(|record| record.operation == 2)
        .collect::<Vec<_>>();
    if images.len() != palettes.len() {
        return None;
    }
    Some(
        images
            .into_iter()
            .zip(palettes)
            .map(|(image, palette)| PreviewKey {
                file_id,
                image_file_record: image.file_record,
                palette_file_record: palette.file_record,
                clear_first_alpha: palette.flags & 2 != 0,
            })
            .collect(),
    )
}

fn load_containers<'a>(
    file: &object::File<'_>,
    sources: &[crate::texture_load_census::TextureFileSource],
    inputs: Vec<TexturePreviewContainerInput<'a>>,
) -> Result<BTreeMap<usize, LoadedContainer<'a>>> {
    let mut containers = BTreeMap::new();
    for input in inputs {
        ensure!(
            sha256(input.bytes) == input.sha256,
            "preview container bytes differ from the verified inventory hash: {}",
            input.iso_path
        );
        let source = sources
            .iter()
            .find(|source| source.file_id == input.file_id)
            .with_context(|| format!("unknown preview file id: {}", input.file_id))?;
        ensure!(
            source.iso_path == input.iso_path,
            "preview container path differs for file id {}",
            input.file_id
        );
        let (_, file_list_bytes) = read_symbol_bytes(file, &source.file_list_symbol)?;
        let file_records = parse_file_records(&file_list_bytes)?;
        ensure!(
            file_records.len() == source.file_records,
            "preview file-list length differs for file id {}",
            input.file_id
        );
        ensure!(
            containers
                .insert(
                    input.file_id,
                    LoadedContainer {
                        identity: TexturePreviewContainerIdentity {
                            file_id: input.file_id,
                            iso_path: input.iso_path,
                            sha256: input.sha256,
                        },
                        bytes: input.bytes,
                        file_records,
                    },
                )
                .is_none(),
            "duplicate preview container file id: {}",
            input.file_id
        );
    }
    ensure!(
        containers.len() == sources.len(),
        "preview corpus requires every linked IMAGE container"
    );
    Ok(containers)
}

#[cfg(test)]
#[path = "texture_preview_corpus/texture_preview_corpus_tests.rs"]
mod texture_preview_corpus_tests;
