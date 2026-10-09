//! Bounded, one-shot editor assistance using the compiler's own checker.
//! This internal protocol is additive: `check` and `symbols` keep their contracts.
use crate::{
    ast::*,
    check::EditorQuery,
    lexer::{lex, K},
    source::Sources,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Query {
    pub file: PathBuf,
    /// One based original-source line and UTF-16 column.
    pub line: usize,
    pub column: usize,
}
pub(crate) struct Request {
    pub files: HashMap<PathBuf, String>,
    pub query: Option<Query>,
}
pub(crate) fn read_request(input: impl Read, cwd: &Path) -> Result<Request, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        files: Vec<Value>,
        query: Option<Query>,
    }
    let mut data = vec![];
    input
        .take(16_000_001)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > 16_000_000 {
        return Err("editor inputは16 MBまでです".into());
    }
    let input: Input = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    let files =
        crate::symbols::read_overlays(json!({"files":input.files}).to_string().as_bytes(), cwd)?;
    let query = input
        .query
        .map(|mut q| {
            q.file = std::fs::canonicalize(cwd.join(&q.file)).map_err(|e| e.to_string())?;
            if q.line == 0 || q.column == 0 {
                return Err("editor query positions are one based".into());
            }
            Ok::<_, String>(q)
        })
        .transpose()?;
    Ok(Request { files, query })
}

struct Loaded {
    sources: Sources,
    primary: Program,
    native: Program,
}
fn load(
    path: &Path,
    natives: &[PathBuf],
    files: &HashMap<PathBuf, String>,
) -> Result<Loaded, String> {
    let mut sources = crate::source::load_with_overlays(
        path,
        path.extension().is_none_or(|x| x != "low"),
        files,
    )?;
    let mut primary = std::mem::take(&mut sources.program);
    let mut native = Program::default();
    for path in natives {
        let ns = crate::source::load_with_overlays(path, false, files)?;
        let mut np = sources.append(ns);
        crate::modules::prepare_native_fragment(&mut np, &primary);
        for class in np.classes {
            if !native.classes.iter().any(|c| c.name == class.name) {
                native.classes.push(class);
            }
        }
        for enumeration in np.enums {
            if !native.enums.iter().any(|e| e.name == enumeration.name) {
                native.enums.push(enumeration);
            }
        }
        for function in np.functions {
            if !native.functions.iter().any(|f| f.name == function.name) {
                native.functions.push(function);
            }
        }
        native
            .modules
            .merge_native(np.modules)
            .map_err(|e| sources.diagnostic(&e))?;
    }
    crate::modules::rebind_native(&mut primary, &mut native).map_err(|e| sources.diagnostic(&e))?;
    Ok(Loaded {
        sources,
        primary,
        native,
    })
}
/// Same High resolution, saved Low check and finalization boundaries as `check`;
/// no outputs, rustc, SQL checks or native application code are executed.
fn frontend_check(loaded: &Loaded, high: bool) -> Result<(), String> {
    let Loaded {
        sources,
        primary,
        native,
    } = loaded;
    let mut p = primary.clone();
    let (nc, ne, nf) = (p.classes.len(), p.enums.len(), p.functions.len());
    let mut resolution = p.clone();
    let mut nr = native.clone();
    nr.functions
        .retain(|f| !f.attrs.iter().any(|(a, _)| a == "replace"));
    crate::modules::synchronize(&mut nr);
    for c in nr.classes {
        if !resolution.classes.iter().any(|old| old.name == c.name) {
            resolution.classes.push(c);
        }
    }
    for e in nr.enums {
        if !resolution.enums.iter().any(|old| old.name == e.name) {
            resolution.enums.push(e);
        }
    }
    for f in nr.functions {
        if !resolution.functions.iter().any(|old| old.name == f.name) {
            resolution.functions.push(f);
        }
    }
    resolution
        .modules
        .merge(nr.modules)
        .map_err(|e| sources.diagnostic(&e))?;
    crate::modules::synchronize(&mut resolution);
    crate::check::check(&mut resolution).map_err(|e| sources.diagnostic(&e))?;
    p.classes = resolution.classes[..nc].to_vec();
    p.enums = resolution.enums[..ne].to_vec();
    p.functions = resolution.functions[..nf].to_vec();
    if high {
        let low = crate::emit::low_with_lines(&p);
        p = crate::parser::parse_generated_low(&low.text)?;
        low.restore_lines(&mut p)?;
    }
    crate::check::finalize(p, native.clone(), sources.provenance())
        .map(|_| ())
        .map_err(|e| crate::diagnostics::finalize_message(&e, sources))
}

/// Convert an exact UTF-16 boundary to a byte boundary; never split a surrogate.
fn byte_column(line: &str, column: usize) -> Option<usize> {
    let wanted = column.checked_sub(1)?;
    let mut utf16 = 0;
    for (byte, ch) in line.char_indices() {
        if utf16 == wanted {
            return Some(byte);
        }
        utf16 += ch.len_utf16();
    }
    (utf16 == wanted).then_some(line.len())
}
fn line_at(text: &str, line: usize) -> Option<(usize, &str)> {
    let mut byte = 0;
    for (i, part) in text.split_inclusive('\n').enumerate() {
        if i + 1 == line {
            return Some((
                byte,
                part.strip_suffix('\n')
                    .unwrap_or(part)
                    .strip_suffix('\r')
                    .unwrap_or(part.strip_suffix('\n').unwrap_or(part)),
            ));
        }
        byte += part.len();
    }
    if text.is_empty() && line == 1
        || text.ends_with('\n') && line == text.bytes().filter(|b| *b == b'\n').count() + 1
    {
        Some((text.len(), ""))
    } else {
        None
    }
}

struct Cursor {
    file: PathBuf,
    line: usize,
    start_column: usize,
    end_column: usize,
    members: bool,
    recovered: Option<String>,
}
/// Recover only a cursor-ending, simple expression statement or dotted place.
/// Recovery is explicitly bounded to a single line. All other syntax failures
/// return the original diagnostic without semantic facts.
fn cursor(query: &Query, text: &str, high: bool) -> Option<Cursor> {
    let (line_byte, line) = line_at(text, query.line)?;
    let caret = byte_column(line, query.column)?;
    let before = &line[..caret];
    let tail_empty = line[caret..].trim().is_empty();
    let tokens = lex(text, high).ok();
    let mut recovered = None;
    let mut members = false;
    let mut start = caret;
    let mut end = caret;
    if tail_empty {
        // A simple dotted name may occur as an RHS, return value or argument.
        // It cannot contain calls/indexes, and only the final dot/prefix changes.
        let mut chain_start = caret;
        for (byte, ch) in before.char_indices().rev() {
            if ch == '.' || ch.is_ascii_alphanumeric() || ch == '_' {
                chain_start = byte;
            } else {
                break;
            }
        }
        let chain = &before[chain_start..];
        if let Some(dot) = chain.rfind('.') {
            let receiver = &chain[..dot];
            let suffix = &chain[dot + 1..];
            let names_valid = receiver.split('.').all(|name| {
                let mut chars = name.chars();
                chars
                    .next()
                    .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
                    && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            });
            if names_valid
                && suffix
                    .chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            {
                start = chain_start;
                end = chain_start + dot;
                members = true;
                // Erasing only the final member suffix retains all other token
                // positions. No recovered token is exposed as a navigation fact.
                let erased = " ".repeat(before[end..].chars().count());
                let mut changed = text.to_owned();
                changed.replace_range(line_byte + end..line_byte + caret, &erased);
                recovered = Some(changed);
            }
        } else if tokens.is_none() || before.trim().is_empty() {
            let trimmed = before.trim_start();
            let mut chars = trimmed.chars();
            let identifier = chars
                .next()
                .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
                && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric());
            if trimmed.is_empty() || identifier {
                start = before.len() - trimmed.len();
                end = start + 4;
                let mut changed = text.to_owned();
                changed.replace_range(line_byte + start..line_byte + caret, "True");
                recovered = Some(changed);
            }
        }
    }
    if !members && recovered.is_none() {
        let token = tokens?.into_iter().find(|t| {
            t.line == query.line
                && matches!(&t.kind, K::Id(name) if {
                    let col = line.chars().take(t.col - 1).map(char::len_utf16).sum::<usize>() + 1;
                    query.column >= col && query.column <= col + name.encode_utf16().count()
                })
        })?;
        let K::Id(name) = token.kind else {
            return None;
        };
        start = line.char_indices().nth(token.col - 1).map(|(b, _)| b)?;
        end = start + name.len();
    }
    let changed_line = recovered
        .as_ref()
        .and_then(|t| line_at(t, query.line).map(|(_, l)| l))
        .unwrap_or(line);
    Some(Cursor {
        file: query.file.clone(),
        line: query.line,
        start_column: changed_line[..start].chars().count() + 1,
        end_column: changed_line[..end].chars().count() + 1,
        members,
        recovered,
    })
}
fn checker_query(sources: &Sources, cursor: &Cursor) -> Option<EditorQuery> {
    let (path, text, offset) = sources.files().find(|(path, _, _)| *path == cursor.file)?;
    let tokens = lex(text, path.extension().is_none_or(|x| x != "low")).ok()?;
    let start = tokens
        .iter()
        .position(|t| t.line == cursor.line && t.col == cursor.start_column)?;
    let end = tokens
        .iter()
        .enumerate()
        .skip(start)
        .take_while(|(_, t)| t.line == cursor.line && t.col < cursor.end_column)
        .last()?
        .0
        + 1;
    Some(EditorQuery {
        line: offset + cursor.line - 1,
        span: Span { start, end },
        members: cursor.members,
    })
}
struct Positions<'a> {
    files: Vec<PositionFile<'a>>,
}
struct PositionFile<'a> {
    path: &'a Path,
    lines: Vec<&'a str>,
    start: usize,
    tokens: Vec<crate::lexer::Token>,
}
impl<'a> Positions<'a> {
    fn new(sources: &'a Sources) -> Result<Self, String> {
        Ok(Self {
            files: sources
                .files()
                .map(|(path, text, start)| {
                    Ok(PositionFile {
                        path,
                        lines: text.lines().collect(),
                        start,
                        tokens: lex(text, path.extension().is_none_or(|x| x != "low"))?,
                    })
                })
                .collect::<Result<_, String>>()?,
        })
    }
    fn binding(&self, binding: crate::ast::BindingId) -> Option<(String, Value)> {
        let file = self.files.iter().find(|file| {
            binding.line >= file.start && binding.line < file.start + file.lines.len().max(1)
        })?;
        let token = file.tokens.get(binding.token)?;
        let K::Id(name) = &token.kind else {
            return None;
        };
        let source_line = file.lines.get(token.line - 1)?;
        let column = source_line
            .chars()
            .take(token.col - 1)
            .map(char::len_utf16)
            .sum::<usize>()
            + 1;
        Some((
            name.clone(),
            json!({"file":file.path,"line":token.line,"column":column,"length":name.encode_utf16().count()}),
        ))
    }
}
fn diagnostic(message: &str, stage: &str, files: &HashMap<PathBuf, String>) -> Value {
    let mut result = json!({"severity":"error","stage":stage,"message":message});
    if let Some(origin) = message
        .lines()
        .find_map(|line| line.trim().strip_prefix("--> "))
    {
        if let Some((path, line)) = origin.rsplit_once(':').and_then(|(path, line)| {
            line.parse::<usize>()
                .ok()
                .map(|line| (PathBuf::from(path), line))
        }) {
            result["file"] = json!(path);
            result["line"] = json!(line);
            // Parser/lexer columns are scalar columns. Checker line-only errors
            // remain line-only; never guess a range from diagnostic words.
            let col = message
                .strip_prefix("error: line ")
                .and_then(|s| s.split(':').nth(1))
                .and_then(|s| s.parse::<usize>().ok());
            if let Some(column) = col {
                let text = files
                    .get(&path)
                    .cloned()
                    .or_else(|| std::fs::read_to_string(&path).ok());
                if let Some(source_line) = text
                    .as_ref()
                    .and_then(|text| text.lines().nth(line.saturating_sub(1)))
                {
                    if column > 0 && column <= source_line.chars().count() + 1 {
                        let utf16 = source_line
                            .chars()
                            .take(column - 1)
                            .map(char::len_utf16)
                            .sum::<usize>()
                            + 1;
                        // A precise reported point does not imply a token span.
                        result["range"] = json!({"line":line,"column":utf16,"length":0});
                    }
                }
            }
        } else {
            result["file"] = json!(origin);
        }
    }
    result
}

pub(crate) fn analyze(
    path: &Path,
    natives: &[PathBuf],
    mut request: Request,
) -> Result<Value, String> {
    let high = path.extension().is_none_or(|x| x != "low");
    let original = load(path, natives, &request.files);
    let original_check = original.as_ref().map(|loaded| frontend_check(loaded, high));
    let mut response = json!({"format":"nagi-assist-v1","semantic_status":"none","full_compile_checked":false,
        "frontend_checked":original.is_ok(),"frontend_accepted":false,"recovered":false,
        "completion":{"kind":"names","access":"read","items":[]},"diagnostics":[]});
    match &original_check {
        Ok(Ok(())) => response["frontend_accepted"] = json!(true),
        Ok(Err(error)) => {
            response["diagnostics"] = json!([diagnostic(error, "check", &request.files)])
        }
        Err(error) => response["diagnostics"] = json!([diagnostic(error, "load", &request.files)]),
    }
    let cursor = request.query.as_ref().and_then(|q| {
        let text = request
            .files
            .get(&q.file)
            .cloned()
            .or_else(|| std::fs::read_to_string(&q.file).ok())?;
        cursor(q, &text, q.file.extension().is_none_or(|x| x != "low"))
    });
    let loaded = if let Some(c) = &cursor {
        if let Some(text) = &c.recovered {
            request.files.insert(c.file.clone(), text.clone());
            response["recovered"] = json!(true);
            load(path, natives, &request.files)
        } else {
            original
        }
    } else {
        original
    };
    let Ok(loaded) = loaded else {
        return Ok(response);
    };
    let query = cursor
        .as_ref()
        .and_then(|c| checker_query(&loaded.sources, c));
    let analysis = crate::check::editor_analysis(&loaded.primary, &loaded.native, query);
    let index = crate::symbols::index_with_types(
        &loaded.sources,
        &[&loaded.primary, &loaded.native],
        analysis.as_ref().map(|(program, _)| program),
    )?;
    response["symbols"] = index.clone();
    let Some(c) = cursor else {
        return Ok(response);
    };
    response["completion"]["kind"] = json!(if c.members { "members" } else { "names" });
    let Some((typed, facts)) = analysis else {
        return Ok(response);
    };
    if !facts.observed || !facts.valid_context {
        return Ok(response);
    }
    response["semantic_status"] = json!("editor-partial");
    let scope = loaded
        .sources
        .module_files()
        .find(|(_, path)| *path == c.file)
        .map(|(module, _)| module.0.as_str())
        .unwrap_or_else(|| c.file.to_str().unwrap_or(""));
    let positions = Positions::new(&loaded.sources)?;
    let mut items = vec![];
    let mut shadowed = HashSet::new();
    for (name, binding) in &facts.shadowed {
        // Unavailable locals still suppress same-spelled globals.
        if let Some((original, _)) = positions.binding(*binding) {
            shadowed.insert(original);
        }
        shadowed.insert(crate::modules::display_symbol(name));
    }
    for item in facts.items {
        let (name, target) = if let Some(binding) = item.binding {
            let Some((name, target)) = positions.binding(binding) else {
                continue;
            };
            (name, Some(target))
        } else {
            (item.name, None)
        };
        items.push(json!({"name":name,"type":crate::symbols::display_type(&item.ty, scope, &typed.modules),"kind":if c.members {"field"} else {"local"},"target":target,"borrowed":item.borrowed,"access":"read"}));
    }
    if c.members {
        // Namespace members come from the resolver's module binding, and their
        // signatures must be verified by this checker invocation. Local names
        // (including moved locals) shadow namespace aliases.
        let file_text = loaded
            .sources
            .files()
            .find(|(path, _, _)| *path == c.file)
            .map(|(_, text, _)| text)
            .unwrap_or("");
        let receiver = line_at(file_text, c.line).and_then(|(_, line)| {
            let start = line.char_indices().nth(c.start_column - 1)?.0;
            let end = line
                .char_indices()
                .nth(c.end_column - 1)
                .map(|(b, _)| b)
                .unwrap_or(line.len());
            line.get(start..end)
        });
        if let Some(receiver) =
            receiver.filter(|name| !name.contains('.') && !shadowed.contains(*name))
        {
            if let Some(binding) = index["bindings"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|b| {
                    b["file"].as_str() == c.file.to_str()
                        && b["name"].as_str() == Some(receiver)
                        && b["kind"] == "module"
                })
            {
                for member in binding["members"].as_array().into_iter().flatten() {
                    if verified_definition(&member["id"], &loaded, &facts.verified_globals) {
                        items.push(json!({"name":member["name"],"kind":member["kind"],"signature":member["signature"],"target":member["location"],"access":"namespace"}));
                    }
                }
            }
        }
    }
    if !c.members {
        for binding in index["bindings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["file"].as_str() == c.file.to_str())
        {
            let Some(name) = binding["name"].as_str() else {
                continue;
            };
            if shadowed.contains(name) {
                continue;
            }
            if let Some(id) = binding.get("definition_id") {
                if verified_definition(id, &loaded, &facts.verified_globals) {
                    items.push(json!({"name":name,"kind":binding["kind"],"signature":binding["definition"]["signature"],"target":binding["target"],"access":"read"}));
                }
            } else if binding["kind"] == "module" {
                items.push(json!({"name":name,"kind":"module","signature":binding["signature"],"target":binding["target"],"access":"namespace"}));
            }
        }
    }
    items.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    response["completion"]["items"] = json!(items);
    Ok(response)
}

fn verified_definition(id: &Value, loaded: &Loaded, globals: &HashSet<String>) -> bool {
    let Ok(id) = serde_json::from_value::<DefId>(id.clone()) else {
        return false;
    };
    globals.contains(&crate::modules::symbol(&id))
        || loaded
            .primary
            .modules
            .definitions
            .iter()
            .chain(&loaded.native.modules.definitions)
            .any(|d| d.id == id && globals.contains(&d.symbol))
}
