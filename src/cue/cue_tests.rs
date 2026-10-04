use super::*;

#[test]
fn parses_single_track_mode2_source() {
    let cue = "FILE \"disc.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n";
    let parsed = CueSheet::parse(cue).unwrap();
    assert_eq!(parsed.data_file, "disc.bin");
    assert_eq!(parsed.track_number, 1);
    assert_eq!(parsed.track_mode, "MODE2/2352");
    assert_eq!(parsed.index_number, 1);
    assert_eq!(parsed.index_start, "00:00:00");
}

#[test]
fn rejects_layout_affecting_cue_records() {
    let cue = "FILE \"disc.bin\" BINARY\nTRACK 01 MODE2/2352\nPREGAP 00:02:00\nINDEX 01 00:00:00\n";
    let error = CueSheet::parse(cue).unwrap_err().to_string();
    assert!(error.contains("unsupported CUE line"));
}

#[test]
fn rejects_multiple_tracks() {
    let cue = "FILE \"disc.bin\" BINARY\nTRACK 01 MODE2/2352\nINDEX 01 00:00:00\nTRACK 02 AUDIO\n";
    let error = CueSheet::parse(cue).unwrap_err().to_string();
    assert!(error.contains("exactly one TRACK"));
}
