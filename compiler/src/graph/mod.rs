//! Static, view-independent relationships between checked Nagi definitions.
//! Display syntax and layout belong to renderers, never to this schema.

use crate::{
    ast::{DefId, DefKind, DefinitionInfo, ModuleId, Program},
    source::Sources,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Component;

pub mod calls;
pub mod html;
pub mod modules;
pub mod render;
mod types;
pub use calls::calls;
pub use modules::{modules, resolve_module_filter};
pub use types::types;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Graph {
    pub schema_version: u32,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub groups: Vec<Group>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub qualified_name: String,
    pub module: Option<String>,
    pub source: Option<SourceLocation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Module,
    Type,
    Class,
    Enum,
    Function,
    Actor,
    Supervisor,
    Database,
    HttpRoute,
    FfiBoundary,
}
impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Type => "type",
            Self::Class => "class",
            Self::Enum => "enum",
            Self::Function => "function",
            Self::Actor => "actor",
            Self::Supervisor => "supervisor",
            Self::Database => "database",
            Self::HttpRoute => "http_route",
            Self::FfiBoundary => "ffi_boundary",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub label: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Calls,
    Contains,
    Owns,
    Borrows,
    Shares,
    Returns,
    Reads,
    Writes,
    DependsOn,
    Supervises,
    SendsMessage,
    Uses,
}
impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Calls => "calls",
            Self::Contains => "contains",
            Self::Owns => "owns",
            Self::Borrows => "borrows",
            Self::Shares => "shares",
            Self::Returns => "returns",
            Self::Reads => "reads",
            Self::Writes => "writes",
            Self::DependsOn => "depends_on",
            Self::Supervises => "supervises",
            Self::SendsMessage => "sends_message",
            Self::Uses => "uses",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Group {
    pub id: String,
    pub label: String,
    pub nodes: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub column: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub module: Option<String>,
    pub focus: Option<String>,
    /// Undirected relationship hops from focus; omitted means one hop.
    pub depth: Option<usize>,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}
impl Graph {
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            nodes: vec![],
            edges: vec![],
            groups: vec![],
            warnings: vec![],
        }
    }
    pub fn add_node(&mut self, node: Node) {
        match self.nodes.binary_search_by(|entry| entry.id.cmp(&node.id)) {
            Ok(index) => {
                let current = &mut self.nodes[index];
                // Merging a declaration view with a type view should retain
                // the field signature, independently of merge order.
                if node.label.len() > current.label.len()
                    || node.label.len() == current.label.len() && node.label < current.label
                {
                    current.label = node.label;
                }
                if current.source.is_none() {
                    current.source = node.source;
                }
                if current.module.is_none() {
                    current.module = node.module;
                }
            }
            Err(index) => self.nodes.insert(index, node),
        }
    }
    pub fn add_edge(&mut self, edge: Edge) {
        if let Err(index) = self.edges.binary_search(&edge) {
            self.edges.insert(index, edge);
        }
    }
    pub fn add_group(&mut self, mut group: Group) {
        group.nodes.sort();
        group.nodes.dedup();
        match self
            .groups
            .binary_search_by(|entry| entry.id.cmp(&group.id))
        {
            Ok(index) => {
                let current = &mut self.groups[index];
                current.nodes.extend(group.nodes);
                current.nodes.sort();
                current.nodes.dedup();
            }
            Err(index) => self.groups.insert(index, group),
        }
    }
    pub fn merge(&mut self, other: Graph) {
        for node in other.nodes {
            self.add_node(node);
        }
        for edge in other.edges {
            self.add_edge(edge);
        }
        for group in other.groups {
            self.add_group(group);
        }
        self.warnings.extend(other.warnings);
        self.normalize();
    }
    /// Also normalize graphs assembled through their public schema fields.
    pub fn normalize(&mut self) {
        let mut nodes = BTreeMap::new();
        for node in std::mem::take(&mut self.nodes) {
            nodes.entry(node.id.clone()).or_insert(node);
        }
        self.nodes = nodes.into_values().collect();
        self.edges.sort();
        self.edges.dedup();
        let mut groups = BTreeMap::<String, Group>::new();
        for mut group in std::mem::take(&mut self.groups) {
            group.nodes.sort();
            group.nodes.dedup();
            match groups.entry(group.id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(group);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let current = entry.get_mut();
                    current.nodes.extend(group.nodes);
                    current.nodes.sort();
                    current.nodes.dedup();
                }
            }
        }
        self.groups = groups.into_values().collect();
        self.warnings.sort();
        self.warnings.dedup();
    }

    pub fn filtered(&self, filter: &Filter) -> Result<Self, String> {
        if filter.depth.is_some() && filter.focus.is_none() {
            return Err("--depth requires --focus".into());
        }
        let mut graph = self.clone();
        graph.normalize();
        let module = filter
            .module
            .as_ref()
            .map(|name| {
                let matches: BTreeSet<_> = graph
                    .nodes
                    .iter()
                    .filter_map(|node| node.module.as_ref())
                    .filter(|module| {
                        *module == name || module.strip_prefix("stdlib:") == Some(name.as_str())
                    })
                    .cloned()
                    .collect();
                match matches.len() {
                    0 => Err(format!("No graph module matches {name:?}")),
                    1 => Ok(matches.into_iter().next().unwrap()),
                    _ => Err(format!(
                        "Ambiguous graph module {name:?}; use its complete module ID"
                    )),
                }
            })
            .transpose()?;
        let candidates: Vec<_> = graph
            .nodes
            .iter()
            .filter(|node| {
                module
                    .as_ref()
                    .is_none_or(|id| node.module.as_ref() == Some(id))
            })
            .collect();
        let allowed: BTreeSet<_> = candidates.iter().map(|node| node.id.clone()).collect();
        let retained = if let Some(name) = &filter.focus {
            let exact: Vec<_> = candidates
                .iter()
                .filter(|node| node.id == *name || node.qualified_name == *name)
                .collect();
            let matching: Vec<_> = if exact.is_empty() {
                candidates
                    .iter()
                    .filter(|node| {
                        node.label.lines().next() == Some(name.as_str())
                            || node.qualified_name.rsplit("::").next() == Some(name.as_str())
                    })
                    .collect()
            } else {
                exact
            };
            let focus = match matching.as_slice() {
                [] => return Err(format!("No graph node matches focus {name:?}")),
                [node] => &node.id,
                _ => {
                    let options = matching
                        .iter()
                        .map(|node| node.qualified_name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(format!("Ambiguous graph focus {name:?}; use a complete node ID or qualified name: {options}"));
                }
            };
            let mut adjacent = BTreeMap::<&str, BTreeSet<&str>>::new();
            for edge in &graph.edges {
                if allowed.contains(&edge.from) && allowed.contains(&edge.to) {
                    adjacent.entry(&edge.from).or_default().insert(&edge.to);
                    adjacent.entry(&edge.to).or_default().insert(&edge.from);
                }
            }
            let mut retained = BTreeSet::from([focus.clone()]);
            let mut queue = VecDeque::from([(focus.as_str(), 0usize)]);
            let depth = filter.depth.unwrap_or(1);
            while let Some((current, distance)) = queue.pop_front() {
                if distance >= depth {
                    continue;
                }
                for &next in adjacent.get(current).into_iter().flatten() {
                    if retained.insert(next.to_owned()) {
                        queue.push_back((next, distance + 1));
                    }
                }
            }
            retained
        } else {
            allowed
        };
        graph.nodes.retain(|node| retained.contains(&node.id));
        graph
            .edges
            .retain(|edge| retained.contains(&edge.from) && retained.contains(&edge.to));
        for group in &mut graph.groups {
            group.nodes.retain(|id| retained.contains(id));
        }
        graph.groups.retain(|group| !group.nodes.is_empty());
        Ok(graph)
    }
}

/// Preserve canonical identities even when Low retains an old logical module.
pub fn definition_id(id: &DefId) -> String {
    format!("def:{}", crate::modules::symbol(id))
}
pub fn module_id(id: &ModuleId) -> String {
    format!("module:{}", id.0)
}

/// Build labels from the complete loaded module set before selecting a view.
/// Only presentation changes: standard and retained logical IDs stay intact.
pub(super) fn module_labels(program: &Program, sources: &Sources) -> BTreeMap<String, String> {
    #[derive(Default)]
    struct Suffixes {
        count: usize,
        children: BTreeMap<String, Suffixes>,
    }
    let physical: BTreeMap<_, _> = sources
        .module_files()
        .map(|(id, path)| (id.0.as_str(), path))
        .collect();
    let modules: BTreeMap<_, _> = program
        .modules
        .modules
        .iter()
        .map(|module| (module.id.0.as_str(), &module.id))
        .collect();
    let mut path_counts = BTreeMap::new();
    for (id, module) in &modules {
        if !crate::stdlib::is_registered_module(module) {
            if let Some(path) = physical.get(id) {
                *path_counts.entry(*path).or_insert(0usize) += 1;
            }
        }
    }
    let mut labels = BTreeMap::new();
    let mut paths = Vec::new();
    let mut suffixes = Suffixes::default();
    for (id, module) in modules {
        if crate::stdlib::is_registered_module(module) {
            labels.insert(id.to_owned(), id.trim_start_matches("stdlib:").to_owned());
            continue;
        }
        let Some(path) = physical.get(id) else {
            labels.insert(id.to_owned(), id.to_owned());
            continue;
        };
        // Two logical identities sharing one physical snapshot cannot be
        // distinguished by path suffixes. Retain their authoritative names.
        if path_counts.get(path).copied().unwrap_or(0) > 1 {
            labels.insert(id.to_owned(), id.to_owned());
            continue;
        }
        let parts: Vec<_> = path
            .components()
            .filter_map(|component| match component {
                Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();
        let mut suffix = &mut suffixes;
        for part in parts.iter().rev() {
            suffix = suffix.children.entry(part.clone()).or_default();
            suffix.count += 1;
        }
        paths.push((id, *path, parts));
    }
    let fixed: BTreeSet<_> = labels.values().cloned().collect();
    for (id, path, parts) in paths {
        let mut suffix = &suffixes;
        let mut label = None;
        for (depth, part) in parts.iter().rev().enumerate() {
            suffix = &suffix.children[part];
            if suffix.count == 1 {
                let candidate = parts[parts.len() - depth - 1..].join("/");
                if !fixed.contains(&candidate) {
                    label = Some(candidate);
                    break;
                }
            }
        }
        let label = label.unwrap_or_else(|| {
            let full = path.display().to_string();
            if fixed.contains(&full) {
                id.to_owned()
            } else {
                full
            }
        });
        labels.insert(id.to_owned(), label);
    }
    labels
}

pub fn source_location(sources: &Sources, line: usize) -> Option<SourceLocation> {
    sources.location(line).map(|location| SourceLocation {
        file: location.path.display().to_string(),
        line: location.line,
        // Definitions do not retain reliable declaration token columns.
        column: None,
    })
}
pub fn definition_node(definition: &DefinitionInfo, sources: &Sources) -> Node {
    Node {
        id: definition_id(&definition.id),
        kind: match definition.id.kind {
            DefKind::Class => NodeKind::Class,
            DefKind::Enum => NodeKind::Enum,
            DefKind::Function => NodeKind::Function,
            DefKind::Resource => NodeKind::Type,
        },
        label: definition.id.name.clone(),
        qualified_name: format!("{}::{}", definition.id.module.0, definition.id.name),
        module: Some(definition.id.module.0.clone()),
        source: if crate::stdlib::is_registered_module(&definition.id.module) {
            None
        } else {
            source_location(sources, definition.line)
        },
    }
}
