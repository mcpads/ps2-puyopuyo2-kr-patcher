use super::*;

#[test]
fn parses_ps2_boot_fields() {
    let parsed =
        parse_system_cnf(b"BOOT2 = cdrom0:\\SLPM_624.00;1\r\nVER = 1.01\r\nVMODE = NTSC\r\n")
            .unwrap();
    assert_eq!(parsed.boot2, "cdrom0:\\SLPM_624.00;1");
    assert_eq!(parsed.version, "1.01");
    assert_eq!(parsed.video_mode, "NTSC");
}

#[test]
fn rejects_duplicate_or_missing_boot_fields() {
    let duplicate = b"BOOT2=x\nBOOT2=y\nVER=1\nVMODE=NTSC\n";
    assert!(
        parse_system_cnf(duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );

    let missing = b"BOOT2=x\nVER=1\n";
    assert!(
        parse_system_cnf(missing)
            .unwrap_err()
            .to_string()
            .contains("VMODE")
    );
}
