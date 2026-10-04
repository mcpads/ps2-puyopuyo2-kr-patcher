use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::product_scope::ProductScope;

mod consumer;
pub mod pss;

use consumer::survey_movie_consumer;
use pss::survey_pss_stream;

#[derive(Clone, Debug)]
pub struct MovieCorpusInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
    pub movies: Vec<MovieInput<'a>>,
}

#[derive(Clone, Debug)]
pub struct MovieInput<'a> {
    pub iso_path: String,
    pub sha256: String,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieLanguageCatalog {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub review_basis: String,
    pub movies: Vec<MovieLanguageEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieLanguageEntry {
    pub iso_path: String,
    pub product_scope: ProductScope,
    pub classification: MovieLanguageClassification,
    pub japanese_evidence_frame: usize,
    pub english_evidence_frame: usize,
    pub note: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MovieLanguageClassification {
    JapaneseOnly,
    JapaneseEnglishMixed,
    EnglishOnly,
    NonText,
    Unresolved,
}

#[derive(Clone, Debug, Serialize)]
pub struct MovieCorpus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub consumer: MovieConsumerEvidence,
    pub review_basis: String,
    pub summary: MovieCorpusSummary,
    pub movies: Vec<MovieEvidence>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MovieCorpusSummary {
    pub movies: usize,
    pub included_movies: usize,
    pub excluded_other_title_promotion_movies: usize,
    pub program_stream_packets: usize,
    pub video_pes_packets: usize,
    pub private_stream_pes_packets: usize,
    pub video_frames: usize,
    pub intra_frames: usize,
    pub predictive_frames: usize,
    pub bidirectionally_predicted_frames: usize,
    pub japanese_target_movies: usize,
    pub japanese_only_movies: usize,
    pub japanese_english_mixed_movies: usize,
    pub english_only_movies: usize,
    pub non_text_movies: usize,
    pub unresolved_movies: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MovieEvidence {
    pub iso_path: String,
    pub sha256: String,
    pub byte_length: usize,
    pub stream: PssStreamEvidence,
    pub product_scope: ProductScope,
    pub classification: MovieLanguageClassification,
    pub translation_target: bool,
    pub requires_english_pixel_preservation: bool,
    pub japanese_evidence_frame: usize,
    pub english_evidence_frame: usize,
    pub review_note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PssStreamEvidence {
    pub pack_headers: usize,
    pub system_headers: usize,
    pub program_end_codes: usize,
    pub video_stream_id: String,
    pub video_pes_packets: usize,
    pub private_stream_pes_packets: usize,
    pub audio_pes_packets: usize,
    pub padding_packets: usize,
    pub elementary_stream_bytes: usize,
    pub elementary_stream_sha256: String,
    pub sequence_headers: usize,
    pub width: u16,
    pub height: u16,
    pub frame_rate_numerator: u32,
    pub frame_rate_denominator: u32,
    pub frames: usize,
    pub intra_frames: usize,
    pub predictive_frames: usize,
    pub bidirectionally_predicted_frames: usize,
    pub other_picture_types: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MovieConsumerEvidence {
    pub table_symbol: String,
    pub table_address: String,
    pub entries: Vec<MovieTableEntry>,
    pub owner_symbol: String,
    pub player_symbol: String,
    pub instructions: Vec<crate::typed_evidence::TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MovieTableEntry {
    pub index: usize,
    pub path_symbol: String,
    pub path_address: String,
    pub iso_path: String,
}

pub fn survey_movie_corpus(
    inputs: MovieCorpusInputs<'_>,
    catalog: &MovieLanguageCatalog,
) -> Result<MovieCorpus> {
    validate_catalog_identity(&inputs, catalog)?;
    ensure!(
        !catalog.review_basis.trim().is_empty(),
        "movie language review basis is empty"
    );
    ensure!(!inputs.movies.is_empty(), "movie corpus is empty");

    let movie_paths = inputs
        .movies
        .iter()
        .map(|movie| movie.iso_path.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        movie_paths.len() == inputs.movies.len(),
        "movie corpus has duplicate ISO paths"
    );
    let catalog_by_path = catalog
        .movies
        .iter()
        .map(|entry| (entry.iso_path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        catalog_by_path.len() == catalog.movies.len(),
        "movie language catalog has duplicate ISO paths"
    );
    ensure!(
        catalog_by_path.keys().copied().collect::<BTreeSet<_>>()
            == movie_paths.iter().map(String::as_str).collect(),
        "movie language catalog does not exactly partition the movie corpus"
    );

    let consumer = survey_movie_consumer(inputs.executable, &movie_paths)?;
    let mut summary = MovieCorpusSummary::default();
    let mut movies = Vec::with_capacity(inputs.movies.len());
    for input in inputs.movies {
        let stream = survey_pss_stream(input.bytes)
            .with_context(|| format!("failed to survey {}", input.iso_path))?;
        let language = catalog_by_path[&input.iso_path.as_str()];
        ensure!(
            language.japanese_evidence_frame < stream.frames,
            "Japanese evidence frame is outside {}",
            input.iso_path
        );
        ensure!(
            language.english_evidence_frame < stream.frames,
            "English evidence frame is outside {}",
            input.iso_path
        );
        ensure!(
            !language.note.trim().is_empty(),
            "movie language note is empty for {}",
            input.iso_path
        );
        let (translation_target, preserve_english) =
            language_scope(language.classification, language.product_scope);
        summary.movies += 1;
        match language.product_scope {
            ProductScope::Included => summary.included_movies += 1,
            ProductScope::ExcludedOtherTitlePromotion => {
                summary.excluded_other_title_promotion_movies += 1;
            }
        }
        summary.program_stream_packets += stream.pack_headers
            + stream.system_headers
            + stream.video_pes_packets
            + stream.private_stream_pes_packets
            + stream.audio_pes_packets
            + stream.padding_packets
            + stream.program_end_codes;
        summary.video_pes_packets += stream.video_pes_packets;
        summary.private_stream_pes_packets += stream.private_stream_pes_packets;
        summary.video_frames += stream.frames;
        summary.intra_frames += stream.intra_frames;
        summary.predictive_frames += stream.predictive_frames;
        summary.bidirectionally_predicted_frames += stream.bidirectionally_predicted_frames;
        if translation_target {
            summary.japanese_target_movies += 1;
        }
        match language.classification {
            MovieLanguageClassification::JapaneseOnly => summary.japanese_only_movies += 1,
            MovieLanguageClassification::JapaneseEnglishMixed => {
                summary.japanese_english_mixed_movies += 1;
            }
            MovieLanguageClassification::EnglishOnly => summary.english_only_movies += 1,
            MovieLanguageClassification::NonText => summary.non_text_movies += 1,
            MovieLanguageClassification::Unresolved => summary.unresolved_movies += 1,
        }
        movies.push(MovieEvidence {
            iso_path: input.iso_path,
            sha256: input.sha256,
            byte_length: input.bytes.len(),
            stream,
            product_scope: language.product_scope,
            classification: language.classification,
            translation_target,
            requires_english_pixel_preservation: preserve_english,
            japanese_evidence_frame: language.japanese_evidence_frame,
            english_evidence_frame: language.english_evidence_frame,
            review_note: language.note.clone(),
        });
    }
    ensure!(
        summary.japanese_only_movies
            + summary.japanese_english_mixed_movies
            + summary.english_only_movies
            + summary.non_text_movies
            + summary.unresolved_movies
            == summary.movies,
        "movie language summary does not partition the corpus"
    );
    ensure!(
        summary.included_movies + summary.excluded_other_title_promotion_movies == summary.movies,
        "movie product scope does not partition the corpus"
    );
    ensure!(
        summary.unresolved_movies == 0,
        "movie language survey still has unresolved files"
    );
    ensure!(
        consumer.entries.len() == summary.movies,
        "movie consumer table does not cover the movie corpus"
    );

    Ok(MovieCorpus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        consumer,
        review_basis: catalog.review_basis.clone(),
        summary,
        movies,
        evidence_boundary: "Every MPEG-2 picture header in every exact-source PSS file is enumerated and every file is linked through MovieFile_Tbl to MovieVideo_Run and Movie_Play. Language is a human-reviewed whole-file classification based on full-duration one-second contact sheets; it does not assert that every individual frame contains text. Product scope is explicit per movie: other-title promotion remains a surveyed source asset but is not a translation target, and every original PSS byte is preserved.",
    })
}

fn validate_catalog_identity(
    inputs: &MovieCorpusInputs<'_>,
    catalog: &MovieLanguageCatalog,
) -> Result<()> {
    ensure!(catalog.schema == 1, "unsupported movie catalog schema");
    ensure!(
        catalog.source_bin_sha256 == inputs.source_bin_sha256,
        "movie catalog source BIN differs"
    );
    ensure!(
        catalog.executable_iso_path == inputs.executable_iso_path,
        "movie catalog executable path differs"
    );
    ensure!(
        catalog.executable_sha256 == inputs.executable_sha256,
        "movie catalog executable hash differs"
    );
    Ok(())
}

fn language_scope(
    classification: MovieLanguageClassification,
    product_scope: ProductScope,
) -> (bool, bool) {
    let preserve_english = matches!(
        classification,
        MovieLanguageClassification::JapaneseEnglishMixed
            | MovieLanguageClassification::EnglishOnly
    );
    if !product_scope.is_included() {
        return (false, preserve_english);
    }
    match classification {
        MovieLanguageClassification::JapaneseOnly => (true, false),
        MovieLanguageClassification::JapaneseEnglishMixed => (true, true),
        MovieLanguageClassification::EnglishOnly => (false, true),
        MovieLanguageClassification::NonText | MovieLanguageClassification::Unresolved => {
            (false, false)
        }
    }
}

#[cfg(test)]
#[path = "movie_corpus/movie_corpus_tests.rs"]
mod movie_corpus_tests;
