use super::{definition_node, source_location, Edge, EdgeKind, Graph, Group, Node, NodeKind};
use crate::{
    ast::{DefKind, DefinitionInfo, Program, Type},
    source::Sources,
};
use std::{collections::BTreeMap, path::Path};

const DISPLAY_FIELDS: usize = 8;

struct Index<'a> {
    definitions: BTreeMap<&'a str, &'a DefinitionInfo>,
    types: BTreeMap<&'a str, Node>,
}
impl<'a> Index<'a> {
    fn new(program: &'a Program, sources: &Sources) -> Self {
        let definitions = program
            .modules
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition))
            .collect();
        let mut index = Self {
            definitions,
            types: BTreeMap::new(),
        };
        for class in &program.classes {
            index.types.insert(
                class.name.as_str(),
                index.node(&class.name, class.line, NodeKind::Class, sources),
            );
        }
        for enumeration in &program.enums {
            index.types.insert(
                enumeration.name.as_str(),
                index.node(&enumeration.name, enumeration.line, NodeKind::Enum, sources),
            );
        }
        index
    }
    fn node(&self, symbol: &str, line: usize, kind: NodeKind, sources: &Sources) -> Node {
        self.definitions
            .get(symbol)
            .map(|definition| definition_node(definition, sources))
            .unwrap_or_else(|| Node {
                // Direct parser clients lack module metadata. Keep this
                // fallback explicitly distinct from canonical definition IDs.
                id: format!("unresolved:{}:{symbol}", kind.as_str()),
                kind,
                label: symbol.into(),
                qualified_name: symbol.into(),
                module: None,
                source: source_location(sources, line),
            })
    }
    fn name<'b>(&'b self, symbol: &'b str) -> &'b str {
        self.definitions
            .get(symbol)
            .map_or(symbol, |definition| definition.id.name.as_str())
    }
    fn display(&self, ty: &Type) -> String {
        if ty.0 == "Option" && ty.1.len() == 1 {
            return format!("{}?", self.display(&ty.1[0]));
        }
        let name = self.name(&ty.0);
        if ty.1.is_empty() {
            name.into()
        } else {
            format!(
                "{name}[{}]",
                ty.1.iter()
                    .map(|argument| self.display(argument))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
    fn references(&self, ty: &Type) -> bool {
        self.types.contains_key(ty.0.as_str())
            || ty.1.iter().any(|argument| self.references(argument))
    }
    fn dependencies(
        &self,
        graph: &mut Graph,
        from: &str,
        prefix: &str,
        ty: &Type,
        return_type: bool,
    ) {
        self.visit(
            graph,
            from,
            &format!("{prefix}: {}", self.display(ty)),
            ty,
            if return_type {
                EdgeKind::Returns
            } else {
                EdgeKind::Owns
            },
            false,
            &[],
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn visit(
        &self,
        graph: &mut Graph,
        from: &str,
        label: &str,
        ty: &Type,
        mut kind: EdgeKind,
        mut wrapped: bool,
        path: &[String],
    ) {
        if let Some(target) = self.types.get(ty.0.as_str()) {
            graph.add_edge(Edge {
                from: from.into(),
                to: target.id.clone(),
                kind,
                label: Some(if path.is_empty() {
                    label.into()
                } else {
                    format!("{label} ({})", path.join(" / "))
                }),
            });
        }
        // Resource/function generic parameters describe a signature, not an
        // owned runtime instance. Preserve the dependency without inventing
        // actor state, message delivery, or captured values.
        let signature = matches!(ty.0.as_str(), "fn" | "Future")
            || self
                .definitions
                .get(ty.0.as_str())
                .is_some_and(|definition| definition.id.kind == DefKind::Resource);
        if signature {
            kind = EdgeKind::Uses;
            wrapped = true;
        } else if kind != EdgeKind::Returns && !wrapped {
            match ty.0.as_str() {
                "view" => {
                    kind = EdgeKind::Borrows;
                    wrapped = true;
                }
                "shared" => {
                    kind = EdgeKind::Shares;
                    wrapped = true;
                }
                // Explicit ownership retains the normal owning relation;
                // an outer borrow/share still applies to its inner spelling.
                "owned" => kind = EdgeKind::Owns,
                _ => {}
            }
        }
        for (position, argument) in ty.1.iter().enumerate() {
            let role = match ty.0.as_str() {
                "Result" if position == 0 => "Ok".into(),
                "Result" if position == 1 => "Err".into(),
                "Option" => "optional".into(),
                "List" => "element".into(),
                "Map" if position == 0 => "key".into(),
                "Map" if position == 1 => "value".into(),
                "view" | "owned" | "shared" => ty.0.clone(),
                _ => format!("{} type argument {}", self.name(&ty.0), position + 1),
            };
            let mut nested = path.to_vec();
            nested.push(role);
            self.visit(graph, from, label, argument, kind, wrapped, &nested);
        }
    }
}

fn add_definition(graph: &mut Graph, node: Node) {
    if let Some(module) = &node.module {
        graph.add_group(Group {
            id: format!("module:{module}"),
            label: module
                .strip_prefix("stdlib:")
                .unwrap_or_else(|| {
                    Path::new(module)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(module)
                })
                .into(),
            nodes: vec![node.id.clone()],
        });
    }
    graph.add_node(node);
}

fn signatures(lines: impl Iterator<Item = String>, count: usize) -> Vec<String> {
    let mut shown: Vec<_> = lines.take(DISPLAY_FIELDS).collect();
    if count > DISPLAY_FIELDS {
        shown.push(format!("… {} more", count - DISPLAY_FIELDS));
    }
    shown
}

/// Declared type relationships only; no runtime/dataflow or instance inference.
pub fn types(program: &Program, sources: &Sources) -> Graph {
    let index = Index::new(program, sources);
    let mut graph = Graph::new();
    graph.warnings.push("Type relationships describe declarations; runtime instances and data flow are not inferred.".into());
    if program.modules.is_empty() && (!program.classes.is_empty() || !program.enums.is_empty()) {
        graph.warnings.push("This parsed program has no module metadata; definition identities are local to this program.".into());
    }
    for class in &program.classes {
        let mut node = index.types[class.name.as_str()].clone();
        let mut lines = vec![node.label.clone()];
        lines.extend(signatures(
            class
                .fields
                .iter()
                .map(|(name, ty)| format!("{name}: {}", index.display(ty))),
            class.fields.len(),
        ));
        node.label = lines.join("\n");
        add_definition(&mut graph, node.clone());
        for (name, ty) in &class.fields {
            index.dependencies(&mut graph, &node.id, name, ty, false);
        }
    }
    for enumeration in &program.enums {
        let mut node = index.types[enumeration.name.as_str()].clone();
        let mut lines = vec![node.label.clone()];
        lines.extend(signatures(
            enumeration.variants.iter().map(|variant| {
                if variant.fields.is_empty() {
                    variant.name.clone()
                } else {
                    format!(
                        "{}({})",
                        variant.name,
                        variant
                            .fields
                            .iter()
                            .map(|(name, ty)| format!("{name}: {}", index.display(ty)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }),
            enumeration.variants.len(),
        ));
        node.label = lines.join("\n");
        add_definition(&mut graph, node.clone());
        for variant in &enumeration.variants {
            for (name, ty) in &variant.fields {
                index.dependencies(
                    &mut graph,
                    &node.id,
                    &format!("{}.{}", variant.name, name),
                    ty,
                    false,
                );
            }
        }
    }
    for function in &program.functions {
        if !index.references(&function.ret)
            && !function.params.iter().any(|(_, ty)| index.references(ty))
        {
            continue;
        }
        let mut node = index.node(&function.name, function.line, NodeKind::Function, sources);
        let provenance = if function.external {
            "extern Rust declaration"
        } else {
            match sources
                .location(function.line)
                .and_then(|location| location.path.extension())
                .and_then(|extension| extension.to_str())
            {
                Some("nagi") => "High function",
                Some("low") => "Low function",
                _ => "function",
            }
        };
        node.label = format!(
            "{}\n{provenance}\n({}) -> {}",
            node.label,
            function
                .params
                .iter()
                .map(|(name, ty)| format!("{name}: {}", index.display(ty)))
                .collect::<Vec<_>>()
                .join(", "),
            index.display(&function.ret)
        );
        add_definition(&mut graph, node.clone());
        for (name, ty) in &function.params {
            index.dependencies(
                &mut graph,
                &node.id,
                &format!("parameter {name}"),
                ty,
                false,
            );
        }
        index.dependencies(&mut graph, &node.id, "return", &function.ret, true);
    }
    graph.normalize();
    graph
}
