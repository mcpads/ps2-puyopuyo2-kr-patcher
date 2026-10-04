use super::*;

fn resolved(value: AbstractValue) -> ResolvedValue {
    ResolvedValue {
        value,
        instruction_indices: BTreeSet::new(),
    }
}

#[test]
fn constant_base_plus_dynamic_index_preserves_the_pointer_table_base() {
    assert_eq!(
        combine_add_values(
            resolved(AbstractValue::Constant(0x0048_f670)),
            ResolvedValue::unknown(),
        )
        .value,
        AbstractValue::IndexedAddress(0x0048_f670)
    );
}

#[test]
fn constant_register_writes_are_recognized_for_call_arguments() {
    let addiu_a0_zero_one = (0x09_u32 << 26) | (u32::from(FILE_ID_REGISTER) << 16) | 1;
    let lui_a1 = (0x0f_u32 << 26) | (u32::from(LOAD_TABLE_REGISTER) << 16) | 0x49;
    assert_eq!(
        destination_register(addiu_a0_zero_one),
        Some(FILE_ID_REGISTER)
    );
    assert_eq!(destination_register(lui_a1), Some(LOAD_TABLE_REGISTER));
}
