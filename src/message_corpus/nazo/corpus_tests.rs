use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[test]
fn every_declared_table_entry_has_a_consumer_reference() {
    let mut references = BTreeMap::<&str, BTreeMap<usize, usize>>::new();
    for reference in INDEXED_REFERENCES {
        for index in reference.table_indices {
            *references
                .entry(reference.table.table_symbol)
                .or_default()
                .entry(*index)
                .or_default() += 1;
        }
    }
    for reference in FIXED_REFERENCES {
        *references
            .entry(reference.table_symbol)
            .or_default()
            .entry(reference.table_index)
            .or_default() += 1;
    }

    for (table_symbol, messages) in ALL_MESSAGE_TABLES {
        let actual_indices: BTreeSet<_> = references[table_symbol].keys().copied().collect();
        let expected_indices: BTreeSet<_> = (0..messages.len()).collect();
        assert_eq!(actual_indices, expected_indices, "{table_symbol}");
    }

    let repeated: Vec<_> = references
        .into_iter()
        .flat_map(|(table, indices)| {
            indices
                .into_iter()
                .filter(|(_, occurrences)| *occurrences > 1)
                .map(move |(index, occurrences)| (table, index, occurrences))
        })
        .collect();
    assert_eq!(
        repeated,
        [
            (WINDOW_TITLE_TABLE_SYMBOL, 8, 2),
            (WINDOW_TITLE_TABLE_SYMBOL, 11, 2),
            (WINDOW_TITLE_TABLE_SYMBOL, 12, 2),
        ]
    );
}

#[test]
fn message_ids_and_symbols_are_unique_across_physical_tables() {
    let mut ids = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    for (_, messages) in ALL_MESSAGE_TABLES {
        for message in messages {
            assert!(ids.insert(message.id), "duplicate ID: {}", message.id);
            assert!(
                symbols.insert(message.symbol),
                "duplicate symbol: {}",
                message.symbol
            );
        }
    }
}

#[test]
fn reference_actors_match_the_complete_renderer_producer_catalog() {
    let mut actual: Vec<_> = INDEXED_REFERENCES
        .iter()
        .map(|reference| (reference.producer_symbol, reference.table.actor))
        .chain(
            FIXED_REFERENCES
                .iter()
                .map(|reference| (reference.producer_symbol, reference.actor)),
        )
        .collect();
    actual.sort_by_key(|(symbol, actor)| (*symbol, actor.upper_offset));

    let mut expected: Vec<_> = super::super::EXPECTED_ACTOR_CREATIONS
        .iter()
        .map(|site| (site.producer_symbol, site.actor))
        .collect();
    expected.sort_by_key(|(symbol, actor)| (*symbol, actor.upper_offset));

    assert_eq!(actual, expected);
}
