use nagic::{
    check, emit,
    graph::{self, Edge, EdgeKind, Filter, Graph, Group, Node, NodeKind},
    source,
};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi type graph 凪 {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn load(&self, name: &str) -> source::Sources {
        let mut sources = source::load(&self.0.join(name), name.ends_with(".nagi")).unwrap();
        check::check(&mut sources.program)
            .unwrap_or_else(|error| panic!("{}", sources.diagnostic(&error)));
        sources
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn named<'a>(graph: &'a Graph, name: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.label.lines().next() == Some(name))
        .unwrap_or_else(|| panic!("Missing {name}: {graph:?}"))
}
fn relationship(graph: &Graph, from: &str, to: &str, kind: EdgeKind, text: &str) -> bool {
    graph.edges.iter().any(|edge| {
        edge.from == named(graph, from).id
            && edge.to == named(graph, to).id
            && edge.kind == kind
            && edge
                .label
                .as_deref()
                .is_some_and(|label| label.contains(text))
    })
}
fn no_dangling(graph: &Graph) {
    let ids: BTreeSet<_> = graph.nodes.iter().map(|node| &node.id).collect();
    assert!(graph
        .edges
        .iter()
        .all(|edge| ids.contains(&edge.from) && ids.contains(&edge.to)));
    assert!(graph
        .groups
        .iter()
        .all(|group| group.nodes.iter().all(|id| ids.contains(id))));
}

const MODEL: &str = "class Id:\n    value: i64\nclass User:\n    id: Id\n    name: str\nenum Fault:\n    Missing\n    Denied(user: User)\nclass Bundle:\n    single: owned[User]\n    shared_users: shared[List[User]]\n    outcome: Result[List[User], Fault]\n    optional: User?\n    lookup: Map[Id, List[User]]\ndef borrowed(users: view[User]) -> view[User]:\n    return users\ndef outcome(user: User) -> Result[User, Fault]:\n    return ok(user)\ndef primitive(value: i64) -> i64:\n    return value\n";

#[test]
fn declarations_preserve_field_variant_wrapper_and_signature_meanings() {
    let f = Fixture::new();
    f.write("main.nagi", MODEL);
    let sources = f.load("main.nagi");
    let graph = graph::types(&sources.program, &sources);
    assert_eq!(graph.schema_version, 1);
    assert_eq!(named(&graph, "Id").label, "Id\nvalue: i64");
    assert!(named(&graph, "Fault").label.contains("Denied(user: User)"));
    assert!(named(&graph, "Bundle").label.contains("optional: User?"));
    assert!(relationship(
        &graph,
        "Bundle",
        "User",
        EdgeKind::Owns,
        "single: owned[User]"
    ));
    assert!(relationship(
        &graph,
        "Bundle",
        "User",
        EdgeKind::Shares,
        "shared_users: shared[List[User]]"
    ));
    assert!(relationship(
        &graph,
        "Bundle",
        "User",
        EdgeKind::Owns,
        "Ok / element"
    ));
    assert!(relationship(
        &graph,
        "Bundle",
        "Fault",
        EdgeKind::Owns,
        "Err"
    ));
    assert!(relationship(
        &graph,
        "Bundle",
        "User",
        EdgeKind::Owns,
        "optional"
    ));
    assert!(relationship(&graph, "Bundle", "Id", EdgeKind::Owns, "key"));
    assert!(relationship(
        &graph,
        "Bundle",
        "User",
        EdgeKind::Owns,
        "value / element"
    ));
    assert!(relationship(
        &graph,
        "Fault",
        "User",
        EdgeKind::Owns,
        "Denied.user"
    ));
    assert!(relationship(
        &graph,
        "borrowed",
        "User",
        EdgeKind::Borrows,
        "parameter users: view[User]"
    ));
    assert!(relationship(
        &graph,
        "borrowed",
        "User",
        EdgeKind::Returns,
        "return: view[User]"
    ));
    assert!(relationship(
        &graph,
        "outcome",
        "Fault",
        EdgeKind::Returns,
        "Err"
    ));
    assert!(graph
        .nodes
        .iter()
        .all(|node| !matches!(node.label.as_str(), "i64" | "str" | "unit" | "Error")));
    assert!(!graph
        .nodes
        .iter()
        .any(|node| node.label.lines().next() == Some("primitive")));
    assert_eq!(named(&graph, "User").source.as_ref().unwrap().line, 3);
    assert_eq!(named(&graph, "User").source.as_ref().unwrap().column, None);
    assert!(graph
        .warnings
        .iter()
        .any(|warning| warning.contains("not inferred")));
    no_dangling(&graph);
    let mut reordered = sources.program.clone();
    reordered.classes.reverse();
    reordered.functions.reverse();
    assert_eq!(
        serde_json::to_string(&graph).unwrap(),
        serde_json::to_string(&graph::types(&reordered, &sources)).unwrap()
    );
}

#[test]
fn imported_same_named_types_keep_author_identity_and_original_source_lines() {
    let f = Fixture::new();
    f.write("left.nagi", "class Task:\n    id: i64\n");
    f.write("right.nagi", "class Task:\n    name: str\n");
    f.write("main.nagi", "from \"left.nagi\" import Task as LeftTask\nimport \"right.nagi\" as right\nclass Holder:\n    left: LeftTask\n    right: right.Task\n");
    let sources = f.load("main.nagi");
    let graph = graph::types(&sources.program, &sources);
    let tasks: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.label.lines().next() == Some("Task"))
        .collect();
    assert_eq!(tasks.len(), 2);
    assert_ne!(tasks[0].id, tasks[1].id);
    assert_ne!(tasks[0].qualified_name, tasks[1].qualified_name);
    assert!(tasks
        .iter()
        .all(|node| node.source.as_ref().unwrap().line == 1));
    assert!(tasks
        .iter()
        .any(|node| node.source.as_ref().unwrap().file.ends_with("left.nagi")));
    assert!(tasks
        .iter()
        .any(|node| node.source.as_ref().unwrap().file.ends_with("right.nagi")));
    let holder = named(&graph, "Holder");
    assert_eq!(holder.source.as_ref().unwrap().line, 3);
    for task in &tasks {
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.from == holder.id && edge.to == task.id));
    }
    assert!(graph
        .filtered(&Filter {
            focus: Some("Task".into()),
            ..Filter::default()
        })
        .unwrap_err()
        .contains("Ambiguous"));
    let exact = graph
        .filtered(&Filter {
            focus: Some(tasks[0].qualified_name.clone()),
            depth: Some(0),
            ..Filter::default()
        })
        .unwrap();
    assert_eq!(exact.nodes.len(), 1);
    assert_eq!(exact.nodes[0].id, tasks[0].id);
    let selected = graph
        .filtered(&Filter {
            module: holder.module.clone(),
            ..Filter::default()
        })
        .unwrap();
    assert_eq!(selected.nodes.len(), 1);
    assert_eq!(selected.nodes[0].id, holder.id);
    no_dangling(&selected);
}

#[test]
fn independent_low_preserves_type_identity_and_reports_its_physical_source() {
    let f = Fixture::new();
    f.write("main.nagi", MODEL);
    let high = f.load("main.nagi");
    let expected = graph::types(&high.program, &high);
    f.write("saved.low", &emit::low(&high.program));
    fs::remove_file(f.0.join("main.nagi")).unwrap();
    let low = f.load("saved.low");
    let actual = graph::types(&low.program, &low);
    assert_eq!(
        expected
            .nodes
            .iter()
            .map(|node| (&node.id, node.kind, &node.module, &node.qualified_name))
            .collect::<Vec<_>>(),
        actual
            .nodes
            .iter()
            .map(|node| (&node.id, node.kind, &node.module, &node.qualified_name))
            .collect::<Vec<_>>()
    );
    assert_eq!(expected.edges, actual.edges);
    assert!(actual.nodes.iter().all(|node| node
        .source
        .as_ref()
        .unwrap()
        .file
        .ends_with("saved.low")));
    assert!(named(&actual, "borrowed").label.contains("Low function"));
}

#[test]
fn resource_type_arguments_are_dependencies_without_inventing_runtime_instances() {
    let f = Fixture::new();
    f.write("main.nagi", "import std.actor as actor\nclass Context:\n    seed: i64\nenum Message:\n    Read\nclass Reply:\n    value: i64\nclass Failure:\n    code: i64\nclass Service:\n    worker: actor.Actor[Message, Reply, Failure]\n    group: actor.Supervisor[Context]\n");
    let sources = f.load("main.nagi");
    let graph = graph::types(&sources.program, &sources);
    for target in ["Message", "Reply", "Failure", "Context"] {
        assert!(relationship(
            &graph,
            "Service",
            target,
            EdgeKind::Uses,
            "type argument"
        ));
    }
    assert!(graph
        .nodes
        .iter()
        .all(|node| matches!(node.kind, NodeKind::Class | NodeKind::Enum)));
    assert!(graph.edges.iter().all(|edge| edge.kind == EdgeKind::Uses));
    no_dangling(&graph);
}

#[test]
fn field_display_limits_do_not_erase_relationships_from_later_fields() {
    let f = Fixture::new();
    let fields = (0..12)
        .map(|i| format!("    item_{i}: Leaf\n"))
        .collect::<String>();
    f.write(
        "main.nagi",
        &format!("class Leaf:\n    value: i64\nclass Large:\n{fields}"),
    );
    let sources = f.load("main.nagi");
    let graph = graph::types(&sources.program, &sources);
    assert!(!named(&graph, "Large").label.contains("item_11"));
    assert!(named(&graph, "Large").label.contains("4 more"));
    assert!(relationship(
        &graph,
        "Large",
        "Leaf",
        EdgeKind::Owns,
        "item_11"
    ));
}

#[test]
fn focused_depth_is_bounded_on_cycles_and_defaults_to_one_hop() {
    let f = Fixture::new();
    f.write("main.nagi", "class A:\n    next: List[B]\nclass B:\n    next: List[C]\nclass C:\n    next: List[D]\nclass D:\n    next: List[A]\nclass Detached:\n    ready: bool\n");
    let sources = f.load("main.nagi");
    let graph = graph::types(&sources.program, &sources);
    let labels = |graph: &Graph| {
        graph
            .nodes
            .iter()
            .map(|node| node.label.lines().next().unwrap().to_owned())
            .collect::<BTreeSet<_>>()
    };
    let focus = |depth| {
        graph
            .filtered(&Filter {
                focus: Some("A".into()),
                depth,
                ..Filter::default()
            })
            .unwrap()
    };
    assert_eq!(labels(&focus(Some(0))), BTreeSet::from(["A".to_owned()]));
    assert_eq!(
        labels(&focus(None)),
        BTreeSet::from(["A".to_owned(), "B".to_owned(), "D".to_owned()])
    );
    assert_eq!(labels(&focus(Some(2))).len(), 4);
    assert_eq!(labels(&focus(Some(usize::MAX))).len(), 4);
    for filtered in [
        focus(None),
        focus(Some(0)),
        focus(Some(2)),
        focus(Some(usize::MAX)),
    ] {
        no_dangling(&filtered);
    }
    assert!(graph
        .filtered(&Filter {
            focus: Some("Missing".into()),
            ..Filter::default()
        })
        .is_err());
    assert!(graph
        .filtered(&Filter {
            module: Some("missing.nagi".into()),
            ..Filter::default()
        })
        .is_err());
    assert!(graph
        .filtered(&Filter {
            depth: Some(2),
            ..Filter::default()
        })
        .unwrap_err()
        .contains("--focus"));
}

#[test]
fn ir_deduplicates_and_sorts_without_renderer_syntax_or_losing_isolated_types() {
    let mut graph = Graph::new();
    let make_node = |id: &str| Node {
        id: id.into(),
        kind: NodeKind::Class,
        label: id.into(),
        qualified_name: id.into(),
        module: Some("module.nagi".into()),
        source: None,
    };
    graph.add_node(make_node("B"));
    graph.add_node(make_node("A"));
    graph.add_node(make_node("A"));
    let edge = Edge {
        from: "A".into(),
        to: "B".into(),
        kind: EdgeKind::Owns,
        label: Some("value".into()),
    };
    graph.add_edge(edge.clone());
    graph.add_edge(edge);
    graph.add_group(Group {
        id: "module".into(),
        label: "module.nagi".into(),
        nodes: vec!["B".into(), "A".into(), "A".into()],
    });
    graph.add_group(Group {
        id: "module".into(),
        label: "module.nagi".into(),
        nodes: vec!["A".into()],
    });
    graph.add_node(make_node("Isolated"));
    graph.normalize();
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["A", "B", "Isolated"]
    );
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.groups.len(), 1);
    assert_eq!(graph.groups[0].nodes, ["A", "B"]);
    let json = serde_json::to_value(&graph).unwrap();
    assert_eq!(json["nodes"][0]["kind"], "class");
    assert_eq!(json["edges"][0]["kind"], "owns");
    assert!(json.get("layout").is_none());
    no_dangling(&graph);
}
