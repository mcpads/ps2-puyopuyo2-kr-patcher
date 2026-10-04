use super::*;

#[test]
fn surveys_mpeg2_picture_population_across_pes_packets() {
    let mut source = pack_header();
    source.extend(pes_packet(
        0xe0,
        &[
            0, 0, 1, 0xb3, 0x2d, 0x01, 0xe0, 0x34, 0, 0, 1, 0x00, 0, 0x08, 0, 0,
        ],
    ));
    source.extend(pes_packet(
        0xe0,
        &[1, 0x00, 0, 0x10, 0, 0, 1, 0x00, 0, 0x18],
    ));
    source.extend([0, 0, 1, PROGRAM_END]);

    let survey = survey_pss_stream(&source).unwrap();

    assert_eq!((survey.width, survey.height), (720, 480));
    assert_eq!(
        (survey.frame_rate_numerator, survey.frame_rate_denominator),
        (30_000, 1_001)
    );
    assert_eq!(survey.frames, 3);
    assert_eq!(survey.intra_frames, 1);
    assert_eq!(survey.predictive_frames, 1);
    assert_eq!(survey.bidirectionally_predicted_frames, 1);
    assert_eq!(survey.video_pes_packets, 2);
}

#[test]
fn rejects_picture_stream_without_a_sequence_header() {
    let mut source = pack_header();
    source.extend(pes_packet(0xe0, &[0, 0, 1, 0x00, 0, 0x08]));
    source.extend([0, 0, 1, PROGRAM_END]);

    let error = survey_pss_stream(&source).unwrap_err();

    assert!(error.to_string().contains("no sequence header"));
}
fn pack_header() -> Vec<u8> {
    vec![0, 0, 1, PACK_HEADER, 0x44, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

fn pes_packet(stream_id: u8, payload: &[u8]) -> Vec<u8> {
    let packet_length = u16::try_from(3 + payload.len()).unwrap();
    let mut packet = vec![0, 0, 1, stream_id];
    packet.extend(packet_length.to_be_bytes());
    packet.extend([0x80, 0, 0]);
    packet.extend(payload);
    packet
}
