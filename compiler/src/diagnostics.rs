use crate::{ast::Program, source::Sources};
use serde_json::Value;
use std::{cell::Cell, path::Path};

/// Generated lines point to the statement or definition that emitted them.
/// Synthetic glue has no origin; columns and Rust edits are never translated.
#[derive(Default)]
pub struct Generated {
    pub text: String,
    lines: Vec<Option<usize>>,
    origin: Option<usize>,
    line_open: bool,
}

impl Generated {
    pub(crate) fn new(text: &str) -> Self {
        let mut out = Self::default();
        out.push_str(text);
        out
    }

    pub(crate) fn origin(&mut self, line: Option<usize>) {
        self.origin = line.filter(|line| *line > 0);
    }

    pub(crate) fn push_str(&mut self, text: &str) {
        for part in text.split_inclusive('\n') {
            if !self.line_open {
                self.lines.push(self.origin);
            }
            self.line_open = !part.ends_with('\n');
        }
        self.text.push_str(text);
    }

    pub(crate) fn push(&mut self, c: char) {
        self.push_str(c.encode_utf8(&mut [0; 4]));
    }

    pub fn line_origin(&self, line: usize) -> Option<usize> {
        self.lines.get(line.checked_sub(1)?).copied().flatten()
    }

    /// The Low parser remains independent. Restore only its diagnostic lines.
    pub fn restore_lines(&self, program: &mut Program) -> Result<(), String> {
        let missing = Cell::new(None);
        crate::source::map_lines(program, |line| {
            self.line_origin(line).unwrap_or_else(|| {
                missing.set(Some(line));
                0
            })
        });
        if let Some(line) = missing.get() {
            Err(format!("generated Low line {line} has no source location"))
        } else {
            Ok(())
        }
    }
}

fn normalized(path: &str) -> String {
    let path = path.replace('\\', "/");
    if let Some(rest) = path.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else {
        path.strip_prefix("//?/").unwrap_or(&path).to_owned()
    }
}

fn same_file(path: &str, expected: &Path) -> bool {
    if path == expected.to_string_lossy() {
        return true;
    }
    // Cargo can retain a Windows short path while emission uses its long form.
    // Resolve existing files before comparing their textual representations.
    if let (Ok(path), Ok(expected)) = (std::fs::canonicalize(path), std::fs::canonicalize(expected))
    {
        return path == expected;
    }
    let expected = expected.to_string_lossy();
    // On Unix, a backslash is part of a filename, not a path separator.
    // Also recognize Windows paths in cross-platform diagnostic fixtures.
    windows_path(&expected) && normalized(path) == normalized(&expected)
}

fn windows_path(path: &str) -> bool {
    cfg!(windows) || path.starts_with(r"\\") || path.as_bytes().get(1) == Some(&b':')
}

fn span_origin(span: &Value, generated: &Generated, file: &Path) -> Option<usize> {
    let name = span.get("file_name")?.as_str()?;
    let relative = matches!(name, "src/main.rs" | "./src/main.rs")
        || windows_path(&file.to_string_lossy())
            && matches!(name, r"src\main.rs" | r".\src\main.rs");
    if !relative && !same_file(name, file) {
        return None;
    }
    generated.line_origin(usize::try_from(span.get("line_start")?.as_u64()?).ok()?)
}

fn original(diagnostic: &Value) -> String {
    let mut text = diagnostic
        .get("rendered")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!(
                "{}: {}",
                diagnostic["level"].as_str().unwrap_or("error"),
                diagnostic["message"].as_str().unwrap_or("Rust diagnostic")
            )
        });
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn mapped_spans<'a>(
    diagnostic: &'a Value,
    generated: &Generated,
    file: &Path,
    sources: &Sources,
) -> Vec<(usize, &'a Value)> {
    let mut mapped: Vec<_> = diagnostic["spans"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|span| {
            let origin = span_origin(span, generated, file)?;
            sources.location(origin)?;
            Some((origin, span))
        })
        .collect();
    mapped.sort_by_key(|(_, span)| span["is_primary"].as_bool() != Some(true));
    mapped
}

fn child_diagnostics<'a>(diagnostic: &'a Value, out: &mut Vec<&'a Value>) {
    if let Some(children) = diagnostic["children"].as_array() {
        for child in children {
            out.push(child);
            child_diagnostics(child, out);
        }
    }
}

/// Cargo's progress remains on stderr. Artifacts stay quiet, and diagnostics
/// from dependencies or native Rust retain rustc's complete rendered output.
pub fn cargo_message(
    line: &str,
    generated: &Generated,
    file: &Path,
    sources: &Sources,
) -> Option<String> {
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => return Some(format!("{line}\n")),
    };
    match value["reason"].as_str() {
        Some("compiler-artifact" | "build-script-executed" | "build-finished") => return None,
        Some("compiler-message") => {}
        _ => return Some(format!("{line}\n")),
    }
    let diagnostic = &value["message"];
    let fallback = original(diagnostic);
    if !value["target"]["src_path"]
        .as_str()
        .is_some_and(|path| same_file(path, file))
    {
        return Some(fallback);
    }
    let mapped = mapped_spans(diagnostic, generated, file, sources);
    if !mapped
        .iter()
        .any(|(_, span)| span["is_primary"].as_bool() == Some(true))
    {
        return Some(fallback);
    }
    let code = diagnostic["code"]["code"]
        .as_str()
        .map(|code| format!("[{code}]"))
        .unwrap_or_default();
    let mut out = format!(
        "{}{code}: {}\n",
        diagnostic["level"].as_str().unwrap_or("error"),
        sources.readable_message(diagnostic["message"].as_str().unwrap_or("Rust diagnostic"))
    );
    let mut seen = std::collections::HashSet::new();
    let mut locations = |mapped: Vec<(usize, &Value)>, out: &mut String| {
        for (origin, span) in mapped {
            let label = span["label"].as_str().unwrap_or("");
            if !seen.insert((origin, label.to_owned())) {
                continue;
            }
            let location = sources.location(origin).unwrap();
            out.push_str(&format!(
                " --> {}:{}\n {} | {}\n",
                location.path.display(),
                location.line,
                location.line,
                location.text
            ));
            if !label.is_empty() {
                out.push_str(&format!("  = {}\n", sources.readable_message(label)));
            }
        }
    };
    locations(mapped, &mut out);
    let mut children = Vec::new();
    child_diagnostics(diagnostic, &mut children);
    for child in children {
        // Only cause notes gain Nagi coordinates. Rust help and replacement
        // suggestions remain in the unchanged rendered details below.
        if child["level"].as_str() != Some("note") {
            continue;
        }
        let mapped = mapped_spans(child, generated, file, sources)
            .into_iter()
            .filter(|(_, span)| span["suggested_replacement"].is_null())
            .collect::<Vec<_>>();
        if mapped.is_empty() {
            continue;
        }
        out.push_str(&format!(
            " {}: {}\n",
            child["level"].as_str().unwrap_or("note"),
            sources.readable_message(child["message"].as_str().unwrap_or("Rust note"))
        ));
        locations(mapped, &mut out);
    }
    // Keep rustc's notes and suggestions in Rust coordinates. Replacements
    // such as '&' or '.clone()' cannot safely be applied to Nagi source.
    out.push_str(" note: Rust backend details (generated code):\n");
    out.push_str(&fallback);
    Some(out)
}
