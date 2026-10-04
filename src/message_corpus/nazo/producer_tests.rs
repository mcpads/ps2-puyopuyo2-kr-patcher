use std::collections::BTreeMap;

use super::*;

#[test]
fn producer_catalog_preserves_all_renderer_actor_creations() {
    let mut counts = BTreeMap::new();
    for site in EXPECTED_ACTOR_CREATIONS {
        *counts.entry(site.producer_symbol).or_insert(0_usize) += 1;
    }

    assert_eq!(counts.len(), PRODUCER_SYMBOLS.len());
    assert_eq!(counts["Make_Nazo_StartMenu"], 3);
    assert_eq!(counts["Make_YesNo_Window"], 1);
    assert_eq!(counts["Nazo_Edit_DataDisp"], 1);
    assert_eq!(counts["Make_Nazo_InfoAct"], 1);
    assert_eq!(counts["Nazo_Menu_Run"], 3);
    assert_eq!(EXPECTED_ACTOR_CREATIONS.len(), 9);
}
