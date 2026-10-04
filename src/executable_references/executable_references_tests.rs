use super::*;

const A0: u32 = 4;

#[test]
fn reports_pointer_and_typed_materialization_as_distinct_static_references() {
    let target = 0x0048_1234_u32;
    let source = [
        target.to_le_bytes(),
        lui(A0, 0x0048).to_le_bytes(),
        addiu(A0, A0, 0x1234).to_le_bytes(),
    ]
    .concat();
    let targets = BTreeMap::from([(target, "target".to_string())]);
    let mut references = BTreeMap::new();

    find_static_references_in_symbol(&source, 0x0040_0000, "owner", &targets, &mut references)
        .unwrap();

    assert_eq!(
        references["target"]
            .iter()
            .map(|reference| reference.kind)
            .collect::<Vec<_>>(),
        vec![
            StaticReferenceKind::Pointer,
            StaticReferenceKind::TypedAddressMaterialization,
        ]
    );
}

#[test]
fn reports_only_direct_calls_to_declared_targets() {
    let target = 0x0040_1230_u32;
    let words = [jal(target), jal(0x0040_4560), 0];
    let symbol = FileBackedTextSymbol {
        address: 0x0040_0000,
        bytes: words.iter().flat_map(|word| word.to_le_bytes()).collect(),
        owners: BTreeSet::from(["caller".to_string()]),
    };
    let mut calls = Vec::new();

    find_direct_calls_in_symbol(
        &symbol,
        &BTreeMap::from([(target, "callee".to_string())]),
        &mut calls,
    )
    .unwrap();

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].address, "0x00400000");
    assert_eq!(calls[0].owner_symbols, vec!["caller"]);
    assert_eq!(calls[0].target_symbol, "callee");
    assert_eq!(calls[0].instruction.address, calls[0].address);
}

fn lui(destination: u32, immediate: u16) -> u32 {
    (0x0f << 26) | (destination << 16) | u32::from(immediate)
}

fn addiu(destination: u32, source: u32, immediate: i16) -> u32 {
    (0x09 << 26) | (source << 21) | (destination << 16) | u32::from(immediate as u16)
}

fn jal(target: u32) -> u32 {
    (0x03 << 26) | ((target >> 2) & 0x03ff_ffff)
}
