use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::{Deserialize, Serialize};

use crate::cpx::{
    CPX_BLOCK_ALIGNMENT, link_image_file_lists, read_symbol_bytes, survey_cpx_container,
};
use crate::indexed_texture::{
    FileRecord, parse_file_records, read_decoded_image, read_palette, render_indexed_texture,
    sha256,
};

pub mod language_survey;

const PALETTE_BYTES: usize = 256 * 4;

#[derive(Clone, Debug)]
pub struct UnlinkedTextureCorpusInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
    pub container_iso_path: String,
    pub container_sha256: String,
    pub container: &'a [u8],
    pub linked_containers: Vec<LinkedTextureContainerInput<'a>>,
}

#[derive(Clone, Debug)]
pub struct LinkedTextureContainerInput<'a> {
    pub iso_path: String,
    pub sha256: String,
    pub bytes: &'a [u8],
    pub file_list_symbol: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnlinkedTextureCorpus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub container_iso_path: String,
    pub container_sha256: String,
    pub linked_container_sha256: BTreeMap<String, String>,
    pub summary: UnlinkedTextureCorpusSummary,
    pub images: Vec<UnlinkedTextureImage>,
    pub evidence_boundary: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnlinkedTextureCorpusSummary {
    pub images: usize,
    pub palette_slots: usize,
    pub preview_candidates: usize,
    pub images_with_one_dimension_candidate: usize,
    pub images_with_multiple_dimension_candidates: usize,
    pub images_with_exact_linked_pair_match: usize,
    pub images_with_linked_palette_match: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnlinkedTextureImage {
    pub image_index: usize,
    pub image_offset: u64,
    pub image_slot_byte_length: u64,
    pub encoded_byte_length: u64,
    pub decoded_byte_length: u64,
    pub decoded_sha256: String,
    pub palette_offset: u64,
    pub palette_sha256: String,
    pub uploaded_palette_sha256: String,
    pub dimension_source: DimensionCandidateSource,
    pub exact_linked_pair_matches: Vec<LinkedTexturePairMatch>,
    pub linked_palette_matches: Vec<LinkedTexturePairMatch>,
    pub previews: Vec<UnlinkedTexturePreview>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionCandidateSource {
    LinkedPixelCount,
    LinkedWidthInference,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LinkedTexturePairMatch {
    pub container_iso_path: String,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnlinkedTexturePreview {
    pub width: u32,
    pub height: u32,
    pub decoded_trailing_byte_length: u64,
    pub swizzled_indices_sha256: String,
    pub linear_indices_sha256: String,
    pub rgba_sha256: String,
    pub png_path: String,
    pub png_sha256: String,
}

#[derive(Clone)]
struct ReferencePair {
    identity: LinkedTexturePairMatch,
    decoded_sha256: String,
    uploaded_palette_sha256: String,
}

struct LinkedDimensionEvidence {
    by_pixel_count: BTreeMap<u32, BTreeSet<(u32, u32)>>,
    widths: BTreeSet<u32>,
}

pub fn render_unlinked_texture_corpus(
    inputs: UnlinkedTextureCorpusInputs<'_>,
    output_directory: &Path,
) -> Result<UnlinkedTextureCorpus> {
    ensure!(
        sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    ensure!(
        sha256(inputs.container) == inputs.container_sha256,
        "unlinked texture container differs from the verified inventory hash"
    );
    ensure!(
        !inputs.linked_containers.is_empty(),
        "unlinked texture rendering has no linked reference containers"
    );

    let mut surveys = Vec::with_capacity(inputs.linked_containers.len() + 1);
    surveys.push(survey_cpx_container(
        &inputs.container_iso_path,
        inputs.container,
    )?);
    for linked in &inputs.linked_containers {
        ensure!(
            sha256(linked.bytes) == linked.sha256,
            "linked texture container differs from the verified inventory hash: {}",
            linked.iso_path
        );
        surveys.push(survey_cpx_container(&linked.iso_path, linked.bytes)?);
    }
    let file_lists = link_image_file_lists(inputs.executable, &mut surveys)?;
    ensure!(
        file_lists.unlinked_containers == [inputs.container_iso_path.clone()],
        "the declared texture container is not the sole unlinked CPX container"
    );
    let unlinked_survey = surveys
        .first()
        .context("unlinked texture survey disappeared")?;
    ensure!(
        unlinked_survey.file_list.is_none(),
        "unlinked texture container unexpectedly acquired an ELF file list"
    );
    let trailing = unlinked_survey
        .trailing_region
        .as_ref()
        .context("unlinked texture container has no palette tail")?;
    let expected_trailing_bytes = unlinked_survey
        .blocks
        .len()
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("palette tail length overflow")?;
    ensure!(
        usize::try_from(trailing.byte_length)? == expected_trailing_bytes,
        "palette tail does not contain one sector per CPX image"
    );

    let executable = parse_executable(inputs.executable)?;
    let reference_pairs =
        collect_reference_pairs(&executable, &inputs.linked_containers, &surveys[1..])?;
    let dimension_evidence = collect_known_dimensions(&surveys[1..])?;

    let trailing_offset = usize::try_from(trailing.offset)?;
    let mut images = Vec::with_capacity(unlinked_survey.blocks.len());
    for block in &unlinked_survey.blocks {
        let palette_offset = trailing_offset
            .checked_add(
                block
                    .index
                    .checked_mul(CPX_BLOCK_ALIGNMENT)
                    .context("palette slot offset overflow")?,
            )
            .context("palette tail offset overflow")?;
        let palette_slot = inputs
            .container
            .get(palette_offset..palette_offset + CPX_BLOCK_ALIGNMENT)
            .context("palette slot exceeds unlinked texture container")?;
        let palette = split_palette_slot(palette_slot)?;
        let mut uploaded_palette = palette.to_vec();
        uploaded_palette[3] = 0;
        let palette_sha256 = sha256(palette);
        let uploaded_palette_sha256 = sha256(&uploaded_palette);
        let (dimension_source, dimensions) = dimension_candidates(
            block.header_value,
            &dimension_evidence.by_pixel_count,
            &dimension_evidence.widths,
        )?;
        let image_record = FileRecord {
            sector_offset: u32::try_from(block.offset / u64::try_from(CPX_BLOCK_ALIGNMENT)?)?,
            sector_count: u32::try_from(
                block.slot_byte_length / u64::try_from(CPX_BLOCK_ALIGNMENT)?,
            )?,
            stored_byte_length: u32::try_from(
                block
                    .encoded_byte_length
                    .checked_sub(1)
                    .context("CPX encoded length has no terminator")?,
            )?,
            width: 0,
            height: 0,
        };
        let palette_record = FileRecord {
            sector_offset: u32::try_from(palette_offset / CPX_BLOCK_ALIGNMENT)?,
            sector_count: 1,
            stored_byte_length: u32::try_from(PALETTE_BYTES)?,
            width: 0,
            height: 0,
        };
        let exact_linked_pair_matches = reference_pairs
            .iter()
            .filter(|pair| {
                pair.decoded_sha256 == block.decoded_sha256
                    && pair.uploaded_palette_sha256 == uploaded_palette_sha256
            })
            .map(|pair| pair.identity.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let linked_palette_matches = reference_pairs
            .iter()
            .filter(|pair| pair.uploaded_palette_sha256 == uploaded_palette_sha256)
            .map(|pair| pair.identity.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut previews = Vec::with_capacity(dimensions.len());
        for (width, height) in dimensions {
            let output_path = output_directory.join(format!(
                "unlinked-image-{:03}-{width}x{height}.png",
                block.index
            ));
            let rendered = render_indexed_texture(
                inputs.container,
                FileRecord {
                    width,
                    height,
                    ..image_record
                },
                palette_record,
                true,
                &output_path,
            )?;
            previews.push(UnlinkedTexturePreview {
                width: rendered.width,
                height: rendered.height,
                decoded_trailing_byte_length: rendered.decoded_trailing_byte_length,
                swizzled_indices_sha256: rendered.swizzled_indices_sha256,
                linear_indices_sha256: rendered.linear_indices_sha256,
                rgba_sha256: rendered.rgba_sha256,
                png_path: rendered.png_path,
                png_sha256: rendered.png_sha256,
            });
        }
        images.push(UnlinkedTextureImage {
            image_index: block.index,
            image_offset: block.offset,
            image_slot_byte_length: block.slot_byte_length,
            encoded_byte_length: block.encoded_byte_length,
            decoded_byte_length: block.decoded_byte_length,
            decoded_sha256: block.decoded_sha256.clone(),
            palette_offset: u64::try_from(palette_offset)?,
            palette_sha256,
            uploaded_palette_sha256,
            dimension_source,
            exact_linked_pair_matches,
            linked_palette_matches,
            previews,
        });
    }

    let linked_container_sha256 = inputs
        .linked_containers
        .iter()
        .map(|linked| (linked.iso_path.clone(), linked.sha256.clone()))
        .collect();
    Ok(UnlinkedTextureCorpus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        container_iso_path: inputs.container_iso_path,
        container_sha256: inputs.container_sha256,
        linked_container_sha256,
        summary: UnlinkedTextureCorpusSummary {
            images: images.len(),
            palette_slots: images.len(),
            preview_candidates: images.iter().map(|image| image.previews.len()).sum(),
            images_with_one_dimension_candidate: images
                .iter()
                .filter(|image| image.previews.len() == 1)
                .count(),
            images_with_multiple_dimension_candidates: images
                .iter()
                .filter(|image| image.previews.len() > 1)
                .count(),
            images_with_exact_linked_pair_match: images
                .iter()
                .filter(|image| !image.exact_linked_pair_matches.is_empty())
                .count(),
            images_with_linked_palette_match: images
                .iter()
                .filter(|image| !image.linked_palette_matches.is_empty())
                .count(),
        },
        images,
        evidence_boundary: "Structural and visual census of a CPX container without an ELF file-list owner. Each CPX image is paired with the same-index RGBA32 palette sector and rendered under every dimension observed for the same pixel count in linked D/G/MD textures, or under dimensions inferred from their validated width repertoire when no exact pixel-count precedent exists. Multiple previews are unresolved dimension candidates. Exact linked pair matches prove duplicated bytes, not runtime reachability. The report does not establish a loader, registration, pattern consumer, or visible runtime path for the unlinked container.".to_string(),
    })
}

fn parse_executable(source: &[u8]) -> Result<object::File<'_>> {
    let file = object::File::parse(source).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips,
        "ELF architecture is not 32-bit MIPS"
    );
    ensure!(
        file.endianness() == Endianness::Little,
        "ELF is not little-endian"
    );
    Ok(file)
}

fn collect_reference_pairs(
    executable: &object::File<'_>,
    inputs: &[LinkedTextureContainerInput<'_>],
    surveys: &[crate::cpx::CpxContainerSurvey],
) -> Result<Vec<ReferencePair>> {
    ensure!(
        inputs.len() == surveys.len(),
        "linked texture inputs and surveys differ"
    );
    let mut pairs = Vec::new();
    for (input, survey) in inputs.iter().zip(surveys) {
        ensure!(
            survey.file_list.is_some(),
            "reference container has no linked file list: {}",
            input.iso_path
        );
        let (_, bytes) = read_symbol_bytes(executable, &input.file_list_symbol)?;
        let records = parse_file_records(&bytes)?;
        let image_records = records
            .iter()
            .filter(|record| record.width > 0 && record.height > 0)
            .count();
        ensure!(
            image_records == survey.blocks.len(),
            "reference image records and CPX blocks differ: {}",
            input.file_list_symbol
        );
        if records.len() != image_records * 2 {
            continue;
        }
        for (block, image_record) in survey.blocks.iter().zip(&records[..image_records]) {
            let image = block
                .image_record
                .as_ref()
                .context("linked CPX block lost its file-list record")?;
            ensure!(
                image.file_list_record == block.index,
                "linked CPX image order differs from its file list"
            );
            let palette_file_record = block.index + image_records;
            let palette_record = records[palette_file_record];
            let decoded = read_decoded_image(input.bytes, *image_record)?;
            let uploaded_palette = read_palette(input.bytes, palette_record, true)?;
            pairs.push(ReferencePair {
                identity: LinkedTexturePairMatch {
                    container_iso_path: input.iso_path.clone(),
                    image_file_record: block.index,
                    palette_file_record,
                    width: image_record.width,
                    height: image_record.height,
                },
                decoded_sha256: sha256(&decoded),
                uploaded_palette_sha256: sha256(&uploaded_palette),
            });
        }
    }
    Ok(pairs)
}

fn collect_known_dimensions(
    surveys: &[crate::cpx::CpxContainerSurvey],
) -> Result<LinkedDimensionEvidence> {
    let mut dimensions = BTreeMap::<u32, BTreeSet<(u32, u32)>>::new();
    let mut widths = BTreeSet::new();
    for survey in surveys {
        for block in &survey.blocks {
            let image = block
                .image_record
                .as_ref()
                .context("linked CPX block lost its file-list dimensions")?;
            dimensions
                .entry(u32::try_from(image.image_byte_length)?)
                .or_default()
                .insert((image.width, image.height));
            widths.insert(image.width);
        }
    }
    Ok(LinkedDimensionEvidence {
        by_pixel_count: dimensions,
        widths,
    })
}

fn split_palette_slot(slot: &[u8]) -> Result<&[u8]> {
    ensure!(
        slot.len() == CPX_BLOCK_ALIGNMENT,
        "unlinked palette slot is not one sector"
    );
    let (palette, padding) = slot.split_at(PALETTE_BYTES);
    ensure!(
        padding.iter().all(|byte| *byte == 0),
        "unlinked palette slot has nonzero trailing padding"
    );
    Ok(palette)
}

fn dimension_candidates(
    pixel_count: u32,
    known_dimensions: &BTreeMap<u32, BTreeSet<(u32, u32)>>,
    known_widths: &BTreeSet<u32>,
) -> Result<(DimensionCandidateSource, BTreeSet<(u32, u32)>)> {
    if let Some(dimensions) = known_dimensions.get(&pixel_count) {
        ensure!(
            !dimensions.is_empty(),
            "linked dimension precedent is empty"
        );
        return Ok((
            DimensionCandidateSource::LinkedPixelCount,
            dimensions.clone(),
        ));
    }
    let dimensions = known_widths
        .iter()
        .filter_map(|width| {
            if !pixel_count.is_multiple_of(*width) {
                return None;
            }
            let height = pixel_count / width;
            (height > 0 && height <= 1_024 && height.is_multiple_of(4)).then_some((*width, height))
        })
        .collect::<BTreeSet<_>>();
    ensure!(
        !dimensions.is_empty(),
        "unlinked image has no dimension candidate from linked widths: {pixel_count}"
    );
    Ok((DimensionCandidateSource::LinkedWidthInference, dimensions))
}

#[cfg(test)]
#[path = "unlinked_texture_corpus/unlinked_texture_corpus_tests.rs"]
mod unlinked_texture_corpus_tests;
