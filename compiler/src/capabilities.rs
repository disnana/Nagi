use crate::ast::{Class, Enum, Type};
use std::collections::{HashMap, HashSet};

/// Payloads must have a completely known owned representation. State and
/// context capabilities are separate from actor message/reply accounting.
pub(crate) fn charge_type_supported(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> Result<(), String> {
    fn visit(
        ty: &Type,
        classes: &HashMap<String, Class>,
        enums: &HashMap<String, Enum>,
        visiting: &mut HashSet<String>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 64 {
            return Err("actor payload type exceeds the charge depth limit (64)".into());
        }
        if matches!(
            ty.0.as_str(),
            "str" | "bytes" | "unit" | "UUID" | "timestamp" | "Error"
        ) && ty.1.is_empty()
        {
            return Ok(());
        }
        if classes.contains_key(&ty.0) || enums.contains_key(&ty.0) {
            if !ty.1.is_empty() {
                return Err(format!("actor payload has unsupported generic type: {ty}"));
            }
            if !visiting.insert(ty.0.clone()) {
                return Ok(());
            }
            let result = if let Some(class) = classes.get(&ty.0) {
                class.fields.iter().try_for_each(|(name, field)| {
                    visit(field, classes, enums, visiting, depth + 1)
                        .map_err(|error| format!("field {ty}.{name}: {error}"))
                })
            } else {
                enums[&ty.0].variants.iter().try_for_each(|variant| {
                    variant.fields.iter().try_for_each(|(name, field)| {
                        visit(field, classes, enums, visiting, depth + 1)
                            .map_err(|error| format!("field {ty}.{}.{name}: {error}", variant.name))
                    })
                })
            };
            visiting.remove(&ty.0);
            return result;
        }
        match ty.0.as_str() {
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64"
            | "bool" if ty.1.is_empty() => Ok(()),
            "owned" | "List" | "Option" if ty.1.len() == 1 => {
                visit(&ty.1[0], classes, enums, visiting, depth + 1)
                    .map_err(|error| format!("{} payload: {error}", ty.0))
            }
            "Result" if ty.1.len() == 2 => ty
                .1
                .iter()
                .enumerate()
                .try_for_each(|(index, inner)| {
                    visit(inner, classes, enums, visiting, depth + 1)
                        .map_err(|error| format!("Result {} payload: {error}", if index == 0 { "success" } else { "failure" }))
                }),
            _ => Err(format!(
                "actor message/reply payload cannot be charged: {ty}; use owned values with known storage (no Map, shared, view, native resource, or function graph)"
            )),
        }
    }
    visit(ty, classes, enums, &mut HashSet::new(), 0)
}

/// A payload can skip field traversal only when every possible representation
/// stores all of its bytes inline. Vec/String are never inline-only.
pub(crate) fn charge_inline_only(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    fn visit(
        ty: &Type,
        classes: &HashMap<String, Class>,
        enums: &HashMap<String, Enum>,
        visiting: &mut HashSet<String>,
        depth: usize,
    ) -> bool {
        if depth > 64 {
            return false;
        }
        if matches!(ty.0.as_str(), "str" | "bytes" | "Error" | "List") {
            return false;
        }
        if matches!(ty.0.as_str(), "unit" | "UUID" | "timestamp") && ty.1.is_empty() {
            return true;
        }
        let fields = if let Some(class) = classes.get(&ty.0) {
            Some(
                class
                    .fields
                    .iter()
                    .map(|(_, field)| field)
                    .collect::<Vec<_>>(),
            )
        } else {
            enums.get(&ty.0).map(|enumeration| {
                enumeration
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|(_, field)| field))
                    .collect::<Vec<_>>()
            })
        };
        if let Some(fields) = fields {
            if !ty.1.is_empty() || !visiting.insert(ty.0.clone()) {
                return false;
            }
            let result = fields
                .into_iter()
                .all(|field| visit(field, classes, enums, visiting, depth + 1));
            visiting.remove(&ty.0);
            return result;
        }
        match ty.0.as_str() {
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64"
            | "bool" => ty.1.is_empty(),
            "owned" | "Option" if ty.1.len() == 1 => {
                visit(&ty.1[0], classes, enums, visiting, depth + 1)
            }
            "Result" if ty.1.len() == 2 => {
                ty.1.iter()
                    .all(|inner| visit(inner, classes, enums, visiting, depth + 1))
            }
            _ => false,
        }
    }
    charge_type_supported(ty, classes, enums).is_ok()
        && visit(ty, classes, enums, &mut HashSet::new(), 0)
}

/// Whether a type's generated representation supports Serde. Error causes and
/// enums remain private data unless a separate serialization contract is added.
pub(crate) fn serde_type(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    fn visit(
        ty: &Type,
        classes: &HashMap<String, Class>,
        enums: &HashMap<String, Enum>,
        visiting: &mut HashSet<String>,
        depth: usize,
    ) -> bool {
        if depth > 64 {
            return false;
        }
        // These names always emit the intrinsic representation, including for
        // direct parser/check callers without canonical module identities.
        if matches!(ty.0.as_str(), "Error" | "Db" | "Html")
            || ty.0 == "fn" && !ty.1.is_empty()
            || ty.is_future()
        {
            return false;
        }
        if matches!(
            ty.0.as_str(),
            "str" | "bytes" | "unit" | "UUID" | "timestamp"
        ) {
            return ty.1.is_empty();
        }
        if !classes.contains_key(&ty.0) && crate::stdlib::resource(&ty.0).is_some() {
            return false;
        }
        if enums.contains_key(&ty.0) {
            return false;
        }
        if let Some(class) = classes.get(&ty.0) {
            if !ty.1.is_empty() {
                return false;
            }
            if !visiting.insert(ty.0.clone()) {
                return true;
            }
            let supported = class
                .fields
                .iter()
                .all(|(_, field)| visit(field, classes, enums, visiting, depth + 1));
            visiting.remove(&ty.0);
            return supported;
        }
        match ty.0.as_str() {
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64"
            | "bool" | "str" | "bytes" | "unit" | "UUID" | "timestamp" => ty.1.is_empty(),
            "List" | "view" | "owned" | "shared" | "Option" if ty.1.len() == 1 => {
                visit(&ty.1[0], classes, enums, visiting, depth + 1)
            }
            "Map" | "Result" if ty.1.len() == 2 => {
                ty.1.iter()
                    .all(|inner| visit(inner, classes, enums, visiting, depth + 1))
            }
            _ => false,
        }
    }

    visit(ty, classes, enums, &mut HashSet::new(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(name: &str, fields: Vec<Type>) -> Class {
        Class {
            name: name.into(),
            field_lines: vec![1; fields.len()],
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(i, ty)| (format!("field{i}"), ty))
                .collect(),
            line: 1,
        }
    }

    #[test]
    fn private_causes_prevent_serialization_through_nested_records() {
        let classes = HashMap::from([
            (
                "Failure".into(),
                class("Failure", vec![Type::named("Error")]),
            ),
            (
                "Wrapper".into(),
                class(
                    "Wrapper",
                    vec![Type::generic("List", vec![Type::named("Failure")])],
                ),
            ),
        ]);
        assert!(!serde_type(
            &Type::named("Wrapper"),
            &classes,
            &HashMap::new()
        ));
    }

    #[test]
    fn indirect_cycles_remain_serializable_only_with_serializable_fields() {
        let mut classes = HashMap::from([(
            "Node".into(),
            class(
                "Node",
                vec![Type::generic("List", vec![Type::named("Node")])],
            ),
        )]);
        assert!(serde_type(&Type::named("Node"), &classes, &HashMap::new()));
        classes
            .get_mut("Node")
            .unwrap()
            .fields
            .push(("cause".into(), Type::named("Error")));
        assert!(!serde_type(&Type::named("Node"), &classes, &HashMap::new()));
    }

    #[test]
    fn charge_rejects_excluded_storage_with_a_nested_field_path() {
        let classes = HashMap::from([(
            "Envelope".into(),
            class(
                "Envelope",
                vec![Type::generic("List", vec![Type::named("Payload")])],
            ),
        )]);
        let enums = HashMap::from([(
            "Payload".into(),
            Enum {
                name: "Payload".into(),
                variants: vec![crate::ast::EnumVariant {
                    name: "Data".into(),
                    fields: vec![(
                        "mapping".into(),
                        Type::generic("Map", vec![Type::named("str"), Type::named("i64")]),
                    )],
                    field_lines: vec![1],
                    line: 1,
                    name_span: Default::default(),
                }],
                line: 1,
            },
        )]);
        let error = charge_type_supported(&Type::named("Envelope"), &classes, &enums)
            .expect_err("Map must be rejected recursively");
        assert!(error.contains("Envelope.field0"), "{error}");
        assert!(error.contains("Payload.Data.mapping"), "{error}");
        for forbidden in [
            Type::generic("shared", vec![Type::named("i64")]),
            Type::generic("view", vec![Type::named("str")]),
            Type::generic("fn", vec![Type::named("unit")]),
            Type::generic("Future", vec![Type::named("unit")]),
            crate::stdlib::resource_type(crate::stdlib::Resource::Status, vec![]),
        ] {
            assert!(charge_type_supported(&forbidden, &classes, &enums).is_err());
        }
    }

    #[test]
    fn charge_recursive_lists_have_known_storage_but_require_a_walk() {
        let classes = HashMap::from([(
            "Node".into(),
            class(
                "Node",
                vec![Type::generic("List", vec![Type::named("Node")])],
            ),
        )]);
        assert!(charge_type_supported(&Type::named("Node"), &classes, &HashMap::new()).is_ok());
        assert!(!charge_inline_only(
            &Type::named("Node"),
            &classes,
            &HashMap::new()
        ));
    }

    #[test]
    fn charge_inline_capability_accounts_for_every_enum_variant() {
        let classes = HashMap::from([(
            "Coordinates".into(),
            class("Coordinates", vec![Type::named("i64"), Type::named("UUID")]),
        )]);
        let mut enums = HashMap::from([(
            "Message".into(),
            Enum {
                name: "Message".into(),
                variants: vec![crate::ast::EnumVariant {
                    name: "Position".into(),
                    fields: vec![(
                        "coordinates".into(),
                        Type::generic("Option", vec![Type::named("Coordinates")]),
                    )],
                    field_lines: vec![1],
                    line: 1,
                    name_span: Default::default(),
                }],
                line: 1,
            },
        )]);
        assert!(charge_inline_only(
            &Type::named("Message"),
            &classes,
            &enums
        ));
        enums.get_mut("Message").unwrap().variants[0]
            .fields
            .push(("cause".into(), Type::named("Error")));
        assert!(charge_type_supported(&Type::named("Message"), &classes, &enums).is_ok());
        assert!(!charge_inline_only(
            &Type::named("Message"),
            &classes,
            &enums
        ));
    }
}
