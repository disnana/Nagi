use super::{module_id, Edge, EdgeKind, Graph, Group, Node, NodeKind, SourceLocation};
use crate::{
    ast::{
        BindingTarget, DefinitionInfo, ImportKind, ImportSource, ModuleId, NameResolution, Program,
        Type, E,
    },
    source::Sources,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::{Component, Path, PathBuf},
};

/// Dependencies use retained identities, never display names or file stems.
pub fn modules(program: &Program, sources: &Sources) -> Graph {
    let mut graph = Graph::new();
    let definitions: HashMap<_, _> = program
        .modules
        .definitions
        .iter()
        .map(|definition| (definition.symbol.as_str(), definition))
        .collect();
    let physical: HashMap<_, _> = sources
        .module_files()
        .map(|(id, path)| (id.clone(), path.to_path_buf()))
        .collect();
    let loaded: HashMap<_, _> = sources
        .module_files()
        .map(|(id, path)| (normalized(path), id.clone()))
        .collect();
    let known: HashSet<_> = program
        .modules
        .modules
        .iter()
        .map(|module| module.id.clone())
        .collect();
    let mut groups = BTreeMap::<&str, Vec<String>>::new();
    for module in &program.modules.modules {
        let path = physical.get(&module.id);
        let standard = crate::stdlib::is_registered_module(&module.id);
        let (group, prefix) = if standard {
            ("Standard modules", "")
        } else if path.is_some_and(|path| path.extension().is_some_and(|ext| ext == "low")) {
            ("Low sources", "Low: ")
        } else if path.is_some() {
            ("High sources", "High: ")
        } else {
            ("Retained logical modules", "Logical: ")
        };
        let label = if standard {
            module.id.0.trim_start_matches("stdlib:").to_owned()
        } else {
            let name = path
                .map(PathBuf::as_path)
                .unwrap_or_else(|| Path::new(&module.path))
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| module.id.0.clone());
            format!("{prefix}{name}")
        };
        let id = module_id(&module.id);
        groups.entry(group).or_default().push(id.clone());
        graph.add_node(Node {
            id,
            kind: NodeKind::Module,
            label,
            qualified_name: module.id.0.clone(),
            module: Some(module.id.0.clone()),
            source: path.map(|path| SourceLocation {
                file: path.display().to_string(),
                line: 1,
                column: None,
            }),
        });
    }

    let mut dependencies = BTreeMap::<(String, String), BTreeSet<&str>>::new();
    let mut import_sites = Vec::new();
    let mut adopted = HashSet::new();
    // Loading consumes the import AST. Reparse only the already loaded text to
    // retain explicit edges for empty modules and distinguish direct imports
    // from transitive visibility. Targets must still be loaded identities.
    for (path, text, start) in sources.files() {
        let Some(owner) = loaded.get(&normalized(path)) else {
            continue;
        };
        let high = path.extension().is_some_and(|ext| ext == "nagi");
        let parsed = match crate::parser::parse(text, high) {
            Ok(parsed) => parsed,
            Err(error) => {
                graph.warnings.push(format!(
                    "Could not recover imports from loaded source {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        if !parsed.modules.is_empty() {
            adopted.insert(path.to_path_buf());
        }
        for import in parsed.module_imports {
            import_sites.push((owner.clone(), start + import.line - 1, import.span));
            let target = match import.source {
                ImportSource::Standard => crate::stdlib::module(&import.path),
                ImportSource::File => {
                    let candidate = path
                        .parent()
                        .unwrap_or_else(|| Path::new(""))
                        .join(&import.path);
                    // Resolve symlinks before removing `..`; doing the latter
                    // first can select a different, already loaded file.
                    std::fs::canonicalize(&candidate)
                        .ok()
                        .and_then(|canonical| loaded.get(&normalized(&canonical)).cloned())
                        .or_else(|| loaded.get(&candidate).cloned())
                }
            };
            let Some(target) = target.filter(|target| known.contains(target)) else {
                graph.warnings.push(format!(
                    "{}:{}: import {:?} has no matching loaded module identity",
                    path.display(),
                    import.line,
                    import.path
                ));
                continue;
            };
            let label = match import.kind {
                ImportKind::Flat => "flat import",
                ImportKind::Module { .. } => "namespace import",
                ImportKind::Names(_) => "from import",
            };
            dependency(&mut dependencies, owner, &target, label);
        }
    }

    // Low preserves logical bindings even when it no longer contains the
    // original import statements. Native integration also introduces scoped
    // bindings, which are dependencies but are not claimed to be file imports.
    for binding in &program.modules.bindings {
        let target = match &binding.target {
            BindingTarget::Module(id) => id,
            BindingTarget::Definition(id) => &id.module,
        };
        if binding.module == *target {
            continue;
        }
        if !known.contains(&binding.module) || !known.contains(target) {
            graph.warnings.push(format!(
                "Binding {} has an unavailable module identity",
                binding.name
            ));
            continue;
        }
        let key = (module_id(&binding.module), module_id(target));
        if dependencies.contains_key(&key) {
            continue;
        }
        let label = if binding.public {
            "flat visibility"
        } else {
            "binding"
        };
        dependency(&mut dependencies, &binding.module, target, label);
    }
    for ((from, to), labels) in dependencies {
        graph.add_edge(Edge {
            from,
            to,
            kind: EdgeKind::DependsOn,
            label: Some(labels.into_iter().collect::<Vec<_>>().join(", ")),
        });
    }
    for reference in &program.modules.references {
        if sources
            .location(reference.line)
            .is_some_and(|location| adopted.contains(location.path))
            || reference.module == reference.target.module
            || import_sites.iter().any(|(module, line, span)| {
                module == &reference.module
                    && *line == reference.line
                    && reference.span.start >= span.start
                    && reference.span.end <= span.end
            })
        {
            continue;
        }
        if known.contains(&reference.module) && known.contains(&reference.target.module) {
            graph.add_edge(Edge {
                from: module_id(&reference.module),
                to: module_id(&reference.target.module),
                kind: EdgeKind::Uses,
                label: None,
            });
        }
    }
    // Flattened Low reconstructs token references without their enclosing
    // module. Checked declarations retain that identity, including type uses
    // and function values which are not direct calls.
    for (name, line, types) in program
        .classes
        .iter()
        .map(|class| {
            (
                &class.name,
                class.line,
                class.fields.iter().map(|(_, ty)| ty).collect::<Vec<_>>(),
            )
        })
        .chain(program.enums.iter().map(|enumeration| {
            (
                &enumeration.name,
                enumeration.line,
                enumeration
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|(_, ty)| ty))
                    .collect(),
            )
        }))
    {
        if let Some(definition) = definitions.get(name.as_str()) {
            let owner = usage_owner(sources, &loaded, &adopted, &definition.id.module, line);
            for ty in types {
                type_uses(&mut graph, &definitions, &owner, ty);
            }
        }
    }
    for function in &program.functions {
        let Some(definition) = definitions.get(function.name.as_str()) else {
            continue;
        };
        let owner = usage_owner(
            sources,
            &loaded,
            &adopted,
            &definition.id.module,
            function.line,
        );
        for ty in function
            .params
            .iter()
            .map(|(_, ty)| ty)
            .chain(std::iter::once(&function.ret))
        {
            type_uses(&mut graph, &definitions, &owner, ty);
        }
        super::calls::statements(&function.body, &mut |expression| {
            if let Some(ty) = &expression.ty {
                type_uses(&mut graph, &definitions, &owner, ty);
            }
            if let E::Call(_, types, _) = &expression.kind {
                for ty in types {
                    type_uses(&mut graph, &definitions, &owner, ty);
                }
            }
            if matches!(
                expression.resolution,
                Some(NameResolution::Function | NameResolution::Standard)
            ) {
                if let E::Call(name, _, _) | E::Name(name) = &expression.kind {
                    if let Some(target) = definitions.get(name.as_str()) {
                        usage(&mut graph, &owner, &target.id.module);
                    }
                }
            }
        });
    }
    for (label, nodes) in groups {
        graph.add_group(Group {
            id: format!("module-sources:{label}"),
            label: label.into(),
            nodes,
        });
    }
    if program.modules.modules.is_empty() {
        graph
            .warnings
            .push("No resolved module metadata is available".into());
    }
    graph.normalize();
    graph
}

fn usage_owner(
    sources: &Sources,
    loaded: &HashMap<PathBuf, ModuleId>,
    adopted: &HashSet<PathBuf>,
    declaration: &ModuleId,
    line: usize,
) -> ModuleId {
    sources
        .location(line)
        .filter(|location| !adopted.contains(location.path))
        .and_then(|location| loaded.get(location.path))
        .unwrap_or(declaration)
        .clone()
}

fn type_uses(
    graph: &mut Graph,
    definitions: &HashMap<&str, &DefinitionInfo>,
    owner: &ModuleId,
    ty: &Type,
) {
    if let Some(target) = definitions.get(ty.0.as_str()) {
        usage(graph, owner, &target.id.module);
    }
    for argument in &ty.1 {
        type_uses(graph, definitions, owner, argument);
    }
}

fn usage(graph: &mut Graph, owner: &ModuleId, target: &ModuleId) {
    if owner != target {
        graph.add_edge(Edge {
            from: module_id(owner),
            to: module_id(target),
            kind: EdgeKind::Uses,
            label: None,
        });
    }
}

fn dependency(
    edges: &mut BTreeMap<(String, String), BTreeSet<&'static str>>,
    from: &ModuleId,
    to: &ModuleId,
    label: &'static str,
) {
    if from != to {
        edges
            .entry((module_id(from), module_id(to)))
            .or_default()
            .insert(label);
    }
}

fn normalized(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

/// CLI aliases are looked up only in the authoritative root scope.
pub fn resolve_module_filter(
    program: &Program,
    sources: &Sources,
    selector: &str,
) -> Result<String, String> {
    if let Some(module) = program
        .modules
        .modules
        .iter()
        .find(|module| module.id.0 == selector)
    {
        return Ok(module.id.0.clone());
    }
    if let Some(id) = crate::stdlib::module(selector) {
        if program.modules.modules.iter().any(|module| module.id == id) {
            return Ok(id.0);
        }
    }
    if let Some((id, _)) = sources
        .module_files()
        .find(|(_, path)| path.display().to_string() == selector)
    {
        return Ok(id.0.clone());
    }
    if let Some(root) = &program.modules.root {
        if let Some(binding) = program.modules.binding(root, selector) {
            if let BindingTarget::Module(id) = &binding.target {
                return Ok(id.0.clone());
            }
        }
    }
    Err(format!(
        "No loaded module matches {selector:?}; use a complete module ID, source path, or root module alias"
    ))
}
