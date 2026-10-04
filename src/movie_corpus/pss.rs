use std::collections::BTreeSet;
use std::ops::Range;

use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};

use super::PssStreamEvidence;

const START_CODE_PREFIX: [u8; 3] = [0, 0, 1];
const PACK_HEADER: u8 = 0xba;
const SYSTEM_HEADER: u8 = 0xbb;
const PROGRAM_END: u8 = 0xb9;
const PRIVATE_STREAM_1: u8 = 0xbd;
const PADDING_STREAM: u8 = 0xbe;

#[derive(Clone, Debug)]
struct PssChunk {
    range: Range<usize>,
    stream_id: Option<u8>,
    video_payload: Option<Range<usize>>,
}

pub(crate) fn survey_pss_stream(source: &[u8]) -> Result<PssStreamEvidence> {
    ensure!(source.len() >= 4, "PSS stream is too short");
    let mut pack_headers = 0;
    let mut system_headers = 0;
    let mut program_end_codes = 0;
    let mut video_stream_ids = BTreeSet::new();
    let mut video_pes_packets = 0;
    let mut private_stream_pes_packets = 0;
    let mut audio_pes_packets = 0;
    let mut padding_packets = 0;
    let mut elementary_stream = Vec::new();

    for chunk in parse_pss_chunks(source)? {
        let Some(stream_id) = chunk.stream_id else {
            continue;
        };
        match stream_id {
            PACK_HEADER => pack_headers += 1,
            PROGRAM_END => program_end_codes += 1,
            _ => match stream_id {
                SYSTEM_HEADER => system_headers += 1,
                PRIVATE_STREAM_1 => private_stream_pes_packets += 1,
                PADDING_STREAM => padding_packets += 1,
                0xc0..=0xdf => audio_pes_packets += 1,
                0xe0..=0xef => {
                    video_stream_ids.insert(stream_id);
                    video_pes_packets += 1;
                    let payload = chunk.video_payload.with_context(|| {
                        format!("video packet has no payload at {:#x}", chunk.range.start)
                    })?;
                    elementary_stream.extend_from_slice(&source[payload]);
                }
                _ => bail!(
                    "unsupported PSS packet id {stream_id:#04x} at {:#x}",
                    chunk.range.start
                ),
            },
        }
    }
    ensure!(pack_headers > 0, "PSS has no pack header");
    ensure!(program_end_codes == 1, "PSS must have one program end code");
    ensure!(
        video_stream_ids.len() == 1,
        "PSS must have exactly one video stream"
    );
    ensure!(video_pes_packets > 0, "PSS has no video PES packets");
    let stream_id = *video_stream_ids
        .first()
        .context("video stream id missing")?;
    let pictures = survey_elementary_stream(&elementary_stream)?;

    Ok(PssStreamEvidence {
        pack_headers,
        system_headers,
        program_end_codes,
        video_stream_id: format!("{stream_id:#04x}"),
        video_pes_packets,
        private_stream_pes_packets,
        audio_pes_packets,
        padding_packets,
        elementary_stream_bytes: elementary_stream.len(),
        elementary_stream_sha256: format!("{:x}", Sha256::digest(&elementary_stream)),
        sequence_headers: pictures.sequence_headers,
        width: pictures.width,
        height: pictures.height,
        frame_rate_numerator: pictures.frame_rate_numerator,
        frame_rate_denominator: pictures.frame_rate_denominator,
        frames: pictures.frames,
        intra_frames: pictures.intra_frames,
        predictive_frames: pictures.predictive_frames,
        bidirectionally_predicted_frames: pictures.bidirectionally_predicted_frames,
        other_picture_types: pictures.other_picture_types,
    })
}

fn parse_pss_chunks(source: &[u8]) -> Result<Vec<PssChunk>> {
    ensure!(source.len() >= 4, "PSS stream is too short");
    let mut cursor = 0_usize;
    let mut chunks = Vec::new();
    while cursor < source.len() {
        if source[cursor] == 0xff {
            chunks.push(PssChunk {
                range: cursor..cursor + 1,
                stream_id: None,
                video_payload: None,
            });
            cursor += 1;
            continue;
        }
        let prefix = source
            .get(cursor..cursor + 4)
            .with_context(|| format!("truncated PSS start code at {cursor:#x}"))?;
        ensure!(
            prefix[..3] == START_CODE_PREFIX,
            "PSS packet does not start with a start code at {cursor:#x}"
        );
        let stream_id = prefix[3];
        let (end, video_payload) = match stream_id {
            PACK_HEADER => {
                let header = source
                    .get(cursor..cursor + 14)
                    .with_context(|| format!("truncated pack header at {cursor:#x}"))?;
                ensure!(
                    header[4] & 0xc0 == 0x40,
                    "PSS pack header is not MPEG-2 at {cursor:#x}"
                );
                let end = cursor + 14 + usize::from(header[13] & 7);
                ensure!(
                    end <= source.len(),
                    "pack header exceeds source at {cursor:#x}"
                );
                (end, None)
            }
            PROGRAM_END => (cursor + 4, None),
            _ => {
                let packet_length = read_packet_length(source, cursor)?;
                let end = cursor
                    .checked_add(6 + packet_length)
                    .context("PSS packet range overflow")?;
                let packet = source
                    .get(cursor..end)
                    .with_context(|| format!("PSS packet exceeds source at {cursor:#x}"))?;
                let video_payload = if matches!(stream_id, 0xe0..=0xef) {
                    let payload_offset = video_pes_payload_offset(packet, cursor)?;
                    Some(cursor + payload_offset..end)
                } else {
                    None
                };
                (end, video_payload)
            }
        };
        chunks.push(PssChunk {
            range: cursor..end,
            stream_id: Some(stream_id),
            video_payload,
        });
        cursor = end;
    }
    Ok(chunks)
}

fn read_packet_length(source: &[u8], cursor: usize) -> Result<usize> {
    let bytes = source
        .get(cursor + 4..cursor + 6)
        .with_context(|| format!("truncated PSS packet length at {cursor:#x}"))?;
    Ok(usize::from(u16::from_be_bytes([bytes[0], bytes[1]])))
}

fn video_pes_payload_offset(packet: &[u8], packet_offset: usize) -> Result<usize> {
    let header = packet
        .get(6..9)
        .with_context(|| format!("truncated video PES header at {packet_offset:#x}"))?;
    ensure!(
        header[0] & 0xc0 == 0x80,
        "video PES header is not MPEG-2 at {packet_offset:#x}"
    );
    let payload_offset = 9 + usize::from(header[2]);
    ensure!(
        payload_offset <= packet.len(),
        "video PES header exceeds packet at {packet_offset:#x}"
    );
    Ok(payload_offset)
}

struct PictureSurvey {
    sequence_headers: usize,
    width: u16,
    height: u16,
    frame_rate_numerator: u32,
    frame_rate_denominator: u32,
    frames: usize,
    intra_frames: usize,
    predictive_frames: usize,
    bidirectionally_predicted_frames: usize,
    other_picture_types: usize,
}

fn survey_elementary_stream(source: &[u8]) -> Result<PictureSurvey> {
    let mut dimensions = BTreeSet::new();
    let mut frame_rates = BTreeSet::new();
    let mut sequence_headers = 0;
    let mut frames = 0;
    let mut intra_frames = 0;
    let mut predictive_frames = 0;
    let mut bidirectionally_predicted_frames = 0;
    let mut other_picture_types = 0;

    for offset in 0..source.len().saturating_sub(3) {
        if source[offset..offset + 3] != START_CODE_PREFIX {
            continue;
        }
        match source[offset + 3] {
            0x00 => {
                let header = source
                    .get(offset + 4..offset + 6)
                    .with_context(|| format!("truncated picture header at {offset:#x}"))?;
                frames += 1;
                match (header[1] >> 3) & 7 {
                    1 => intra_frames += 1,
                    2 => predictive_frames += 1,
                    3 => bidirectionally_predicted_frames += 1,
                    _ => other_picture_types += 1,
                }
            }
            0xb3 => {
                let header = source
                    .get(offset + 4..offset + 8)
                    .with_context(|| format!("truncated sequence header at {offset:#x}"))?;
                let width = (u16::from(header[0]) << 4) | u16::from(header[1] >> 4);
                let height = (u16::from(header[1] & 0x0f) << 8) | u16::from(header[2]);
                let frame_rate = frame_rate(header[3] & 0x0f)?;
                dimensions.insert((width, height));
                frame_rates.insert(frame_rate);
                sequence_headers += 1;
            }
            _ => {}
        }
    }
    ensure!(
        sequence_headers > 0,
        "video elementary stream has no sequence header"
    );
    ensure!(frames > 0, "video elementary stream has no picture headers");
    ensure!(
        dimensions.len() == 1,
        "video elementary stream changes dimensions"
    );
    ensure!(
        frame_rates.len() == 1,
        "video elementary stream changes frame rate"
    );
    ensure!(
        intra_frames + predictive_frames + bidirectionally_predicted_frames + other_picture_types
            == frames,
        "picture coding types do not partition frames"
    );
    let (width, height) = *dimensions.first().context("video dimensions missing")?;
    let (frame_rate_numerator, frame_rate_denominator) =
        *frame_rates.first().context("video frame rate missing")?;
    Ok(PictureSurvey {
        sequence_headers,
        width,
        height,
        frame_rate_numerator,
        frame_rate_denominator,
        frames,
        intra_frames,
        predictive_frames,
        bidirectionally_predicted_frames,
        other_picture_types,
    })
}

fn frame_rate(code: u8) -> Result<(u32, u32)> {
    match code {
        1 => Ok((24_000, 1_001)),
        2 => Ok((24, 1)),
        3 => Ok((25, 1)),
        4 => Ok((30_000, 1_001)),
        5 => Ok((30, 1)),
        6 => Ok((50, 1)),
        7 => Ok((60_000, 1_001)),
        8 => Ok((60, 1)),
        _ => bail!("unsupported MPEG-2 frame-rate code {code}"),
    }
}

#[cfg(test)]
#[path = "pss_tests.rs"]
mod pss_tests;
