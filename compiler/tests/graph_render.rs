use nagic::graph::{render, Edge, EdgeKind, Graph, Group, Node, NodeKind};
use std::path::Path;

fn node(id: &str, label: &str, kind: NodeKind) -> Node {
    Node {
        id: id.into(),
        kind,
        label: label.into(),
        qualified_name: id.into(),
        module: None,
        source: None,
    }
}

fn fixture() -> Graph {
    Graph {
        nodes: vec![
            node(
                "id_a\"[unsafe]",
                "凪 [Class]\n\"quoted\" \\ <script>alert(1)</script>\nlink: javascript:bad",
                NodeKind::Class,
            ),
            node("id_b::$()", "helper\nLow function", NodeKind::Function),
            node("id_c#ffi", "crate::helper\nRust FFI", NodeKind::FfiBoundary),
        ],
        edges: vec![
            Edge {
                from: "id_a\"[unsafe]".into(),
                to: "id_b::$()".into(),
                kind: EdgeKind::Owns,
                label: Some("field [view] \"x\"".into()),
            },
            Edge {
                from: "id_b::$()".into(),
                to: "id_c#ffi".into(),
                kind: EdgeKind::Calls,
                label: None,
            },
        ],
        groups: vec![
            Group {
                id: "group_a'unsafe".into(),
                label: "High module [認証]".into(),
                nodes: vec!["id_a\"[unsafe]".into()],
            },
            Group {
                id: "group_b\\".into(),
                label: "Low \"core\"".into(),
                nodes: vec!["id_b::$()".into()],
            },
        ],
        ..Graph::new()
    }
}

#[test]
fn labels_preserve_unicode_without_becoming_diagram_syntax_or_links() {
    let graph = fixture();
    let mermaid = render::mermaid(&graph);
    assert!(mermaid.contains("凪 [Class]<br/>&quot;quoted&quot;"));
    assert!(mermaid.contains("&lt;script&gt;"));
    assert!(!mermaid.contains("<script>"));
    assert!(!mermaid.contains("id_a"));
    assert!(mermaid.contains("\"securityLevel\":\"strict\""));
    assert!(mermaid.contains("\"htmlLabels\":true"));
    assert!(mermaid.contains("class n2 ffi;"));
    assert!(!mermaid
        .lines()
        .any(|line| line.trim_start().starts_with("click ")));

    let d2 = render::d2(&graph);
    assert!(d2.contains("凪 [Class]\\n\\\"quoted\\\" \\\\"));
    assert!(d2.contains("field [view] \\\"x\\\""));
    assert!(!d2.contains("id_a"));
    assert!(!d2
        .lines()
        .any(|line| line.trim_start().starts_with("link:")));
    assert!(d2.contains("style.stroke-dash: 3"));
}

#[test]
fn mermaid_literal_hashes_backticks_and_directives_cannot_change_the_render_config() {
    let mut graph = Graph::new();
    graph.nodes.push(node(
        "unsafe",
        "\u{60}label\u{60} #34; %%{init: {\"securityLevel\":\"loose\"}}%% <a href=\"https://example.invalid\">link</a>",
        NodeKind::Function,
    ));
    let output = render::mermaid(&graph);
    assert!(output.contains("&grave;label&grave; &num;34;"));
    assert!(output.contains(
        "&percnt;&percnt;{init: {&quot;securityLevel&quot;:&quot;loose&quot;}}&percnt;&percnt;"
    ));
    assert!(output.contains("&lt;a href=&quot;https://example.invalid&quot;&gt;"));
    assert_eq!(output.matches("%%{init:").count(), 1);
    assert!(!output.contains("<a "));
    assert!(!output.contains("#34;"));
}

#[test]
fn diagram_order_is_deterministic_and_empty_graphs_are_readable() {
    let graph = fixture();
    let mut reordered = graph.clone();
    reordered.nodes.reverse();
    reordered.edges.reverse();
    reordered.groups.reverse();
    assert_eq!(render::mermaid(&graph), render::mermaid(&reordered));
    assert_eq!(render::d2(&graph), render::d2(&reordered));
    let empty = Graph::new();
    assert!(render::mermaid(&empty).contains("No matching definitions"));
    assert_eq!(
        render::d2(&empty),
        "direction: right\nempty: \"No matching definitions\"\n"
    );
}

#[test]
fn clustered_edges_reference_declared_nodes_without_group_boundary_duplicates() {
    let mut graph = fixture();
    graph.groups[1].nodes.push(graph.nodes[0].id.clone());
    graph.edges.push(Edge {
        from: "missing".into(),
        to: graph.nodes[0].id.clone(),
        kind: EdgeKind::Uses,
        label: None,
    });
    graph.edges.push(graph.edges[0].clone());
    let d2 = render::d2(&graph);
    let endpoints = ["g0.n0", "g1.n1", "n2"];
    let edges = d2
        .lines()
        .filter(|line| line.contains(" -> "))
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), 2);
    for edge in edges {
        let (from, remainder) = edge.split_once(" -> ").unwrap();
        let (to, _) = remainder.split_once(": ").unwrap();
        assert!(endpoints.contains(&from));
        assert!(endpoints.contains(&to));
    }
    for id in ["n0", "n1", "n2"] {
        let declaration = format!("{id}: ");
        assert_eq!(
            d2.lines()
                .filter(|line| line.trim_start().starts_with(&declaration))
                .count(),
            1,
            "each bound node must be declared once"
        );
    }
    let mermaid = render::mermaid(&graph);
    assert_eq!(mermaid.matches(" -->|").count(), 2);
    assert_eq!(mermaid.matches("n0[\"").count(), 1);
}

#[test]
fn image_arguments_are_validated_before_starting_a_renderer() {
    let graph = Graph::new();
    let output = std::env::temp_dir().join("nagi-invalid-output.svg");
    assert!(render::image(&graph, &output, "pdf", "elk")
        .unwrap_err()
        .contains("svg or png"));
    assert!(render::image(&graph, &output, "svg", "elk --watch")
        .unwrap_err()
        .contains("layout"));
    assert!(
        render::image(&graph, Path::new("relative.svg"), "svg", "elk")
            .unwrap_err()
            .contains("absolute")
    );
    assert!(
        render::image(&graph, &output.with_extension("png"), "svg", "elk")
            .unwrap_err()
            .contains(".svg")
    );
}

#[cfg(unix)]
mod process_tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "nagi-render-test-{}-{stamp}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn isolated_export(fail: bool, missing: bool) {
        let scratch = Scratch::new();
        let bin = scratch.0.join("bin");
        let temp = scratch.0.join("tmp");
        fs::create_dir(&bin).unwrap();
        fs::create_dir(&temp).unwrap();
        let log = scratch.0.join("arguments.txt");
        let output = scratch.0.join("result $(touch INJECTED) [diagram];.svg");
        if !missing {
            let executable = bin.join("d2");
            fs::write(&executable, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$NAGI_FAKE_D2_LOG\"\nif [ \"$NAGI_FAKE_D2_FAIL\" = 1 ]; then printf 'fake D2 rejected layout\\n' >&2; exit 3; fi\nfor argument; do output=\"$argument\"; done\nprintf '<svg xmlns=\"http://www.w3.org/2000/svg\"/>' > \"$output\"\n").unwrap();
            fs::set_permissions(executable, fs::Permissions::from_mode(0o700)).unwrap();
        }
        // A separate test process owns PATH/TMPDIR. Other concurrent tests in
        // the parent process keep their environment and renderer untouched.
        let status = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("process_tests::image_child")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env("PATH", &bin)
            .env("TMPDIR", &temp)
            .env("NAGI_RENDER_CHILD_OUTPUT", &output)
            .env("NAGI_FAKE_D2_LOG", &log)
            .env("NAGI_FAKE_D2_FAIL", if fail { "1" } else { "0" })
            .env("NAGI_FAKE_D2_MISSING", if missing { "1" } else { "0" })
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            fs::read_dir(&temp).unwrap().count(),
            0,
            "temporary source must be cleaned after success and failure"
        );
        assert!(!scratch.0.join("INJECTED").exists());
        if missing {
            assert!(!log.exists());
            return;
        }
        let arguments = fs::read_to_string(&log).unwrap();
        let arguments = arguments.lines().collect::<Vec<_>>();
        assert_eq!(arguments.len(), 5);
        assert_eq!(&arguments[..3], &["--layout", "elk", "--"]);
        assert!(Path::new(arguments[3]).starts_with(&temp));
        assert_eq!(Path::new(arguments[3]).extension().unwrap(), "d2");
        assert!(!Path::new(arguments[3]).exists());
        assert_eq!(arguments[4], output.to_str().unwrap());
        assert_eq!(output.exists(), !fail);
    }

    #[test]
    fn export_passes_arguments_literally_without_a_shell() {
        isolated_export(false, false);
    }
    #[test]
    fn failed_renderer_reports_stderr_and_removes_its_source() {
        isolated_export(true, false);
    }
    #[test]
    fn missing_renderer_explains_installation_and_d2_source_fallback() {
        isolated_export(false, true);
    }

    #[test]
    fn image_child() {
        let Some(output) = std::env::var_os("NAGI_RENDER_CHILD_OUTPUT") else {
            return;
        };
        let result = render::image(&fixture(), Path::new(&output), "svg", "elk");
        if std::env::var("NAGI_FAKE_D2_MISSING").as_deref() == Ok("1") {
            let message = result.unwrap_err();
            assert!(message.contains("not found on PATH"));
            assert!(message.contains("--format d2"));
        } else if std::env::var("NAGI_FAKE_D2_FAIL").as_deref() == Ok("1") {
            assert!(result.unwrap_err().contains("fake D2 rejected layout"));
        } else {
            result.unwrap();
        }
    }
}
