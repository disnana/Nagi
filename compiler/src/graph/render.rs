//! Text renderers bind graph identities independently of display labels.

use super::{Edge, Graph, Node, NodeKind};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

struct Cluster<'a> {
    label: &'a str,
    nodes: Vec<usize>,
}

struct Bindings<'a> {
    nodes: Vec<&'a Node>,
    clusters: Vec<Cluster<'a>>,
    indices: BTreeMap<&'a str, usize>,
    paths: Vec<String>,
}

impl<'a> Bindings<'a> {
    fn new(graph: &'a Graph) -> Self {
        let mut unique = BTreeMap::new();
        for node in &graph.nodes {
            unique.entry(node.id.as_str()).or_insert(node);
        }
        let nodes = unique.values().copied().collect::<Vec<_>>();
        let indices = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), index))
            .collect::<BTreeMap<_, _>>();
        let mut paths = (0..nodes.len())
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>();
        let mut groups = graph.groups.iter().collect::<Vec<_>>();
        groups.sort_by(|a, b| a.id.cmp(&b.id).then(a.label.cmp(&b.label)));
        let mut claimed = BTreeSet::new();
        let mut clusters = Vec::new();
        for group in groups {
            let mut members = group
                .nodes
                .iter()
                .filter_map(|id| indices.get(id.as_str()).copied())
                .collect::<Vec<_>>();
            members.sort_unstable();
            members.dedup();
            members.retain(|index| claimed.insert(*index));
            if members.is_empty() {
                continue;
            }
            let cluster = clusters.len();
            for &index in &members {
                paths[index] = format!("g{cluster}.n{index}");
            }
            clusters.push(Cluster {
                label: &group.label,
                nodes: members,
            });
        }
        Self {
            nodes,
            clusters,
            indices,
            paths,
        }
    }

    fn edges<'g>(&self, graph: &'g Graph) -> Vec<&'g Edge> {
        let mut edges = graph
            .edges
            .iter()
            .filter(|edge| {
                self.indices.contains_key(edge.from.as_str())
                    && self.indices.contains_key(edge.to.as_str())
            })
            .collect::<Vec<_>>();
        edges.sort();
        edges.dedup();
        edges
    }
}

fn edge_label(edge: &Edge) -> String {
    let kind = edge.kind.as_str().replace('_', " ");
    match edge.label.as_deref().filter(|label| !label.is_empty()) {
        Some(label) => format!("{kind}: {label}"),
        None => kind,
    }
}

fn mermaid_label(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\n' | '\r' => escaped.push_str("<br/>"),
            '\t' => escaped.push_str("&Tab;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '#' => escaped.push_str("&num;"),
            '%' => escaped.push_str("&percnt;"),
            '\\' => escaped.push_str("&bsol;"),
            '\u{60}' => escaped.push_str("&grave;"),
            value if value.is_control() => escaped.push('\u{fffd}'),
            value => escaped.push(value),
        }
    }
    escaped
}

fn d2_label(value: &str) -> String {
    // D2 double-quoted strings accept JSON escapes. Keeping every label in a
    // string prevents paths, link properties, blocks and selectors executing.
    let value = value
        .chars()
        .map(|c| {
            if c.is_control() && !matches!(c, '\n' | '\r' | '\t') {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect::<String>();
    serde_json::to_string(&value).expect("serializing a string cannot fail")
}

fn class(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Module => "module",
        NodeKind::FfiBoundary => "ffi",
        NodeKind::Type | NodeKind::Class | NodeKind::Enum => "type",
        _ => "definition",
    }
}

/// A small left-to-right diagram with literal labels and module groups.
pub fn mermaid(graph: &Graph) -> String {
    let bindings = Bindings::new(graph);
    // Mermaid's SVG text renderer decodes entities differently for nodes,
    // clusters and edges. HTML labels consistently decode literal escapes;
    // every input markup character is escaped, including Markdown backticks
    // and directive prefixes. Only renderer-owned line breaks become markup.
    let mut output = String::from("%%{init: {\"securityLevel\":\"strict\",\"flowchart\":{\"htmlLabels\":true}}}%%\nflowchart LR\n");
    if bindings.nodes.is_empty() {
        output.push_str("  empty[\"No matching definitions\"]\n");
        return output;
    }
    let mut clustered = BTreeSet::new();
    for (index, cluster) in bindings.clusters.iter().enumerate() {
        let _ = writeln!(
            output,
            "  subgraph g{index}[\"{}\"]",
            mermaid_label(cluster.label)
        );
        for &index in &cluster.nodes {
            clustered.insert(index);
            let _ = writeln!(
                output,
                "    n{index}[\"{}\"]",
                mermaid_label(&bindings.nodes[index].label)
            );
        }
        output.push_str("  end\n");
    }
    for (index, node) in bindings.nodes.iter().enumerate() {
        if !clustered.contains(&index) {
            let _ = writeln!(output, "  n{index}[\"{}\"]", mermaid_label(&node.label));
        }
    }
    for edge in bindings.edges(graph) {
        let from = bindings.indices[edge.from.as_str()];
        let to = bindings.indices[edge.to.as_str()];
        let _ = writeln!(
            output,
            "  n{from} -->|\"{}\"| n{to}",
            mermaid_label(&edge_label(edge))
        );
    }
    output.push_str("  classDef module fill:#F1F5F9,stroke:#64748B,color:#0F172A;\n  classDef type fill:#EFF6FF,stroke:#3B82F6,color:#172554;\n  classDef definition fill:#F0FDF4,stroke:#16A34A,color:#14532D;\n  classDef ffi fill:#FFF7ED,stroke:#EA580C,color:#7C2D12;\n");
    for (index, node) in bindings.nodes.iter().enumerate() {
        let _ = writeln!(output, "  class n{index} {};", class(node.kind));
    }
    output
}

fn d2_node(output: &mut String, node: &Node, index: usize, indent: &str) {
    let (fill, stroke) = match class(node.kind) {
        "module" => ("#F1F5F9", "#64748B"),
        "type" => ("#EFF6FF", "#3B82F6"),
        "ffi" => ("#FFF7ED", "#EA580C"),
        _ => ("#F0FDF4", "#16A34A"),
    };
    let _ = writeln!(output, "{indent}n{index}: {} {{", d2_label(&node.label));
    let _ = writeln!(output, "{indent}  shape: rectangle\n{indent}  style.fill: \"{fill}\"\n{indent}  style.stroke: \"{stroke}\"\n{indent}  style.border-radius: 6");
    if node.kind == NodeKind::FfiBoundary {
        let _ = writeln!(output, "{indent}  style.stroke-dash: 3");
    }
    let _ = writeln!(output, "{indent}}}");
}

/// D2 containers represent the groups supplied by static analysis. Edges
/// bind to actual nested node paths, rather than duplicating group crossings.
pub fn d2(graph: &Graph) -> String {
    let bindings = Bindings::new(graph);
    let mut output = String::from("direction: right\n");
    if bindings.nodes.is_empty() {
        output.push_str("empty: \"No matching definitions\"\n");
        return output;
    }
    let mut clustered = BTreeSet::new();
    for (index, cluster) in bindings.clusters.iter().enumerate() {
        let _ = writeln!(output, "g{index}: {} {{", d2_label(cluster.label));
        output.push_str(
            "  direction: right\n  style.fill: \"#FAFAFA\"\n  style.stroke: \"#CBD5E1\"\n",
        );
        for &index in &cluster.nodes {
            clustered.insert(index);
            d2_node(&mut output, bindings.nodes[index], index, "  ");
        }
        output.push_str("}\n");
    }
    for (index, node) in bindings.nodes.iter().enumerate() {
        if !clustered.contains(&index) {
            d2_node(&mut output, node, index, "");
        }
    }
    for edge in bindings.edges(graph) {
        let from = &bindings.paths[bindings.indices[edge.from.as_str()]];
        let to = &bindings.paths[bindings.indices[edge.to.as_str()]];
        let _ = writeln!(output, "{from} -> {to}: {}", d2_label(&edge_label(edge)));
    }
    output
}

struct TempSource(PathBuf);
impl Drop for TempSource {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn temporary_source(source: &str) -> Result<TempSource, String> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..64 {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nagi-map-{}-{stamp}-{sequence}.d2",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                let source_file = TempSource(path);
                let result = file.write_all(source.as_bytes());
                drop(file);
                result.map_err(|error| format!("failed to write temporary D2 source: {error}"))?;
                return Ok(source_file);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("failed to create temporary D2 source: {error}")),
        }
    }
    Err("could not create a unique temporary D2 source".into())
}

fn bounded_stderr(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let mut result = String::new();
    for character in text.chars() {
        if result.len() + character.len_utf8() > 4096 {
            result.push('…');
            break;
        }
        if !character.is_control() || matches!(character, '\n' | '\t') {
            result.push(character);
        }
    }
    result.trim().to_owned()
}

/// Export with an installed D2 executable. This function invokes no shell and
/// passes only an allowlisted layout, a private source file and an absolute
/// SVG/PNG destination. TALA availability belongs to the installed D2 version.
pub fn image(graph: &Graph, output: &Path, format: &str, layout: &str) -> Result<(), String> {
    if !matches!(format, "svg" | "png") {
        return Err("image format must be svg or png".into());
    }
    if !matches!(layout, "elk" | "dagre" | "tala") {
        return Err("D2 layout must be elk, dagre or tala".into());
    }
    if !output.is_absolute() {
        return Err("image output path must be absolute".into());
    }
    if output.extension().and_then(|extension| extension.to_str()) != Some(format) {
        return Err(format!("{format} output must have a .{format} extension"));
    }
    let source = temporary_source(&d2(graph))?;
    let mut child = Command::new("d2")
        .arg("--layout").arg(layout).arg("--").arg(&source.0).arg(output)
        .stdout(Stdio::null()).stderr(Stdio::piped())
        .spawn()
        .map_err(|error| if error.kind() == std::io::ErrorKind::NotFound {
            "D2 was not found on PATH. Install D2 to export SVG/PNG, or use --format d2 to save the diagram source.".into()
        } else { format!("failed to start D2: {error}") })?;
    // Drain the renderer's pipe without retaining unbounded diagnostics. Its
    // stdout is unused when a destination file is supplied.
    let mut pipe = child.stderr.take().expect("configured stderr pipe");
    let mut stderr = Vec::with_capacity(4097);
    let mut buffer = [0; 4096];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(length) => {
                let retained = length.min(4097_usize.saturating_sub(stderr.len()));
                stderr.extend_from_slice(&buffer[..retained]);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("failed to read D2 diagnostics: {error}"));
            }
        }
    }
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for D2: {error}"))?;
    if !status.success() {
        let stderr = bounded_stderr(&stderr);
        return Err(if stderr.is_empty() {
            format!("D2 failed ({status})")
        } else {
            format!("D2 failed ({status}): {stderr}")
        });
    }
    if !fs::metadata(output).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0) {
        return Err("D2 reported success but did not produce an image".into());
    }
    Ok(())
}
