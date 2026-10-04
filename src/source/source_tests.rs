use tempfile::tempdir;

use super::*;

fn identity() -> FileIdentity {
    FileIdentity {
        filename: "disc.bin".to_string(),
        size: 4,
        sha256: "0".repeat(64),
    }
}

#[test]
fn rejects_source_size_before_content_is_admitted() {
    let actual = FileHash {
        byte_length: 3,
        sha256: "0".repeat(64),
    };
    let error = verify_file("BIN", &identity(), &actual)
        .unwrap_err()
        .to_string();
    assert!(error.contains("BIN size mismatch"));
}

#[test]
fn rejects_source_hash_even_when_size_matches() {
    let actual = FileHash {
        byte_length: 4,
        sha256: "1".repeat(64),
    };
    let error = verify_file("BIN", &identity(), &actual)
        .unwrap_err()
        .to_string();
    assert!(error.contains("BIN SHA-256 mismatch"));
}

#[test]
fn extraction_reuses_identical_files_and_refuses_different_content() {
    let directory = tempdir().unwrap();
    let output = safe_output_path(directory.path(), "/DATA/FILE.BIN;1").unwrap();
    let bytes = b"verified extraction";
    let sha256 = format!("{:x}", Sha256::digest(bytes));
    assert!(!write_new_or_verify(&output, bytes, &sha256).unwrap());
    assert!(write_new_or_verify(&output, bytes, &sha256).unwrap());

    std::fs::write(&output, b"different").unwrap();
    let error = write_new_or_verify(&output, bytes, &sha256)
        .unwrap_err()
        .to_string();
    assert!(error.contains("refusing to overwrite"));
}

#[test]
fn extraction_rejects_parent_traversal() {
    let error = safe_output_path(Path::new("out"), "/../escape.bin")
        .unwrap_err()
        .to_string();
    assert!(error.contains("unsafe ISO path"));
}

#[test]
fn translation_initialization_refuses_to_overwrite_existing_json() {
    let directory = tempdir().unwrap();
    let output = directory.path().join("translation.json");
    let original = serde_json::json!({"status": "untranslated"});
    write_new_json_file(&output, &original).unwrap();

    let replacement = serde_json::json!({"status": "complete"});
    let error = write_new_json_file(&output, &replacement)
        .unwrap_err()
        .to_string();
    assert!(error.contains("refusing to overwrite"));
    assert_eq!(
        read_json_file::<serde_json::Value>(&output).unwrap(),
        original
    );
}
