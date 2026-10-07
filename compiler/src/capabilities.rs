use crate::ast::{Class, Enum, Type};
use std::collections::{HashMap, HashSet};

// A function pointer's signature and a native phantom marker are not stored
// payloads. Nominal fields form a finite graph; use an iterative walk so a deep
// DTO chain is neither stack overflow nor an invented auth-proof finding.
fn payload_any(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
    native_payloads: bool,
    stop_at_shared: bool,
    predicate: impl Fn(crate::stdlib::Resource) -> bool,
) -> bool {
    let mut pending = vec![ty];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if current.0 == "fn" || stop_at_shared && current.0 == "shared" {
            continue;
        }
        if let Some(resource) = crate::stdlib::resource(&current.0) {
            if predicate(resource) {
                return true;
            }
            if native_payloads {
                for index in crate::stdlib::resource_info(resource)
                    .inline_type_arguments
                    .iter()
                    .chain(crate::stdlib::shared_type_arguments(resource))
                {
                    if let Some(inner) = current.1.get(*index) {
                        pending.push(inner);
                    }
                }
            }
            continue;
        }
        pending.extend(&current.1);
        if !visited.insert(current.0.as_str()) {
            continue;
        }
        if let Some(class) = classes.get(&current.0) {
            pending.extend(class.fields.iter().map(|(_, field)| field));
        } else if let Some(enumeration) = enums.get(&current.0) {
            pending.extend(
                enumeration
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|(_, field)| field)),
            );
        }
    }
    false
}

/// Auth proofs may move through owned wrappers, but not be cloned via Arc.
/// Follow physical payloads using canonical symbols, rather than type mentions.
pub(crate) fn contains_auth_proof(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    payload_any(ty, classes, enums, true, false, |resource| {
        matches!(
            resource,
            crate::stdlib::Resource::Principal | crate::stdlib::Resource::Grant
        )
    })
}

/// Task handles and their opaque failures cannot be cloned or shared through
/// a wrapper or a generated record. Follow stored payloads, not phantom types.
pub(crate) fn contains_task_owner(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    payload_any(ty, classes, enums, true, false, |r| {
        matches!(
            r,
            crate::stdlib::Resource::Task | crate::stdlib::Resource::TaskFailure
        )
    })
}

/// Registered native types decide their own Debug contract. Their custom
/// formatter need not format phantom/indirect type parameters. Owned wrappers
/// and generated class/enum fields, however, do require their payload's Debug.
pub(crate) fn debug_supported(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    !payload_any(ty, classes, enums, false, false, |resource| {
        !crate::stdlib::resource_info(resource).debug
    })
}

pub(crate) fn contains_sqlite_nonshared(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    payload_any(ty, classes, enums, true, false, |resource| {
        let info = crate::stdlib::resource_info(resource);
        info.module == crate::stdlib::StandardModule::Sqlite && !info.shared
    })
}

/// Clone of an Arc clones its handle, not its payload. A function signature is
/// likewise not a stored transaction. This traversal follows actual Clone work.
pub(crate) fn contains_sqlite_noncopy(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    payload_any(ty, classes, enums, true, true, |resource| {
        let info = crate::stdlib::resource_info(resource);
        info.module == crate::stdlib::StandardModule::Sqlite && !info.copy
    })
}

pub(crate) fn contains_same_task_resource(
    ty: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    payload_any(
        ty,
        classes,
        enums,
        true,
        false,
        crate::stdlib::requires_same_task,
    )
}

/// Fields supported by the generated FromRow adapter. This is not a solver for
/// handwritten Rust implementations; only the new SQLite API requires it.
pub(crate) fn generated_row_supported(
    class: &Class,
    classes: &HashMap<String, Class>,
    resolved: bool,
) -> bool {
    class.fields.iter().all(|(_, ty)| {
        let mut scalar = ty;
        while scalar.0 == "owned" {
            let Some(inner) = scalar.1.first() else {
                return false;
            };
            scalar = inner;
        }
        if scalar.0 == "Option" {
            let Some(inner) = scalar.1.first() else {
                return false;
            };
            scalar = inner;
        }
        while scalar.0 == "owned" {
            let Some(inner) = scalar.1.first() else {
                return false;
            };
            scalar = inner;
        }
        [
            "i8", "i16", "i32", "i64", "u8", "u16", "u32", "f32", "f64", "bool", "str", "bytes",
        ]
        .contains(&scalar.0.as_str())
            && (matches!(scalar.0.as_str(), "str" | "bytes")
                || resolved
                || !classes.contains_key(&scalar.0))
    })
}

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
        if !classes.contains_key(&ty.0) {
            if let Some(resource) = crate::stdlib::resource(&ty.0) {
                return crate::stdlib::native_serde_supported(resource);
            }
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
    #[test]
    fn native_payload_roles_are_purpose_specific() {
        use crate::stdlib::{resource_type as native, Resource as R};
        let classes = HashMap::new();
        let enums = HashMap::new();
        let principal = native(R::Principal, vec![]);
        let grant = native(R::Grant, vec![Type::named("Read")]);
        let option = |t| Type::generic("Option", vec![t]);
        // Hand-written representation inventory. Only inline/shared are current
        // traversal APIs; indirect/signature/phantom express why excluded slots
        // must not silently be defaulted to absent runtime payload.
        for &resource in crate::stdlib::RESOURCES {
            let [inline, shared, indirect, signature, phantom]: [&[usize]; 5] = match resource {
                R::App => [&[], &[0], &[], &[1], &[]],
                R::Supervisor => [&[], &[0], &[], &[], &[]],
                R::Actor => [&[], &[], &[0, 1, 2], &[], &[]],
                R::Turn => [&[0, 1, 2], &[], &[], &[], &[]],
                R::Grant => [&[], &[], &[], &[], &[0]],
                R::Task => [&[], &[], &[0], &[], &[]],
                _ => [&[], &[], &[], &[], &[]],
            };
            let info = crate::stdlib::resource_info(resource);
            use crate::stdlib::TypeArgumentRole as Role;
            for (role, expected) in [
                (Role::InlinePayload, inline),
                (Role::SharedPayload, shared),
                (Role::IndirectProtocol, indirect),
                (Role::CallbackSignature, signature),
                (Role::NominalPhantom, phantom),
            ] {
                assert_eq!(
                    crate::stdlib::type_argument_positions(resource, role),
                    expected,
                    "{resource:?} {role:?}"
                );
            }
            assert_eq!(info.inline_type_arguments, inline, "{resource:?}");
            assert_eq!(
                crate::stdlib::shared_type_arguments(resource),
                shared,
                "{resource:?}"
            );
            let positions: Vec<_> = [inline, shared, indirect, signature, phantom]
                .into_iter()
                .flatten()
                .copied()
                .collect();
            assert_eq!(
                positions.len(),
                info.arity,
                "classification missing or duplicated: {resource:?}"
            );
            assert_eq!(
                positions.into_iter().collect::<HashSet<_>>(),
                (0..info.arity).collect(),
                "classification out of range or duplicated: {resource:?}"
            );
        }
        // Helper-level traversal observations, not claims that these generic
        // types are accepted by actor_payload/Charge or nominal marker validation.
        for (ty, expected) in [
            (
                native(R::App, vec![Type::named("i64"), principal.clone()]),
                false,
            ),
            (
                native(
                    R::App,
                    vec![option(principal.clone()), Type::named("Error")],
                ),
                true,
            ),
            (native(R::Supervisor, vec![option(grant.clone())]), true),
            (
                native(
                    R::Turn,
                    vec![principal.clone(), Type::named("i64"), Type::named("Error")],
                ),
                true,
            ),
            (
                native(
                    R::Actor,
                    vec![principal.clone(), grant.clone(), Type::named("Error")],
                ),
                false,
            ),
            (
                Type::generic("fn", vec![grant.clone(), Type::named("unit")]),
                false,
            ),
            (Type::generic("List", vec![option(grant)]), true),
        ] {
            assert_eq!(
                contains_auth_proof(&ty, &classes, &enums),
                expected,
                "{ty:?}"
            );
        }
        // Actor's indirect protocol data is validated separately; it is not
        // phantom merely because this particular walk doesn't traverse it.
        assert!(charge_type_supported(&principal, &classes, &enums).is_err());
    }

    #[test]
    fn native_debug_does_not_require_generic_payload_debug() {
        use crate::stdlib::{resource_type as native, Resource as R};
        let classes = HashMap::new();
        let enums = HashMap::new();
        let principal = native(R::Principal, vec![]);
        for resource in [R::App, R::Supervisor, R::Actor, R::Turn] {
            let args = vec![principal.clone(); crate::stdlib::resource_info(resource).arity];
            assert!(debug_supported(&native(resource, args), &classes, &enums));
        }
        assert!(!debug_supported(&principal, &classes, &enums));
        assert!(!debug_supported(
            &Type::generic("Option", vec![principal.clone()]),
            &classes,
            &enums
        ));
        assert!(debug_supported(
            &Type::generic("fn", vec![principal, Type::named("unit")]),
            &classes,
            &enums
        ));
    }

    #[test]
    fn serde_charge_and_owned_fields_are_distinct_capabilities() {
        use crate::stdlib::{resource_type as native, Resource as R};
        let classes = HashMap::from([
            (
                "Scalar".into(),
                class(
                    "Scalar",
                    vec![Type::generic("owned", vec![Type::named("i64")])],
                ),
            ),
            ("Text".into(), class("Text", vec![Type::named("str")])),
            (
                "Private".into(),
                class("Private", vec![Type::named("Error")]),
            ),
            (
                "Storage".into(),
                class("Storage", vec![native(R::Status, vec![])]),
            ),
        ]);
        let enums = HashMap::from([(
            "Choice".into(),
            Enum {
                name: "Choice".into(),
                line: 1,
                variants: vec![crate::ast::EnumVariant {
                    name: "Value".into(),
                    fields: vec![("n".into(), Type::named("i64"))],
                    field_lines: vec![1],
                    line: 1,
                    name_span: Default::default(),
                }],
            },
        )]);
        for (ty, serde, charge, inline) in [
            (Type::named("Scalar"), true, true, true),
            (Type::named("Text"), true, true, false),
            (Type::named("Private"), false, true, false),
            (Type::named("Storage"), false, false, false),
            (Type::named("Choice"), false, true, true),
            (Type::named("Db"), false, false, false),
        ] {
            assert_eq!(serde_type(&ty, &classes, &enums), serde, "{ty:?}");
            assert_eq!(
                charge_type_supported(&ty, &classes, &enums).is_ok(),
                charge,
                "{ty:?}"
            );
            assert_eq!(charge_inline_only(&ty, &classes, &enums), inline, "{ty:?}");
        }
        for &resource in crate::stdlib::RESOURCES {
            // This observes capability helpers, not generic well-formedness.
            let args = vec![Type::named("i64"); crate::stdlib::resource_info(resource).arity];
            let ty = native(resource, args);
            assert!(!serde_type(&ty, &classes, &enums), "{resource:?}");
            assert!(
                charge_type_supported(&ty, &classes, &enums).is_err(),
                "{resource:?}"
            );
        }
        // Resource storage=true allows a field; it does not promise Serde or Charge.
        assert!(crate::stdlib::resource_info(R::Status).storage);
    }
}
