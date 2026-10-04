use super::validation::validate_authoring_for_test;
use super::{
    SiblingStoryMatchKind, SiblingStoryReference, StoryDialogueAuthoring, StoryDialogueSegment,
    StoryDialogueStatus,
};

fn draft() -> StoryDialogueAuthoring {
    StoryDialogueAuthoring {
        status: StoryDialogueStatus::NeedsReview,
        scene_id: Some("will_o_wisp".to_string()),
        scene_variant: Some("primary".to_string()),
        segments: vec![StoryDialogueSegment {
            segment_id: "demo-00/001".to_string(),
            speaker_id: "arle".to_string(),
            source_text: "ずいぶん気持ちよさそうだね。".to_string(),
            korean_text: "정말 기분 좋아 보이네.".to_string(),
            sibling_references: vec![SiblingStoryReference {
                stable_id: "story_17_9E17".to_string(),
                match_kind: SiblingStoryMatchKind::NearExpandedMatch,
                note: "PS2는 종조사와 조판이 다르다.".to_string(),
            }],
            translator_notes: Vec::new(),
        }],
        transcription_notes: Vec::new(),
        transcription_review: None,
        human_approval: None,
        independent_review: None,
    }
}

#[test]
fn accepts_a_direct_draft_with_a_sibling_reference() {
    validate_authoring_for_test("story-dialogue/demo-00", &draft()).unwrap();
}

#[test]
fn rejects_japanese_left_in_the_korean_draft() {
    let mut authoring = draft();
    authoring.segments[0].korean_text = "정말 気持ち 좋아 보이네.".to_string();
    let error = validate_authoring_for_test("story-dialogue/demo-00", &authoring).unwrap_err();
    assert!(error.to_string().contains("still contains Japanese"));
}

#[test]
fn rejects_layout_that_would_shift_later_dialogue_rows() {
    let mut authoring = draft();
    authoring.segments[0].source_text = "重そうだね。\n手伝ってあげるよ！".to_string();
    authoring.segments[0].korean_text = "무거워 보여. 도와줄게!".to_string();
    authoring.segments.push(StoryDialogueSegment {
        segment_id: "demo-00/002".to_string(),
        speaker_id: "arle".to_string(),
        source_text: "次の台詞。".to_string(),
        korean_text: "다음 대사.".to_string(),
        sibling_references: Vec::new(),
        translator_notes: Vec::new(),
    });

    let error = validate_authoring_for_test("story-dialogue/demo-00", &authoring).unwrap_err();
    assert!(error.to_string().contains("line slot count differs"));
}

#[test]
fn accepts_more_korean_characters_when_the_physical_row_count_is_stable() {
    let mut authoring = draft();
    authoring.segments[0].source_text = "短い\n余裕のある行".to_string();
    authoring.segments[0].korean_text = "첫 줄이 넘쳐\n짧아".to_string();

    validate_authoring_for_test("story-dialogue/demo-00", &authoring).unwrap();
}

#[test]
fn rejects_human_review_without_transcription_review() {
    let mut authoring = draft();
    authoring.status = StoryDialogueStatus::NeedsHumanReview;
    let error = validate_authoring_for_test("story-dialogue/demo-00", &authoring).unwrap_err();
    assert!(error.to_string().contains("lacks verified transcription"));
}

#[test]
fn rejects_complete_before_visual_and_runtime_gates_exist() {
    let mut authoring = draft();
    authoring.status = StoryDialogueStatus::Complete;
    let error = validate_authoring_for_test("story-dialogue/demo-00", &authoring).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("complete story status is unavailable")
    );
}
