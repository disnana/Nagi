use nagic::{
    ast::{DefId, DefKind, ModuleId},
    check, emit,
    graph::{self, EdgeKind, Graph, Node, NodeKind},
    parser, source,
};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi graph relations 凪 {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn module(&self, name: &str) -> ModuleId {
        ModuleId(
            fs::canonicalize(self.0.join(name))
                .unwrap()
                .display()
                .to_string(),
        )
    }
    fn definition(&self, file: &str, name: &str) -> String {
        graph::definition_id(&DefId {
            module: self.module(file),
            kind: DefKind::Function,
            name: name.into(),
        })
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
fn node<'a>(graph: &'a Graph, id: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("Missing {id}: {graph:?}"))
}
fn edge(graph: &Graph, from: &str, to: &str, kind: EdgeKind) -> bool {
    graph
        .edges
        .iter()
        .any(|edge| edge.from == from && edge.to == to && edge.kind == kind)
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

#[test]
fn module_edges_use_exact_identities_and_recover_empty_flat_imports() {
    let f = Fixture::new();
    f.write("left/orders.nagi", "def score() -> i64:\n    return 1\n");
    f.write("right/orders.nagi", "def score() -> i64:\n    return 2\n");
    f.write(
        "empty.nagi",
        "# An imported module can have no definitions.\n",
    );
    f.write("hidden.nagi", "def hidden() -> i64:\n    return 3\n");
    f.write("facade.nagi", "import \"hidden.nagi\" as private_module\ndef answer() -> i64:\n    return private_module.hidden()\n");
    f.write("main.nagi", "import \"left/orders.nagi\" as left\nimport \"right/orders.nagi\" as right\nfrom \"left/orders.nagi\" import score as selected\nimport \"empty.nagi\"\nimport \"facade.nagi\" as facade\nimport std.http.server as http\nimport std.actor as actor\ndef main():\n    print(left.score() + right.score() + selected() + facade.answer())\n");
    let sources = f.load("main.nagi");
    let graph = graph::modules(&sources.program, &sources);
    let root = graph::module_id(&f.module("main.nagi"));
    let left = graph::module_id(&f.module("left/orders.nagi"));
    let right = graph::module_id(&f.module("right/orders.nagi"));
    let empty = graph::module_id(&f.module("empty.nagi"));
    let facade = graph::module_id(&f.module("facade.nagi"));
    let hidden = graph::module_id(&f.module("hidden.nagi"));
    assert_ne!(left, right);
    assert_eq!(node(&graph, &left).label, "High: orders.nagi");
    assert_eq!(node(&graph, &right).label, "High: orders.nagi");
    assert!(edge(&graph, &root, &left, EdgeKind::DependsOn));
    assert!(edge(&graph, &root, &right, EdgeKind::DependsOn));
    assert!(edge(&graph, &root, &empty, EdgeKind::DependsOn));
    assert!(graph.edges.iter().any(|edge| edge.from == root
        && edge.to == empty
        && edge.label.as_deref() == Some("flat import")));
    assert!(edge(&graph, &facade, &hidden, EdgeKind::DependsOn));
    assert!(!edge(&graph, &root, &hidden, EdgeKind::DependsOn));
    assert!(edge(&graph, &root, &left, EdgeKind::Uses));
    assert!(edge(&graph, &root, &right, EdgeKind::Uses));
    for name in ["std.http.server", "std.actor"] {
        let id = nagic::stdlib::module(name).unwrap();
        assert!(edge(
            &graph,
            &root,
            &graph::module_id(&id),
            EdgeKind::DependsOn
        ));
        assert!(node(&graph, &graph::module_id(&id)).source.is_none());
        assert_eq!(
            graph::resolve_module_filter(&sources.program, &sources, name).unwrap(),
            id.0
        );
    }
    assert_eq!(
        graph::resolve_module_filter(&sources.program, &sources, "left").unwrap(),
        f.module("left/orders.nagi").0
    );
    for selector in ["orders.nagi", "selected", "private_module"] {
        assert!(graph::resolve_module_filter(&sources.program, &sources, selector).is_err());
    }
    assert!(graph.warnings.is_empty(), "{graph:?}");
    no_dangling(&graph);
}

#[test]
fn module_imports_are_read_from_loaded_overlay_text() {
    let f = Fixture::new();
    f.write("helper.nagi", "def answer() -> i64:\n    return 42\n");
    f.write("main.nagi", "def main():\n    print(0)\n");
    let path = fs::canonicalize(f.0.join("main.nagi")).unwrap();
    let overlays = HashMap::from([(
        path,
        "import \"helper.nagi\" as helper\ndef main():\n    print(helper.answer())\n".into(),
    )]);
    let mut sources = source::load_with_overlays(&f.0.join("main.nagi"), true, &overlays).unwrap();
    check::check(&mut sources.program).unwrap();
    let graph = graph::modules(&sources.program, &sources);
    assert!(graph
        .edges
        .iter()
        .any(|edge| edge.from == graph::module_id(&f.module("main.nagi"))
            && edge.to == graph::module_id(&f.module("helper.nagi"))
            && edge.label.as_deref() == Some("namespace import")));
    assert!(!fs::read_to_string(f.0.join("main.nagi"))
        .unwrap()
        .contains("import"));
    assert!(graph.warnings.is_empty());
    no_dangling(&graph);
}

#[test]
fn one_line_low_import_references_do_not_hide_body_uses() {
    let f = Fixture::new();
    f.write("helper.low", "fn answer() -> i64 { return 42; }\n");
    f.write(
        "main.low",
        "from \"helper.low\" import answer; fn main() -> unit { print(answer()); }\n",
    );
    let sources = f.load("main.low");
    let graph = graph::modules(&sources.program, &sources);
    assert!(edge(
        &graph,
        &graph::module_id(&f.module("main.low")),
        &graph::module_id(&f.module("helper.low")),
        EdgeKind::Uses
    ));
    f.write(
        "unused.low",
        "from \"helper.low\" import answer; fn main() -> unit { print(0); }\n",
    );
    let unused = f.load("unused.low");
    let graph = graph::modules(&unused.program, &unused);
    assert!(!edge(
        &graph,
        &graph::module_id(&f.module("unused.low")),
        &graph::module_id(&f.module("helper.low")),
        EdgeKind::Uses
    ));
}

#[test]
fn calls_resolve_named_aliases_without_guessing_local_function_values() {
    let f = Fixture::new();
    f.write(
        "left.nagi",
        "def score(value: i64) -> i64:\n    return value + 1\n",
    );
    f.write(
        "right.nagi",
        "def score(value: i64) -> i64:\n    return value + 2\n",
    );
    f.write("main.nagi", "import \"left.nagi\" as left\nimport \"right.nagi\" as right\nfrom \"left.nagi\" import score as selected\ndef score(value: i64) -> i64:\n    return value + 100\ndef invoke(callback: fn[i64, i64], value: i64) -> i64:\n    return callback(value)\ndef alias_only(value: i64) -> i64:\n    callback = left.score\n    return callback(value)\ndef main():\n    score = left.score\n    print(score(1))\n    print(selected(2) + left.score(3) + right.score(4))\n    print(invoke(left.score, 5))\n    print(alias_only(6))\n");
    let sources = f.load("main.nagi");
    let graph = graph::calls(&sources.program, &sources);
    let main = f.definition("main.nagi", "main");
    let left = f.definition("left.nagi", "score");
    let right = f.definition("right.nagi", "score");
    assert!(edge(&graph, &main, &left, EdgeKind::Calls));
    assert!(edge(&graph, &main, &right, EdgeKind::Calls));
    assert!(!edge(
        &graph,
        &main,
        &f.definition("main.nagi", "score"),
        EdgeKind::Calls
    ));
    assert!(!edge(
        &graph,
        &f.definition("main.nagi", "alias_only"),
        &left,
        EdgeKind::Calls
    ));
    assert!(!edge(
        &graph,
        &f.definition("main.nagi", "invoke"),
        &left,
        EdgeKind::Calls
    ));
    assert!(graph
        .warnings
        .iter()
        .any(|warning| warning.contains("local function value or alias score")));
    assert!(graph
        .warnings
        .iter()
        .any(|warning| warning.contains("callback edge not inferred")));
    assert!(graph
        .warnings
        .iter()
        .all(|warning| !warning.contains("__nagi_local_")));
    assert!(graph
        .nodes
        .iter()
        .all(|node| node.label.lines().next() != Some("print")));
    assert_eq!(node(&graph, &left).source.as_ref().unwrap().line, 1);
    assert!(node(&graph, &main).label.contains("High function"));
    no_dangling(&graph);
    // CLI mapping reparses Low and restores only source lines. Low token spans
    // must not be interpreted as original High token positions in warnings.
    let generated = emit::low_with_lines(&sources.program);
    let mut transported = parser::parse(&generated.text, false).unwrap();
    generated.restore_lines(&mut transported).unwrap();
    check::check(&mut transported).unwrap();
    let transported_graph = graph::calls(&transported, &sources);
    assert_eq!(graph.warnings, transported_graph.warnings);
    assert_eq!(graph.edges, transported_graph.edges);
}

#[test]
fn nested_calls_are_visited_but_class_and_enum_constructors_are_not_calls() {
    let f = Fixture::new();
    let scalar = [
        "assigned",
        "returned",
        "yes",
        "no",
        "loop_body",
        "iterable",
        "for_body",
        "ok_arm",
        "err_arm",
        "unary",
        "binary",
        "list_item",
        "indexer",
        "record_value",
        "field_value",
        "enum_value",
    ];
    let mut text = String::from(
        "class Envelope:\n    value: i64\nenum Outcome:\n    Empty\n    Value(value: i64)\n",
    );
    for name in scalar {
        text.push_str(&format!("def {name}() -> i64:\n    return 1\n"));
    }
    for name in ["if_condition", "while_condition"] {
        text.push_str(&format!("def {name}() -> bool:\n    return False\n"));
    }
    for name in ["matched", "tried"] {
        text.push_str(&format!(
            "def {name}() -> Result[i64, Error]:\n    return ok(1)\n"
        ));
    }
    text.push_str("async def spawned():\n    print(0)\nasync def awaited() -> i64:\n    return 1\nasync def main() -> Result[unit, Error]:\n    value = assigned()\n    if if_condition():\n        print(yes())\n    else:\n        print(no())\n    while while_condition():\n        print(loop_body())\n    for number in range(iterable()):\n        print(for_body())\n    match matched():\n        case Ok(_):\n            print(ok_arm())\n        case Err(_):\n            print(err_arm())\n    async with scope:\n        spawn spawned()\n    print(-unary() + binary())\n    print([list_item()][indexer()])\n    envelope = Envelope(value=record_value())\n    print(Envelope(value=field_value()).value)\n    outcome = Outcome.Value(enum_value())\n    empty = Outcome.Empty\n    print(await awaited())\n    print(try tried())\n    return ok(print(returned()))\n");
    f.write("main.nagi", &text);
    let sources = f.load("main.nagi");
    let graph = graph::calls(&sources.program, &sources);
    let main = f.definition("main.nagi", "main");
    for name in scalar.into_iter().chain([
        "if_condition",
        "while_condition",
        "matched",
        "tried",
        "spawned",
        "awaited",
    ]) {
        assert!(
            edge(
                &graph,
                &main,
                &f.definition("main.nagi", name),
                EdgeKind::Calls
            ),
            "Missing nested call {name}: {graph:?}"
        );
    }
    assert!(graph
        .nodes
        .iter()
        .all(|node| node.kind == NodeKind::Function));
    assert!(!graph.nodes.iter().any(|node| matches!(
        node.label.lines().next(),
        Some("Envelope" | "Outcome" | "print" | "range" | "ok")
    )));
    assert!(graph.warnings.is_empty(), "{graph:?}");
    no_dangling(&graph);
}

#[test]
fn native_replacements_keep_the_generated_id_and_low_ffi_locations() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        "def value() -> i64:\n    return 1\ndef main():\n    print(value())\n",
    );
    f.write("replacement.low", "@replace generated::value\nfn implementation() -> i64 { return bridge(); }\n@rust(\"native::bridge\")\nextern fn bridge() -> i64;\n");
    let mut sources = f.load("main.nagi");
    let mut program = std::mem::take(&mut sources.program);
    let native_sources = source::load(&f.0.join("replacement.low"), false).unwrap();
    let native = sources.append(native_sources);
    check::integrate(&mut program, native).unwrap();
    let graph = graph::calls(&program, &sources);
    let value = f.definition("main.nagi", "value");
    let bridge = f.definition("replacement.low", "bridge");
    let boundary = format!("ffi:{bridge}");
    assert!(edge(
        &graph,
        &f.definition("main.nagi", "main"),
        &value,
        EdgeKind::Calls
    ));
    assert!(edge(&graph, &value, &bridge, EdgeKind::Calls));
    assert!(edge(&graph, &bridge, &boundary, EdgeKind::Calls));
    assert!(node(&graph, &value).label.contains("Low function"));
    assert!(node(&graph, &value)
        .source
        .as_ref()
        .unwrap()
        .file
        .ends_with("replacement.low"));
    assert_eq!(node(&graph, &value).source.as_ref().unwrap().line, 2);
    assert_eq!(node(&graph, &bridge).source.as_ref().unwrap().line, 4);
    assert_eq!(node(&graph, &boundary).kind, NodeKind::FfiBoundary);
    assert_eq!(node(&graph, &boundary).label, "Rust: native::bridge");
    assert!(graph.warnings.is_empty(), "{graph:?}");
    no_dangling(&graph);
    let modules = graph::modules(&program, &sources);
    assert_eq!(
        node(&modules, &graph::module_id(&f.module("replacement.low"))).label,
        "Low: replacement.low"
    );
    no_dangling(&modules);
}

#[test]
fn saved_low_keeps_definition_ids_without_mapping_foreign_modules_to_its_file() {
    let f = Fixture::new();
    f.write("library.nagi", "def score() -> i64:\n    return 42\n");
    f.write(
        "main.nagi",
        "import \"library.nagi\" as library\ndef main():\n    print(library.score())\n",
    );
    let original = f.load("main.nagi");
    f.write("saved.low", &emit::low(&original.program));
    let saved = f.load("saved.low");
    let original_calls = graph::calls(&original.program, &original);
    let saved_calls = graph::calls(&saved.program, &saved);
    assert_eq!(
        original_calls
            .nodes
            .iter()
            .map(|node| &node.id)
            .collect::<Vec<_>>(),
        saved_calls
            .nodes
            .iter()
            .map(|node| &node.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(original_calls.edges, saved_calls.edges);
    for node in &saved_calls.nodes {
        assert!(node.label.contains("Low function"));
        let source = node.source.as_ref().unwrap();
        assert!(source.file.ends_with("saved.low"));
        assert!(fs::read_to_string(&source.file)
            .unwrap()
            .lines()
            .nth(source.line - 1)
            .unwrap()
            .contains("fn "));
    }
    let modules = graph::modules(&saved.program, &saved);
    let root = graph::module_id(&f.module("main.nagi"));
    let library = graph::module_id(&f.module("library.nagi"));
    assert!(node(&modules, &root)
        .source
        .as_ref()
        .unwrap()
        .file
        .ends_with("saved.low"));
    assert!(node(&modules, &library).source.is_none());
    assert!(node(&modules, &library).label.starts_with("Logical: "));
    assert!(edge(&modules, &root, &library, EdgeKind::DependsOn));
    assert!(edge(&modules, &root, &library, EdgeKind::Uses));
    assert_eq!(
        graph::resolve_module_filter(
            &saved.program,
            &saved,
            &fs::canonicalize(f.0.join("saved.low"))
                .unwrap()
                .display()
                .to_string()
        )
        .unwrap(),
        f.module("main.nagi").0
    );
    no_dangling(&saved_calls);
    no_dangling(&modules);
}

#[test]
fn checked_standard_and_database_operations_are_boundaries_not_routing_or_table_guesses() {
    let f = Fixture::new();
    f.write("main.nagi", "import std.http.server as http\nimport std.actor as actor\nasync def handler(request: http.Request, state: shared[i64]) -> Result[http.Response, Error]:\n    return ok(http.empty(http.Status.OK))\ndef main():\n    app = http.app_default[i64](1)\n    result = http.route(app, http.Method.GET, \"/items\", handler)\nasync def database() -> Result[unit, Error]:\n    database = try await db_open(\":memory:\")\n    try await db_exec(database, \"CREATE TABLE items(value INTEGER)\")\n    await actor.yield_now()\n    return ok(print(0))\n");
    let sources = f.load("main.nagi");
    let graph = graph::calls(&sources.program, &sources);
    let http = nagic::stdlib::module("std.http.server").unwrap();
    let actor = nagic::stdlib::module("std.actor").unwrap();
    for (module, name, caller) in [
        (http.clone(), "app_default", "main"),
        (http.clone(), "route", "main"),
        (http, "empty", "handler"),
        (actor, "yield_now", "database"),
    ] {
        let id = graph::definition_id(&DefId {
            module,
            kind: DefKind::Function,
            name: name.into(),
        });
        assert!(edge(
            &graph,
            &f.definition("main.nagi", caller),
            &id,
            EdgeKind::Calls
        ));
        assert!(node(&graph, &id).label.contains("Rust API"));
        assert!(node(&graph, &id).source.is_none());
    }
    for name in ["db_open", "db_exec"] {
        let id = format!("builtin:{name}");
        assert!(edge(
            &graph,
            &f.definition("main.nagi", "database"),
            &id,
            EdgeKind::Calls
        ));
        assert_eq!(node(&graph, &id).kind, NodeKind::Database);
        assert!(node(&graph, &id).label.contains("operation"));
    }
    assert!(!edge(
        &graph,
        &f.definition("main.nagi", "main"),
        &f.definition("main.nagi", "handler"),
        EdgeKind::Calls
    ));
    assert!(graph
        .warnings
        .iter()
        .any(|warning| warning.contains("callback edge not inferred")));
    assert!(graph.nodes.iter().all(|node| !matches!(
        node.kind,
        NodeKind::HttpRoute | NodeKind::Actor | NodeKind::Supervisor
    )));
    assert!(graph
        .nodes
        .iter()
        .all(|node| !node.label.contains("items(value")));
    no_dangling(&graph);

    f.write(
        "shadow.nagi",
        "def db_open(value: i64) -> i64:\n    return value\ndef main():\n    print(db_open(1))\n",
    );
    let shadow = f.load("shadow.nagi");
    let shadow_graph = graph::calls(&shadow.program, &shadow);
    assert!(edge(
        &shadow_graph,
        &f.definition("shadow.nagi", "main"),
        &f.definition("shadow.nagi", "db_open"),
        EdgeKind::Calls
    ));
    assert!(shadow_graph
        .nodes
        .iter()
        .all(|node| node.kind != NodeKind::Database));
}
