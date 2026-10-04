use super::*;

#[test]
fn declared_font_families_have_unique_roles_and_symbols() {
    let families = FONT_CONSUMERS
        .iter()
        .map(|spec| spec.family)
        .collect::<BTreeSet<_>>();
    let symbols = FONT_CONSUMERS
        .iter()
        .flat_map(|spec| {
            [
                spec.renderer_symbol,
                spec.lookup_symbol,
                spec.font_table_symbol,
            ]
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(families.len(), FONT_CONSUMERS.len());
    assert_eq!(symbols.len(), FONT_CONSUMERS.len() * 3);
}

#[test]
fn producer_count_comparison_exposes_missing_and_extra_renderer_references() {
    let references = [reference("declared"), reference("extra")];
    let counts = compare_renderer_producer_counts(
        &references,
        &BTreeMap::from([("declared".to_string(), 2)]),
    );

    assert_eq!(counts.len(), 2);
    assert!(counts.iter().all(|count| count.status == "mismatch"));
    assert_eq!(
        counts
            .iter()
            .map(|count| (
                count.producer_symbol.as_str(),
                count.declared_actor_creations,
                count.observed_static_references,
            ))
            .collect::<Vec<_>>(),
        vec![("declared", 2, 1), ("extra", 0, 1)]
    );
}

fn reference(owner_symbol: &str) -> StaticReference {
    StaticReference {
        owner_symbol: owner_symbol.to_string(),
        address: "0x00400000".to_string(),
        kind: StaticReferenceKind::TypedAddressMaterialization,
    }
}
