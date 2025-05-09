// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    compiler::{as_module, compile_units},
    tests::execute_function_with_single_storage_for_test,
};
use move_binary_format::errors::VMResult;
use move_core_types::{
    account_address::AccountAddress,
    identifier::Identifier,
    language_storage::ModuleId,
    u256::U256,
    value::{serialize_values, MoveValue},
    vm_status::StatusCode,
};
use move_vm_runtime::move_vm::SerializedReturnValues;
use move_vm_test_utils::InMemoryStorage;

const TEST_ADDR: AccountAddress = AccountAddress::new([42; AccountAddress::LENGTH]);
const TEST_MODULE_ID: &str = "M";
const EXPECT_MUTREF_OUT_VALUE: u64 = 90;
const USE_MUTREF_LABEL: &str = "use_mutref";
const USE_REF_LABEL: &str = "use_ref";
const FUN_NAMES: [&str; 2] = [USE_MUTREF_LABEL, USE_REF_LABEL];

// ensure proper errors are returned when ref & mut ref args fail to deserialize
#[test]
fn fail_arg_deserialize() {
    let mod_code = setup_module();
    // all of these should fail to deserialize because the functions expect u64 args
    let values = vec![
        MoveValue::U8(16),
        MoveValue::U16(1006),
        MoveValue::U32(16000),
        MoveValue::U128(512),
        MoveValue::U256(U256::from(12345u32)),
        MoveValue::Bool(true),
    ];
    for value in values {
        for name in FUN_NAMES {
            let status = run(&mod_code, name, value.clone())
                .expect_err("Should have failed to deserialize non-u64 type to u64")
                .major_status();
            assert_eq!(status, StatusCode::FAILED_TO_DESERIALIZE_ARGUMENT);
        }
    }
}

// check happy path for writing to mut ref args - may be unnecessary / covered by other tests
#[test]
fn mutref_output_success() {
    let mod_code = setup_module();
    let ret_values = run(&mod_code, USE_MUTREF_LABEL, MoveValue::U64(1)).unwrap();
    assert_eq!(1, ret_values.mutable_reference_outputs.len());
    let parsed = parse_u64_arg(&ret_values.mutable_reference_outputs.first().unwrap().1);
    assert_eq!(EXPECT_MUTREF_OUT_VALUE, parsed);
}

// TODO - how can we cause serialization errors in values returned from Move ?
// that would allow us to test error paths for outputs as well

fn setup_module() -> ModuleCode {
    // first function takes a mutable ref & writes to it, the other takes immutable ref, so we exercise both paths
    let code = format!(
        r#"
        module 0x{}::{} {{
            fun {}(a: &mut u64) {{ *a = {}; }}
            fun {}(_a: & u64) {{ }}
        }}
    "#,
        TEST_ADDR.to_hex(),
        TEST_MODULE_ID,
        USE_MUTREF_LABEL,
        EXPECT_MUTREF_OUT_VALUE,
        USE_REF_LABEL
    );

    let module_id = ModuleId::new(TEST_ADDR, Identifier::new(TEST_MODULE_ID).unwrap());
    (module_id, code)
}

fn run(module: &ModuleCode, fun_name: &str, arg: MoveValue) -> VMResult<SerializedReturnValues> {
    let module_id = &module.0;
    let function_name = Identifier::new(fun_name).unwrap();

    let modules = vec![module.clone()];
    let mut storage = InMemoryStorage::new();
    compile_modules(&mut storage, &modules);

    execute_function_with_single_storage_for_test(
        &storage,
        module_id,
        &function_name,
        &[],
        serialize_values(&vec![arg]),
    )
}

type ModuleCode = (ModuleId, String);

fn compile_modules(storage: &mut InMemoryStorage, modules: &[ModuleCode]) {
    modules.iter().for_each(|(id, code)| {
        compile_module(storage, id, code);
    });
}

fn compile_module(storage: &mut InMemoryStorage, mod_id: &ModuleId, code: &str) {
    let mut units = compile_units(code).unwrap();
    let module = as_module(units.pop().unwrap());
    let mut blob = vec![];
    module.serialize(&mut blob).unwrap();
    storage.add_module_bytes(mod_id.address(), mod_id.name(), blob.into());
}

fn parse_u64_arg(arg: &[u8]) -> u64 {
    let as_arr: [u8; 8] = arg[..8]
        .try_into()
        .expect("wrong u64 length, must be 8 bytes");
    u64::from_le_bytes(as_arr)
}
