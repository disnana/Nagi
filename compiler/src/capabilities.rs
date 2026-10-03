use crate::ast::{Class, Enum, Type};
use std::collections::{HashMap, HashSet};

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
}
