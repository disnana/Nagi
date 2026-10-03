use nagic::graph::{html, Edge, EdgeKind, Graph, Group, Node, NodeKind, SourceLocation};
use serde_json::Value;

fn node(id: &str, file: Option<&str>) -> Node {
    Node {
        id: id.into(),
        kind: NodeKind::Class,
        label: "Order\nitem: str".into(),
        qualified_name: "orders::Order".into(),
        module: Some("orders".into()),
        source: file.map(|file| SourceLocation {
            file: file.into(),
            line: 12,
            column: Some(3),
        }),
    }
}

fn embedded(page: &str) -> (&str, Value) {
    let json = page
        .split_once("<script id=\"graph-data\" type=\"application/json\">")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    (json, serde_json::from_str(json).unwrap())
}

#[test]
fn hostile_unicode_graph_data_remains_json_and_cannot_close_the_script() {
    let hostile = "</script><script>globalThis.attacked=true</script><img src=x onerror=alert(1)> & 凪😀 \u{2028}\u{2029} __GRAPH_JSON__ __LINKS_JSON__";
    let mut graph = Graph::new();
    let mut record = node(hostile, Some("/tmp/凪 <>&.nagi"));
    record.label = hostile.into();
    record.qualified_name = hostile.into();
    graph.nodes.push(record);
    graph.groups.push(Group {
        id: hostile.into(),
        label: hostile.into(),
        nodes: vec![hostile.into()],
    });
    graph.edges.push(Edge {
        from: hostile.into(),
        to: hostile.into(),
        kind: EdgeKind::Owns,
        label: Some(hostile.into()),
    });
    graph.warnings.push(hostile.into());
    let page = html::html(&graph);
    let (json, payload) = embedded(&page);
    assert_eq!(payload["graph"], serde_json::to_value(&graph).unwrap());
    assert!(!json.contains(['<', '>', '&', '\u{2028}', '\u{2029}']));
    assert!(json.contains("\\u003c/script\\u003e"));
    assert!(json.contains("\\u2028\\u2029"));
    assert_eq!(page.matches("<script").count(), 2);
    assert_eq!(page.matches("</script>").count(), 2);
    assert!(!page.contains("<img"));
}

#[test]
fn only_physical_absolute_sources_receive_encoded_user_action_links() {
    let mut graph = Graph::new();
    graph.nodes = vec![
        node("unix", Some("/tmp/凪 \"#?%<>&.nagi")),
        node("windows", Some("C:\\Project\\a b#file.nagi")),
        node("relative", Some("orders.nagi")),
        node("url", Some("javascript:alert(1)")),
        node("remote", Some("https://example.invalid/source.nagi")),
        node("virtual", Some("stdlib:std.actor")),
        node("absent", None),
    ];
    let mut standard = node("standard", Some("/tmp/pretend-standard.nagi"));
    standard.module = Some("stdlib:std.actor".into());
    graph.nodes.push(standard);
    let mut zero = node("zero", Some("/tmp/file.nagi"));
    zero.source.as_mut().unwrap().line = 0;
    graph.nodes.push(zero);
    let page = html::html(&graph);
    let (_, payload) = embedded(&page);
    let links = payload["source_links"].as_object().unwrap();
    assert_eq!(links.len(), 2);
    assert_eq!(
        links["unix"],
        "vscode://file/tmp/%E5%87%AA%20%22%23%3F%25%3C%3E%26.nagi:12:3"
    );
    assert_eq!(
        links["windows"],
        "vscode://file/C:/Project/a%20b%23file.nagi:12:3"
    );
    assert!(!page.contains("window.location"));
    assert!(!page.contains("location.href"));
}

#[test]
fn standalone_viewer_keeps_assets_local_and_uses_safe_dom_text() {
    let page = html::html(&Graph::new());
    let (header, rest) = page.split_once("<script id=\"graph-data\"").unwrap();
    let program = rest.split_once("<script>\n").unwrap().1;
    assert!(header.contains("default-src 'none'"));
    assert!(header.contains("connect-src 'none'"));
    assert!(header.contains("base-uri 'none'"));
    assert!(!header.contains("<link"));
    assert!(!header.contains(" src="));
    assert!(!header.contains("@import"));
    assert!(!program.contains("innerHTML"));
    assert!(!program.contains("outerHTML"));
    assert!(!program.contains("insertAdjacentHTML"));
    assert!(!program.contains("eval("));
    assert!(!program.contains("fetch("));
    assert!(!program.contains("WebSocket"));
    assert!(program.contains("textContent"));
    assert!(program.contains("JSON.stringify(graph,null,2)"));
    assert!(page.contains("No nodes in this view"));
}
