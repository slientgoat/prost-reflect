#![cfg(test)]

mod arbitrary;
mod json;

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    iter::FromIterator,
};

use once_cell::sync::Lazy;
use proptest::{prelude::*, test_runner::TestCaseError};
use prost::{bytes::Bytes, Message};
use prost_reflect::{DynamicMessage, FileDescriptor, MapKey, ReflectMessage, Value};

include!(concat!(env!("OUT_DIR"), "/test.rs"));

const FILE_DESCRIPTOR_SET_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/file_descriptor_set.bin"));

static TEST_FILE_DESCRIPTOR: Lazy<FileDescriptor> =
    Lazy::new(|| FileDescriptor::decode(FILE_DESCRIPTOR_SET_BYTES).unwrap());

fn test_file_descriptor() -> FileDescriptor {
    TEST_FILE_DESCRIPTOR.clone()
}

#[test]
fn clear_message() {
    let mut dynamic = to_dynamic(&Scalars {
        double: 1.1,
        float: 2.2,
        int32: 3,
        int64: 4,
        uint32: 5,
        uint64: 6,
        sint32: 7,
        sint64: 8,
        fixed32: 9,
        fixed64: 10,
        sfixed32: 11,
        sfixed64: 12,
        r#bool: true,
        string: "5".to_owned(),
        bytes: b"6".to_vec(),
    });

    dynamic.clear();

    assert!(!dynamic.has_field_by_name("double"));
    assert!(!dynamic.has_field_by_name("float"));
    assert!(!dynamic.has_field_by_name("int32"));
    assert!(!dynamic.has_field_by_name("int64"));
    assert!(!dynamic.has_field_by_name("uint32"));
    assert!(!dynamic.has_field_by_name("uint64"));
    assert!(!dynamic.has_field_by_name("sint32"));
    assert!(!dynamic.has_field_by_name("sint64"));
    assert!(!dynamic.has_field_by_name("fixed32"));
    assert!(!dynamic.has_field_by_name("fixed64"));
    assert!(!dynamic.has_field_by_name("sfixed32"));
    assert!(!dynamic.has_field_by_name("sfixed64"));
    assert!(!dynamic.has_field_by_name("bool"));
    assert!(!dynamic.has_field_by_name("string"));
    assert!(!dynamic.has_field_by_name("bytes"));

    let encoded_bytes = dynamic.encode_to_vec();
    assert!(encoded_bytes.is_empty());
}

#[test]
#[should_panic(expected = "nvalid value U32(5) for field")]
fn set_field_validates_type() {
    let mut dynamic = to_dynamic(&Scalars::default());

    dynamic.set_field_by_name("double", Value::U32(5));
}

#[test]
fn test_descriptor_methods() {
    let message_desc = test_file_descriptor()
        .get_message_by_name("my.package.MyMessage")
        .unwrap();
    assert_eq!(message_desc.name(), "MyMessage");
    assert_eq!(message_desc.full_name(), "my.package.MyMessage");
    assert_eq!(message_desc.parent_message(), None);
    assert_eq!(message_desc.package_name(), "my.package");

    let field_desc = message_desc.get_field_by_name("my_field").unwrap();
    assert_eq!(field_desc.name(), "my_field");
    assert_eq!(field_desc.full_name(), "my.package.MyMessage.my_field");

    let nested_message_desc = test_file_descriptor()
        .get_message_by_name("my.package.MyMessage.MyNestedMessage")
        .unwrap();
    assert_eq!(nested_message_desc.name(), "MyNestedMessage");
    assert_eq!(
        nested_message_desc.full_name(),
        "my.package.MyMessage.MyNestedMessage"
    );
    assert_eq!(
        nested_message_desc.parent_message(),
        Some(message_desc.clone())
    );
    assert_eq!(nested_message_desc.package_name(), "my.package");

    let enum_desc = test_file_descriptor()
        .get_enum_by_name("my.package.MyEnum")
        .unwrap();
    assert_eq!(enum_desc.name(), "MyEnum");
    assert_eq!(enum_desc.full_name(), "my.package.MyEnum");
    assert_eq!(enum_desc.parent_message(), None);
    assert_eq!(enum_desc.package_name(), "my.package");

    let enum_value_desc = enum_desc.get_value_by_name("MY_VALUE").unwrap();
    assert_eq!(enum_value_desc.name(), "MY_VALUE");
    assert_eq!(enum_value_desc.full_name(), "my.package.MY_VALUE");

    let nested_enum_desc = test_file_descriptor()
        .get_enum_by_name("my.package.MyMessage.MyNestedEnum")
        .unwrap();
    assert_eq!(nested_enum_desc.name(), "MyNestedEnum");
    assert_eq!(
        nested_enum_desc.full_name(),
        "my.package.MyMessage.MyNestedEnum"
    );
    assert_eq!(nested_enum_desc.parent_message(), Some(message_desc));
    assert_eq!(nested_enum_desc.package_name(), "my.package");

    let service_desc = test_file_descriptor()
        .services()
        .find(|s| s.full_name() == "my.package.MyService")
        .unwrap();
    assert_eq!(service_desc.name(), "MyService");
    assert_eq!(service_desc.full_name(), "my.package.MyService");
    assert_eq!(service_desc.package_name(), "my.package");

    let method_desc = service_desc
        .methods()
        .find(|m| m.name() == "MyMethod")
        .unwrap();
    assert_eq!(method_desc.name(), "MyMethod");
    assert_eq!(method_desc.full_name(), "my.package.MyService.MyMethod");
}

#[test]
fn test_descriptor_names_no_package() {
    let message_desc = test_file_descriptor()
        .get_message_by_name("MyMessage")
        .unwrap();
    assert_eq!(message_desc.name(), "MyMessage");
    assert_eq!(message_desc.full_name(), "MyMessage");
    assert_eq!(message_desc.parent_message(), None);
    assert_eq!(message_desc.package_name(), "");

    let field_desc = message_desc.get_field_by_name("my_field").unwrap();
    assert_eq!(field_desc.name(), "my_field");
    assert_eq!(field_desc.full_name(), "MyMessage.my_field");

    let nested_message_desc = test_file_descriptor()
        .get_message_by_name("MyMessage.MyNestedMessage")
        .unwrap();
    assert_eq!(nested_message_desc.name(), "MyNestedMessage");
    assert_eq!(nested_message_desc.full_name(), "MyMessage.MyNestedMessage");
    assert_eq!(
        nested_message_desc.parent_message(),
        Some(message_desc.clone())
    );
    assert_eq!(nested_message_desc.package_name(), "");

    let enum_desc = test_file_descriptor().get_enum_by_name("MyEnum").unwrap();
    assert_eq!(enum_desc.name(), "MyEnum");
    assert_eq!(enum_desc.full_name(), "MyEnum");
    assert_eq!(enum_desc.parent_message(), None);
    assert_eq!(enum_desc.package_name(), "");

    let enum_value_desc = enum_desc.get_value_by_name("MY_VALUE").unwrap();
    assert_eq!(enum_value_desc.name(), "MY_VALUE");
    assert_eq!(enum_value_desc.full_name(), "MY_VALUE");

    let nested_enum_desc = test_file_descriptor()
        .get_enum_by_name("MyMessage.MyNestedEnum")
        .unwrap();
    assert_eq!(nested_enum_desc.name(), "MyNestedEnum");
    assert_eq!(nested_enum_desc.full_name(), "MyMessage.MyNestedEnum");
    assert_eq!(nested_enum_desc.parent_message(), Some(message_desc));
    assert_eq!(nested_enum_desc.package_name(), "");

    let service_desc = test_file_descriptor()
        .services()
        .find(|s| s.full_name() == "MyService")
        .unwrap();
    assert_eq!(service_desc.name(), "MyService");
    assert_eq!(service_desc.full_name(), "MyService");
    assert_eq!(service_desc.package_name(), "");

    let method_desc = service_desc
        .methods()
        .find(|m| m.name() == "MyMethod")
        .unwrap();
    assert_eq!(method_desc.name(), "MyMethod");
    assert_eq!(method_desc.full_name(), "MyService.MyMethod");
}

#[test]
fn decode_scalars() {
    let dynamic = to_dynamic(&Scalars {
        double: 1.1,
        float: 2.2,
        int32: 3,
        int64: 4,
        uint32: 5,
        uint64: 6,
        sint32: 7,
        sint64: 8,
        fixed32: 9,
        fixed64: 10,
        sfixed32: 11,
        sfixed64: 12,
        r#bool: true,
        string: "5".to_owned(),
        bytes: b"6".to_vec(),
    });

    assert_eq!(
        dynamic.get_field_by_name("double").unwrap().as_f64(),
        Some(1.1)
    );
    assert_eq!(
        dynamic.get_field_by_name("float").unwrap().as_f32(),
        Some(2.2)
    );
    assert_eq!(
        dynamic.get_field_by_name("int32").unwrap().as_i32(),
        Some(3)
    );
    assert_eq!(
        dynamic.get_field_by_name("int64").unwrap().as_i64(),
        Some(4)
    );
    assert_eq!(
        dynamic.get_field_by_name("uint32").unwrap().as_u32(),
        Some(5)
    );
    assert_eq!(
        dynamic.get_field_by_name("uint64").unwrap().as_u64(),
        Some(6)
    );
    assert_eq!(
        dynamic.get_field_by_name("sint32").unwrap().as_i32(),
        Some(7)
    );
    assert_eq!(
        dynamic.get_field_by_name("sint64").unwrap().as_i64(),
        Some(8)
    );
    assert_eq!(
        dynamic.get_field_by_name("fixed32").unwrap().as_u32(),
        Some(9)
    );
    assert_eq!(
        dynamic.get_field_by_name("fixed64").unwrap().as_u64(),
        Some(10)
    );
    assert_eq!(
        dynamic.get_field_by_name("sfixed32").unwrap().as_i32(),
        Some(11)
    );
    assert_eq!(
        dynamic.get_field_by_name("sfixed64").unwrap().as_i64(),
        Some(12)
    );
    assert_eq!(
        dynamic.get_field_by_name("bool").unwrap().as_bool(),
        Some(true)
    );
    assert_eq!(
        dynamic.get_field_by_name("string").unwrap().as_str(),
        Some("5")
    );
    assert_eq!(
        dynamic.get_field_by_name("bytes").unwrap().as_bytes(),
        Some(&Bytes::from_static(b"6"))
    );
}

#[test]
fn decode_scalar_arrays() {
    let dynamic = to_dynamic(&ScalarArrays {
        double: vec![1.1, 2.2],
        float: vec![3.3f32, 4.4f32],
        int32: vec![5, -6],
        int64: vec![7, -8],
        uint32: vec![9, 10],
        uint64: vec![11, 12],
        sint32: vec![13, -14],
        sint64: vec![15, -16],
        fixed32: vec![17, 18],
        fixed64: vec![19, 20],
        sfixed32: vec![21, -22],
        sfixed64: vec![23, -24],
        r#bool: vec![true, false],
        string: vec!["25".to_owned(), "26".to_owned()],
        bytes: vec![b"27".to_vec(), b"28".to_vec()],
    });

    assert_eq!(
        dynamic.get_field_by_name("double").unwrap().as_list(),
        Some([Value::F64(1.1), Value::F64(2.2)].as_ref())
    );
    assert_eq!(
        dynamic.get_field_by_name("float").unwrap().as_list(),
        Some([Value::F32(3.3f32), Value::F32(4.4f32)].as_ref())
    );
    assert_eq!(
        dynamic.get_field_by_name("int32").unwrap().as_list(),
        Some([Value::I32(5), Value::I32(-6)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("int64").unwrap().as_list(),
        Some([Value::I64(7), Value::I64(-8)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("uint32").unwrap().as_list(),
        Some([Value::U32(9), Value::U32(10)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("uint64").unwrap().as_list(),
        Some([Value::U64(11), Value::U64(12)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("sint32").unwrap().as_list(),
        Some([Value::I32(13), Value::I32(-14)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("sint64").unwrap().as_list(),
        Some([Value::I64(15), Value::I64(-16)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("fixed32").unwrap().as_list(),
        Some([Value::U32(17), Value::U32(18)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("fixed64").unwrap().as_list(),
        Some([Value::U64(19), Value::U64(20)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("sfixed32").unwrap().as_list(),
        Some([Value::I32(21), Value::I32(-22)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("sfixed64").unwrap().as_list(),
        Some([Value::I64(23), Value::I64(-24)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("bool").unwrap().as_list(),
        Some([Value::Bool(true), Value::Bool(false)].as_ref()),
    );
    assert_eq!(
        dynamic.get_field_by_name("string").unwrap().as_list(),
        Some(
            [
                Value::String("25".to_owned()),
                Value::String("26".to_owned())
            ]
            .as_ref()
        ),
    );
    assert_eq!(
        dynamic.get_field_by_name("bytes").unwrap().as_list(),
        Some(
            [
                Value::Bytes(Bytes::from_static(b"27")),
                Value::Bytes(Bytes::from_static(b"28"))
            ]
            .as_ref()
        ),
    );
}

#[test]
fn decode_complex_type() {
    let dynamic = to_dynamic(&ComplexType {
        string_map: HashMap::from_iter([
            (
                "1".to_owned(),
                Scalars {
                    double: 1.1,
                    float: 2.2,
                    int32: 3,
                    ..Default::default()
                },
            ),
            (
                "2".to_owned(),
                Scalars {
                    int64: 4,
                    uint32: 5,
                    uint64: 6,
                    ..Default::default()
                },
            ),
        ]),
        int_map: HashMap::from_iter([
            (
                3,
                Scalars {
                    sint32: 7,
                    sint64: 8,
                    fixed32: 9,
                    ..Default::default()
                },
            ),
            (
                4,
                Scalars {
                    sint64: 8,
                    fixed32: 9,
                    fixed64: 10,
                    ..Default::default()
                },
            ),
        ]),
        nested: Some(Scalars {
            sfixed32: 11,
            sfixed64: 12,
            r#bool: true,
            string: "5".to_owned(),
            bytes: b"6".to_vec(),
            ..Default::default()
        }),
        my_enum: vec![0, 1, 2, 3],
    });

    fn empty_scalars() -> DynamicMessage {
        DynamicMessage::new(
            test_file_descriptor()
                .get_message_by_name(".test.Scalars")
                .unwrap(),
        )
    }

    assert_eq!(
        dynamic.get_field_by_name("string_map").unwrap().as_map(),
        Some(&HashMap::from_iter([
            (MapKey::String("1".to_owned()), {
                let mut msg = empty_scalars();
                msg.set_field_by_name("double", Value::F64(1.1));
                msg.set_field_by_name("float", Value::F32(2.2));
                msg.set_field_by_name("int32", Value::I32(3));
                Value::Message(msg)
            }),
            (MapKey::String("2".to_owned()), {
                let mut msg = empty_scalars();
                msg.set_field_by_name("int64", Value::I64(4));
                msg.set_field_by_name("uint32", Value::U32(5));
                msg.set_field_by_name("uint64", Value::U64(6));
                Value::Message(msg)
            })
        ])),
    );
    assert_eq!(
        dynamic.get_field_by_name("int_map").unwrap().as_map(),
        Some(&HashMap::from_iter([
            (MapKey::I32(3), {
                let mut msg = empty_scalars();
                msg.set_field_by_name("sint32", Value::I32(7));
                msg.set_field_by_name("sint64", Value::I64(8));
                msg.set_field_by_name("fixed32", Value::U32(9));
                Value::Message(msg)
            }),
            (MapKey::I32(4), {
                let mut msg = empty_scalars();
                msg.set_field_by_name("sint64", Value::I64(8));
                msg.set_field_by_name("fixed32", Value::U32(9));
                msg.set_field_by_name("fixed64", Value::U64(10));
                Value::Message(msg)
            })
        ])),
    );
    assert_eq!(
        dynamic.get_field_by_name("nested").unwrap().as_message(),
        Some(&{
            let mut msg = empty_scalars();
            msg.set_field_by_name("sfixed32", Value::I32(11));
            msg.set_field_by_name("sfixed64", Value::I64(12));
            msg.set_field_by_name("bool", Value::Bool(true));
            msg.set_field_by_name("string", Value::String("5".to_owned()));
            msg.set_field_by_name("bytes", Value::Bytes(Bytes::from_static(b"6")));
            msg
        })
    );
    assert_eq!(
        dynamic.get_field_by_name("my_enum").unwrap().as_list(),
        Some(
            [
                Value::EnumNumber(0),
                Value::EnumNumber(1),
                Value::EnumNumber(2),
                Value::EnumNumber(3),
            ]
            .as_ref()
        ),
    );
}

#[test]
fn decode_default_values() {
    let dynamic = DynamicMessage::new(
        test_file_descriptor()
            .get_message_by_name(".test2.DefaultValues")
            .unwrap(),
    );

    assert_eq!(
        dynamic.get_field_by_name("double").unwrap().as_f64(),
        Some(1.1)
    );
    assert_eq!(
        dynamic.get_field_by_name("float").unwrap().as_f32(),
        Some(2.2)
    );
    assert_eq!(
        dynamic.get_field_by_name("int32").unwrap().as_i32(),
        Some(-3)
    );
    assert_eq!(
        dynamic.get_field_by_name("int64").unwrap().as_i64(),
        Some(4)
    );
    assert_eq!(
        dynamic.get_field_by_name("uint32").unwrap().as_u32(),
        Some(5)
    );
    assert_eq!(
        dynamic.get_field_by_name("uint64").unwrap().as_u64(),
        Some(6)
    );
    assert_eq!(
        dynamic.get_field_by_name("sint32").unwrap().as_i32(),
        Some(-7)
    );
    assert_eq!(
        dynamic.get_field_by_name("sint64").unwrap().as_i64(),
        Some(8)
    );
    assert_eq!(
        dynamic.get_field_by_name("fixed32").unwrap().as_u32(),
        Some(9)
    );
    assert_eq!(
        dynamic.get_field_by_name("fixed64").unwrap().as_u64(),
        Some(10)
    );
    assert_eq!(
        dynamic.get_field_by_name("sfixed32").unwrap().as_i32(),
        Some(-11)
    );
    assert_eq!(
        dynamic.get_field_by_name("sfixed64").unwrap().as_i64(),
        Some(12)
    );
    assert_eq!(
        dynamic.get_field_by_name("bool").unwrap().as_bool(),
        Some(true)
    );
    assert_eq!(
        dynamic.get_field_by_name("string").unwrap().as_str(),
        Some("hello")
    );
    assert_eq!(
        dynamic.get_field_by_name("bytes").unwrap().as_bytes(),
        Some(&Bytes::from_static(
            b"\0\x01\x07\x08\x0C\n\r\t\x0B\\\'\"\xFE"
        ))
    );
    assert_eq!(
        dynamic
            .get_field_by_name("defaulted_enum")
            .unwrap()
            .as_enum_number(),
        Some(3)
    );
    assert_eq!(
        dynamic.get_field_by_name("enum").unwrap().as_enum_number(),
        Some(2)
    );
}

#[test]
fn set_oneof() {
    let mut dynamic = DynamicMessage::new(
        test_file_descriptor()
            .get_message_by_name(".test.MessageWithOneof")
            .unwrap(),
    );

    assert_eq!(
        dynamic.descriptor().oneofs().next().unwrap().name(),
        "test_oneof"
    );

    dynamic.set_field_by_name("oneof_field_1", Value::String("hello".to_owned()));
    assert!(dynamic.has_field_by_name("oneof_field_1"));

    dynamic.set_field_by_name("oneof_field_2", Value::I32(5));
    assert!(dynamic.has_field_by_name("oneof_field_2"));
    assert!(!dynamic.has_field_by_name("oneof_field_1"));
}

#[test]
fn roundtrip_scalars() {
    roundtrip(&Scalars {
        double: 1.1,
        float: 2.2,
        int32: 3,
        int64: 4,
        uint32: 5,
        uint64: 6,
        sint32: 7,
        sint64: 8,
        fixed32: 9,
        fixed64: 10,
        sfixed32: 11,
        sfixed64: 12,
        r#bool: true,
        string: "5".to_owned(),
        bytes: b"6".to_vec(),
    })
    .unwrap();
}

#[test]
fn roundtrip_scalar_arrays() {
    roundtrip(&ScalarArrays {
        double: vec![1.1, 2.2],
        float: vec![3.3f32, 4.4f32],
        int32: vec![5, -6],
        int64: vec![7, -8],
        uint32: vec![9, 10],
        uint64: vec![11, 12],
        sint32: vec![13, -14],
        sint64: vec![15, -16],
        fixed32: vec![17, 18],
        fixed64: vec![19, 20],
        sfixed32: vec![21, -22],
        sfixed64: vec![23, 24],
        r#bool: vec![true, false],
        string: vec!["25".to_owned(), "26".to_owned()],
        bytes: vec![b"27".to_vec(), b"28".to_vec()],
    })
    .unwrap();
}

#[test]
fn roundtrip_complex_type() {
    roundtrip(&ComplexType {
        string_map: HashMap::from_iter([
            (
                "1".to_owned(),
                Scalars {
                    double: 1.1,
                    float: 2.2,
                    int32: 3,
                    ..Default::default()
                },
            ),
            (
                "2".to_owned(),
                Scalars {
                    int64: 4,
                    uint32: 5,
                    uint64: 6,
                    ..Default::default()
                },
            ),
        ]),
        int_map: HashMap::from_iter([
            (
                3,
                Scalars {
                    sint32: 7,
                    sint64: 8,
                    fixed32: 9,
                    ..Default::default()
                },
            ),
            (
                4,
                Scalars {
                    sint64: 8,
                    fixed32: 9,
                    fixed64: 10,
                    ..Default::default()
                },
            ),
        ]),
        nested: Some(Scalars {
            sfixed32: 11,
            sfixed64: 12,
            r#bool: true,
            string: "5".to_owned(),
            bytes: b"6".to_vec(),
            ..Default::default()
        }),
        my_enum: vec![0, 1, 2, 3],
    })
    .unwrap();
}

#[test]
fn roundtrip_well_known_types() {
    roundtrip(&WellKnownTypes {
        timestamp: Some(prost_types::Timestamp {
            seconds: 63_108_020,
            nanos: 21_000_000,
        }),
        duration: Some(prost_types::Duration {
            seconds: 1,
            nanos: 340_012,
        }),
        r#struct: Some(prost_types::Struct {
            fields: BTreeMap::from_iter([
                (
                    "number".to_owned(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::NumberValue(42.)),
                    },
                ),
                (
                    "null".to_owned(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::NullValue(0)),
                    },
                ),
            ]),
        }),
        float: Some(42.1),
        double: Some(12.4),
        int32: Some(1),
        int64: Some(-2),
        uint32: Some(3),
        uint64: Some(4),
        bool: Some(false),
        string: Some("hello".to_owned()),
        bytes: Some(b"hello".to_vec()),
        mask: Some(prost_types::FieldMask {
            paths: vec!["field_one".to_owned(), "field_two.b.d".to_owned()],
        }),
        list: Some(prost_types::ListValue {
            values: vec![
                prost_types::Value {
                    kind: Some(prost_types::value::Kind::StringValue("foo".to_owned())),
                },
                prost_types::Value {
                    kind: Some(prost_types::value::Kind::BoolValue(false)),
                },
            ],
        }),
        null: 0,
        empty: Some(()),
    })
    .unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 32,
        .. ProptestConfig::default()
    })]

    #[test]
    fn roundtrip_arb_scalars(message: Scalars) {
        roundtrip(&message)?;
    }

    #[test]
    fn roundtrip_arb_scalar_arrays(message: ScalarArrays) {
        roundtrip(&message)?;
    }

    #[test]
    fn roundtrip_arb_complex_type(message: ComplexType) {
        roundtrip(&message)?;
    }

    #[test]
    fn roundtrip_arb_well_known_types(message: WellKnownTypes) {
        roundtrip(&message)?;
    }
}

#[test]
fn unpacked_fields_accept_packed_bytes() {
    let desc = TEST_FILE_DESCRIPTOR
        .get_message_by_name("test2.UnpackedScalarArray")
        .unwrap();
    assert!(desc.get_field_by_name("unpacked_double").unwrap().is_list());
    assert!(!desc
        .get_field_by_name("unpacked_double")
        .unwrap()
        .is_packed());
    let mut message = DynamicMessage::new(desc);
    message
        .merge(
            [
                0o322, 0o2, b' ', 0, 0, 0, 0, 0, 0, 0, 0, 0o232, 0o231, 0o231, 0o231, 0o231, 0o231,
                0o271, b'?', 0o377, 0o377, 0o377, 0o377, 0o377, 0o377, 0o357, 0o177, 0, 0, 0, 0, 0,
                0, 0o20, 0,
            ]
            .as_ref(),
        )
        .unwrap();

    assert_eq!(
        message
            .get_field_by_name("unpacked_double")
            .unwrap()
            .as_list(),
        Some([
            Value::F64(0.0),
            Value::F64(0.1),
            Value::F64(179769313486231570000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.0),
            Value::F64(0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000022250738585072014),
        ].as_ref())
    );
}

fn to_dynamic<T>(message: &T) -> DynamicMessage
where
    T: PartialEq + Debug + ReflectMessage + Default,
{
    let mut dynamic_message = DynamicMessage::new(message.descriptor());
    dynamic_message.transcode_from(message).unwrap();
    dynamic_message
}

fn roundtrip<T>(message: &T) -> Result<(), TestCaseError>
where
    T: PartialEq + Debug + ReflectMessage + Default,
{
    let dynamic_message = to_dynamic(message);
    let roundtripped_message: T = dynamic_message.transcode_to().unwrap();
    prop_assert_eq!(message, &roundtripped_message);
    Ok(())
}
