use super::*;

fn solid(width: usize, height: usize, pixel: [u8; 4]) -> RgbaImage {
    RgbaImage {
        width,
        height,
        pixels: pixel.repeat(width * height),
    }
}

#[test]
fn draft_changes_only_the_declared_japanese_logo_bounds() {
    let mut source = solid(768, 256, [1, 2, 3, 255]);
    let protection = vec![false; source.width * source.height];
    fill(&mut source, EDITABLE_BOUNDS, [0, 0, 0, 0], &protection).unwrap();
    let master = solid(40, 10, [240, 40, 10, 255]);
    let japanese = vec![true; source.width * source.height];
    let output = compose(
        &source,
        &master,
        PixelBounds {
            x: 0,
            y: 0,
            width: 40,
            height: 10,
        },
        &japanese,
        &protection,
    )
    .unwrap();

    let (changed, outside) = changed_pixel_counts(&source, &output).unwrap();
    assert!(changed > 0);
    assert_eq!(outside, 0);
    assert_eq!(&output.pixels[..4], &[1, 2, 3, 255]);
}

#[test]
fn alpha_bounds_rejects_an_empty_master() {
    let master = solid(4, 4, [0, 0, 0, 0]);
    assert!(alpha_bounds(&master).is_err());
}

#[test]
fn lettering_placement_stays_inside_the_editable_bounds() {
    assert!(contains(EDITABLE_BOUNDS, LETTERING_PLACEMENT));
    let fitted = fit_inside(
        PixelBounds {
            x: 0,
            y: 0,
            width: 1600,
            height: 400,
        },
        LETTERING_PLACEMENT,
    )
    .unwrap();
    assert!(contains(LETTERING_PLACEMENT, fitted));
}

#[test]
fn renderer_creates_a_missing_output_directory() {
    let temporary = tempfile::tempdir().unwrap();
    let source_path = temporary.path().join("source.png");
    let generated_path = temporary.path().join("generated.png");
    let master_path = temporary.path().join("master.png");
    let output_path = temporary.path().join("missing/title.png");
    let mut source = solid(768, 256, [0, 0, 0, 0]);
    source.pixels[(10 * source.width + 10) * 4..(10 * source.width + 10) * 4 + 4]
        .copy_from_slice(&[1, 2, 3, 255]);
    source.pixels[(20 * source.width + 20) * 4..(20 * source.width + 20) * 4 + 4]
        .copy_from_slice(&[4, 5, 6, 255]);
    let master = solid(40, 10, [240, 40, 10, 255]);
    write_rgba_png(&source_path, 768, 256, &source.pixels).unwrap();
    write_rgba_png(&generated_path, 40, 10, &master.pixels).unwrap();
    write_rgba_png(&master_path, 40, 10, &master.pixels).unwrap();
    let generated_bytes = fs::read(&generated_path).unwrap();
    let master_bytes = fs::read(&master_path).unwrap();
    let (components, labels) = visible_components(&source).unwrap();
    let japanese = select_component_mask(
        &components,
        &labels,
        &["visible-000".to_string()],
        source.width,
        source.height,
        "Japanese source",
    )
    .unwrap();
    let protected = select_component_mask(
        &components,
        &labels,
        &["visible-001".to_string()],
        source.width,
        source.height,
        "protected",
    )
    .unwrap();
    let declared_japanese = temporary.path().join("declared/japanese.png");
    let declared_nontext = temporary.path().join("declared/nontext.png");
    let declared_english = temporary.path().join("declared/english.png");
    write_mask_png(&declared_japanese, source.width, source.height, &japanese).unwrap();
    write_mask_png(&declared_nontext, source.width, source.height, &protected).unwrap();
    write_mask_png(&declared_english, source.width, source.height, &protected).unwrap();
    let asset = TitleDraftAsset {
        schema: 1,
        candidate_id: "candidate".to_string(),
        expected_text: "뿌요뿌요".to_string(),
        use_case: "logo-brand".to_string(),
        tool_path: "test".to_string(),
        generation_prompt: "test".to_string(),
        generated_source: HashedAsset {
            path: generated_path.to_string_lossy().into_owned(),
            sha256: sha256(&generated_bytes),
        },
        transparent_master: HashedAsset {
            path: master_path.to_string_lossy().into_owned(),
            sha256: sha256(&master_bytes),
        },
        japanese_source_components: vec!["visible-000".to_string()],
        protected_nontext_components: vec!["visible-001".to_string()],
        protected_english_components: vec!["visible-001".to_string()],
        japanese_source_mask: hashed_output_asset(&declared_japanese).unwrap(),
        protected_nontext_mask: hashed_output_asset(&declared_nontext).unwrap(),
        protected_english_mask: hashed_output_asset(&declared_english).unwrap(),
        review_status: "pre_human_review".to_string(),
        evidence_boundary: "test".to_string(),
    };

    let japanese_mask = temporary.path().join("missing/japanese.png");
    let nontext_mask = temporary.path().join("missing/nontext.png");
    let english_mask = temporary.path().join("missing/english.png");
    render_title_translation_draft(
        &source_path,
        &asset,
        &output_path,
        &japanese_mask,
        &nontext_mask,
        &english_mask,
    )
    .unwrap();
    assert!(output_path.is_file());
    assert!(japanese_mask.is_file());
}
