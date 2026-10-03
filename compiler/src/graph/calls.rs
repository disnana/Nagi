use super::{
    definition_id, definition_node, source_location, Edge, EdgeKind, Graph, Group, Node, NodeKind,
};
use crate::{
    ast::{DefKind, DefinitionInfo, Expr, Function, NameResolution, Program, Stmt, E, S},
    source::Sources,
};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

/// Only the checker's direct named targets become call edges. Function values
/// and aliases do not retain a stable target ID, so they remain warnings.
pub fn calls(program: &Program, sources: &Sources) -> Graph {
    let mut graph = Graph::new();
    let definitions: HashMap<_, _> = program
        .modules
        .definitions
        .iter()
        .map(|definition| (definition.symbol.as_str(), definition))
        .collect();
    let mut functions = HashMap::new();
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for function in &program.functions {
        let Some(definition) = definitions.get(function.name.as_str()).copied() else {
            graph.warnings.push(format!(
                "{}: function {} has no retained definition identity",
                position(sources, function.line),
                crate::modules::display_symbol(&function.name)
            ));
            continue;
        };
        if definition.id.kind != DefKind::Function {
            continue;
        }
        functions.insert(function.name.as_str(), (function, definition));
        let node = function_node(function, definition, sources);
        groups
            .entry(definition.id.module.0.clone())
            .or_default()
            .push(node.id.clone());
        graph.add_node(node);
        if function.external {
            add_ffi(&mut graph, function, definition, sources);
        }
    }
    for function in &program.functions {
        let Some((_, definition)) = functions.get(function.name.as_str()) else {
            continue;
        };
        let caller = definition_id(&definition.id);
        statements(&function.body, &mut |expression| {
            let E::Call(name, _, arguments) = &expression.kind else {
                return;
            };
            for argument in arguments {
                if argument.ty.as_ref().is_some_and(|ty| ty.0 == "fn") {
                    graph.warnings.push(format!(
                                "{}: function value passed to {} is not a direct call; callback edge not inferred",
                                position(sources, argument.line),
                            // Source binding spellings survive the CLI's Low
                            // reparse; its token spans belong to a different file.
                            sources.diagnostic(name)
                            ));
                }
            }
            match expression.resolution {
                    Some(NameResolution::Function) => {
                        if let Some((_, target)) = functions.get(name.as_str()) {
                            graph.add_edge(Edge {
                                from: caller.clone(),
                                to: definition_id(&target.id),
                                kind: EdgeKind::Calls,
                                label: None,
                            });
                        } else {
                            graph.warnings.push(format!(
                                "{}: direct function target {} has no loaded definition identity",
                                position(sources, expression.line),
                                crate::modules::display_symbol(name)
                            ));
                        }
                    }
                    Some(NameResolution::Standard) => {
                        let target = definitions.get(name.as_str()).copied().filter(|target| {
                            target.id.kind == DefKind::Function
                                && crate::stdlib::definition(&target.id)
                                && target.symbol == crate::modules::symbol(&target.id)
                        });
                        if let Some(target) = target {
                            let mut node = definition_node(target, sources);
                            node.label = format!(
                                "{}\n{} Rust API",
                                target.id.name,
                                target.id.module.0.trim_start_matches("stdlib:")
                            );
                            groups
                                .entry(target.id.module.0.clone())
                                .or_default()
                                .push(node.id.clone());
                            graph.add_node(node);
                            graph.add_edge(Edge {
                                from: caller.clone(),
                                to: definition_id(&target.id),
                                kind: EdgeKind::Calls,
                                label: Some("standard library".into()),
                            });
                        } else {
                            graph.warnings.push(format!(
                                "{}: standard call has no authoritative registry identity",
                                position(sources, expression.line)
                            ));
                        }
                    }
                    Some(NameResolution::Local) => graph.warnings.push(format!(
                        "{}: local function value or alias {} has no retained call target; no edge inferred",
                        position(sources, expression.line),
                    sources.diagnostic(name)
                    )),
                    Some(NameResolution::Builtin) => {
                        if database_builtin(name) {
                            let id = format!("builtin:{name}");
                            graph.add_node(Node {
                                id: id.clone(),
                                kind: NodeKind::Database,
                                label: format!("{name}\nDatabase operation"),
                                qualified_name: format!("builtin::{name}"),
                                module: None,
                                source: None,
                            });
                            graph.add_edge(Edge {
                                from: caller.clone(),
                                to: id.clone(),
                                kind: EdgeKind::Calls,
                                label: Some("database builtin".into()),
                            });
                            graph.add_group(Group {
                                id: "boundary:database".into(),
                                label: "Database operations".into(),
                                nodes: vec![id],
                            });
                        }
                    }
                    Some(NameResolution::Enum) => {}
                    _ => graph.warnings.push(format!(
                        "{}: unresolved call target {}; no edge inferred",
                        position(sources, expression.line),
                        crate::modules::display_symbol(name)
                    )),
                    }
        });
    }
    for (module, nodes) in groups {
        graph.add_group(Group {
            id: format!("module:{}", module),
            label: module
                .strip_prefix("stdlib:")
                .unwrap_or_else(|| {
                    Path::new(&module)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&module)
                })
                .into(),
            nodes,
        });
    }
    graph.normalize();
    graph
}

fn function_node(function: &Function, definition: &DefinitionInfo, sources: &Sources) -> Node {
    let mut node = definition_node(definition, sources);
    // A native replacement retains the generated DefId but its declaration
    // and body are in the loaded Low fragment.
    node.source = source_location(sources, function.line);
    let language = sources
        .location(function.line)
        .and_then(|location| location.path.extension())
        .and_then(|extension| extension.to_str())
        .map(|extension| match extension {
            "nagi" => "High ",
            "low" => "Low ",
            _ => "",
        })
        .unwrap_or("");
    node.label = format!(
        "{}\n{language}{}",
        definition.id.name,
        if function.external {
            "extern Rust declaration"
        } else if function.asynchronous {
            "async function"
        } else {
            "function"
        }
    );
    node
}

fn add_ffi(graph: &mut Graph, function: &Function, definition: &DefinitionInfo, sources: &Sources) {
    let target = function
        .attrs
        .iter()
        .find(|(name, _)| name == "rust")
        .map(|(_, target)| target.as_str())
        .unwrap_or("unresolved native target");
    let from = definition_id(&definition.id);
    let boundary = format!("ffi:{from}");
    graph.add_node(Node {
        id: boundary.clone(),
        kind: NodeKind::FfiBoundary,
        label: format!("Rust: {target}"),
        qualified_name: format!(
            "{}::{} -> {target}",
            definition.id.module.0, definition.id.name
        ),
        module: Some(definition.id.module.0.clone()),
        source: source_location(sources, function.line),
    });
    graph.add_edge(Edge {
        from,
        to: boundary,
        kind: EdgeKind::Calls,
        label: Some("extern Rust boundary".into()),
    });
}

fn position(sources: &Sources, line: usize) -> String {
    sources
        .location(line)
        .map(|location| format!("{}:{}", location.path.display(), location.line))
        .unwrap_or_else(|| format!("line {line}"))
}

fn database_builtin(name: &str) -> bool {
    matches!(
        name,
        "db_open" | "db_exec" | "db_all" | "db_query" | "db_write" | "db_insert" | "db_update"
    )
}

pub(super) fn statements(body: &[Stmt], visit: &mut impl FnMut(&Expr)) {
    for statement in body {
        match &statement.kind {
            S::Assign { value, .. } | S::Expr(value) | S::Spawn(value) => expr(value, visit),
            S::Return(value) => {
                if let Some(value) = value {
                    expr(value, visit);
                }
            }
            S::If(condition, yes, no) => {
                expr(condition, visit);
                statements(yes, visit);
                statements(no, visit);
            }
            S::Match(value, arms) => {
                expr(value, visit);
                for arm in arms {
                    statements(&arm.body, visit);
                }
            }
            S::While(condition, body) | S::For(_, condition, body) => {
                expr(condition, visit);
                statements(body, visit);
            }
            S::Scope(body) => statements(body, visit),
        }
    }
}

fn expr(expression: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(expression);
    match &expression.kind {
        E::Binary(left, _, right) | E::Index(left, right) => {
            expr(left, visit);
            expr(right, visit);
        }
        E::Unary(_, value) | E::Field(value, _) | E::Await(value) | E::Try(value) => {
            expr(value, visit)
        }
        E::Call(_, _, values) | E::List(values) => {
            for value in values {
                expr(value, visit);
            }
        }
        E::Record(_, fields) => {
            for (_, value) in fields {
                expr(value, visit);
            }
        }
        E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null | E::Name(_) => {}
    }
}
