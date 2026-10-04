use super::*;

#[test]
fn approved_translation_has_twenty_four_stable_hangul_assignments() {
    let text = "MEMORY CARD 슬롯@P의 메모리 카드(PS2)\n를 확인 중입니다\n메모리 카드(PS2)를\n빼거나 꽂지 마십시오";

    assert_eq!(
        collect_unique_hangul([text]).unwrap(),
        [
            '슬', '롯', '의', '메', '모', '리', '카', '드', '를', '확', '인', '중', '입', '니',
            '다', '빼', '거', '나', '꽂', '지', '마', '십', '시', '오'
        ]
    );
}
