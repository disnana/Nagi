//! Import queries use the parser's spelling rules and the source loader's graph.
//! A directory name is a path fact, never a checked expression/type fact.
use super::*;

const CATALOG_BUDGET: usize = 8192;
struct ImportQuery {
    names: bool,
    source: Option<(String, ImportSource)>,
    prefix: String,
    replacement: Value,
}
enum Recognition {
    Query(ImportQuery),
    Suppress,
}
fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}
fn token_bytes(line: &str, token: &Token) -> Option<(usize, usize)> {
    let start = line.char_indices().nth(token.col - 1).map(|(b, _)| b)?;
    let length = match &token.kind {
        K::Id(name) | K::Sym(name) => name.len(),
        // Quoted paths are only consumed as complete lexer-recognized sources.
        K::Str(_) => {
            let quote = line[start..].chars().next()?;
            let mut escaped = false;
            let mut length = quote.len_utf8();
            for c in line[start + quote.len_utf8()..].chars() {
                length += c.len_utf8();
                if c == quote && !escaped {
                    break;
                }
                escaped = c == '\\' && !escaped;
            }
            length
        }
        _ => return None,
    };
    Some((start, start + length))
}
fn recognize(query: &Query, text: &str, high: bool) -> Option<Recognition> {
    if text.len() > crate::parser::SOURCE_LIMIT {
        return None;
    }
    let (_, line) = line_at(text, query.line)?;
    let caret = byte_column(line, query.column)?;
    let all = lex(text, high).ok()?;
    let mut depth = 0usize;
    // High indentation disappears inside ()/[]; Low parameters precede braces.
    // Query-line symbols stay in `tokens` and cannot become an import keyword.
    let mut continuation = Vec::new();
    let mut tokens = vec![];
    for token in &all {
        if token.line > query.line {
            break;
        }
        match &token.kind {
            K::Indent => depth += 1,
            K::Dedent => depth = depth.saturating_sub(1),
            K::Sym(s) if !high && s == "{" => depth += 1,
            K::Sym(s) if !high && s == "}" => depth = depth.saturating_sub(1),
            K::Sym(s) if token.line < query.line && (s == "(" || s == "[") => {
                continuation.push(if s == "(" { ")" } else { "]" });
            }
            K::Sym(s) if token.line < query.line && (s == ")" || s == "]") => {
                if continuation.pop() != Some(s.as_str()) {
                    return None;
                }
            }
            K::Newline | K::Eof => {}
            _ if token.line == query.line => {
                if depth != 0 || !continuation.is_empty() {
                    return None;
                }
                tokens.push(token);
            }
            _ => {}
        }
    }
    let first = tokens.first()?;
    let from = match &first.kind {
        K::Id(name) if name == "from" => true,
        K::Id(name) if name == "import" => false,
        _ => return None,
    };
    let suppressed = Some(Recognition::Suppress);
    let (_, keyword_end) = token_bytes(line, first)?;
    if caret <= keyword_end {
        return suppressed;
    }
    let location = |start: usize, end: usize| {
        json!({"file":query.file,"line":query.line,
        "column":line[..start].encode_utf16().count()+1,"length":line[start..end].encode_utf16().count()})
    };
    let make = |names, source, start, end| {
        Recognition::Query(ImportQuery {
            names,
            source,
            prefix: line[start..caret].to_owned(),
            replacement: location(start, end),
        })
    };
    let Some(source_token) = tokens.get(1) else {
        return if line[keyword_end..caret].chars().all(char::is_whitespace) {
            Some(make(false, None, caret, caret))
        } else {
            suppressed
        };
    };
    let (start, mut end) = token_bytes(line, source_token)?;
    let mut next = 2;
    let source = match &source_token.kind {
        K::Str(path) => (path.clone(), ImportSource::File),
        K::Id(name) => {
            let mut name = name.clone();
            while let (Some(dot), Some(member)) = (tokens.get(next), tokens.get(next + 1)) {
                if !matches!(&dot.kind, K::Sym(s) if s == ".") {
                    break;
                }
                let K::Id(part) = &member.kind else { break };
                name.push('.');
                name.push_str(part);
                end = token_bytes(line, member)?.1;
                next += 2;
            }
            // An unfinished std prefix is a source query too.
            if tokens
                .get(next)
                .is_some_and(|t| matches!(&t.kind, K::Sym(s) if s == "."))
            {
                end = token_bytes(line, tokens[next])?.1;
                name.push('.');
                next += 1;
            }
            if start <= caret && caret <= end {
                if name.contains('.') && name != "std." && !name.starts_with("std.") {
                    return suppressed;
                }
                if let Some(tail) = tokens.get(next) {
                    let valid = if from {
                        matches!(&tail.kind, K::Id(name) if name == "import")
                    } else {
                        matches!(&tail.kind, K::Id(name) if name == "as")
                            || matches!(&tail.kind, K::Sym(s) if s == ";")
                    };
                    if !valid {
                        return suppressed;
                    }
                }
                return Some(make(false, None, start, end));
            }
            match crate::parser::named_import_source(&name, high) {
                Ok(source) => source,
                Err(_) => return suppressed,
            }
        }
        _ => return suppressed,
    };
    // No completion inside a string, trailing comment, alias or malformed tail.
    if caret <= end || !from {
        return suppressed;
    }
    let Some(import) = tokens.get(next) else {
        return suppressed;
    };
    if !matches!(&import.kind, K::Id(name) if name == "import") {
        return suppressed;
    }
    let (_, mut previous_end) = token_bytes(line, import)?;
    if caret <= previous_end {
        return suppressed;
    }
    next += 1;
    loop {
        let Some(name) = tokens.get(next) else {
            return if line[previous_end..caret].chars().all(char::is_whitespace) {
                Some(make(true, Some(source), caret, caret))
            } else {
                suppressed
            };
        };
        let K::Id(_) = &name.kind else {
            return suppressed;
        };
        let (start, end) = token_bytes(line, name)?;
        if caret >= previous_end
            && caret <= end
            && line[previous_end..start].chars().all(char::is_whitespace)
        {
            if caret < start {
                return suppressed;
            }
            if tokens.get(next + 1).is_some_and(|tail| {
                !matches!(&tail.kind,
                K::Id(name) if name == "as")
                    && !matches!(&tail.kind, K::Sym(s) if s == "," || s == ";")
            }) {
                return suppressed;
            }
            return Some(make(true, Some(source), start, end));
        }
        next += 1;
        if tokens
            .get(next)
            .is_some_and(|t| matches!(&t.kind, K::Id(name) if name == "as"))
        {
            next += 1;
            let Some(alias) = tokens.get(next) else {
                return suppressed;
            };
            if !matches!(&alias.kind, K::Id(_)) {
                return suppressed;
            }
            let (_, end) = token_bytes(line, alias)?;
            if caret <= end {
                return suppressed;
            }
            next += 1;
        }
        let Some(comma) = tokens.get(next) else {
            return suppressed;
        };
        if !matches!(&comma.kind, K::Sym(s) if s == ",") {
            return suppressed;
        }
        let (_, end) = token_bytes(line, comma)?;
        if caret < end {
            return suppressed;
        }
        previous_end = end;
        next += 1;
    }
}

fn catalog(query: &Query, prefix: &str, high: bool) -> (Vec<Value>, bool) {
    let mut items = vec![];
    let Some(parent) = query.file.parent() else {
        return (items, true);
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return (items, true);
    };
    for (visited, entry) in entries.enumerate() {
        if visited >= CATALOG_BUDGET {
            return (vec![], true);
        }
        let Ok(entry) = entry else {
            return (vec![], true);
        };
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some(if high { "nagi" } else { "low" }) {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !identifier(name) || name == "std" || !name.starts_with(prefix) || !path.is_file() {
            continue;
        }
        let Ok(target) = std::fs::canonicalize(&path) else {
            continue;
        };
        if target == query.file {
            continue;
        }
        items.push(json!({"name":name,"kind":"module","access":"namespace",
            "target":{"file":target,"line":1,"column":1,"length":0}}));
    }
    for &module in crate::stdlib::MODULES {
        let info = crate::stdlib::module_info(module);
        if info.name.starts_with(prefix) {
            items.push(
                json!({"name":info.name,"kind":"module","access":"namespace",
                "target":{"file":info.id,"line":1,"column":1,"length":0}}),
            );
        }
    }
    items.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    items.dedup_by(|a, b| a["name"] == b["name"]);
    if items.len() > CATALOG_BUDGET {
        return (vec![], true);
    }
    (items, false)
}

fn exports(
    query: &Query,
    source: &(String, ImportSource),
    prefix: &str,
    high: bool,
    files: &HashMap<PathBuf, String>,
) -> Option<(Vec<Value>, Value)> {
    let (sources, program, root) = if source.1 == ImportSource::Standard {
        let root = crate::stdlib::module(&source.0)?;
        let mut program = Program::default();
        crate::modules::register_standard_module(&mut program.modules, &root).ok()?;
        (Sources::default(), program, root)
    } else {
        let target = crate::source::import_file(&query.file, &source.0, high).ok()?;
        let mut sources = crate::source::load_with_overlays(&target, high, files).ok()?;
        let program = std::mem::take(&mut sources.program);
        let root = program.modules.root.clone()?;
        (sources, program, root)
    };
    let mut items = crate::symbols::import_exports(&sources, &program, &root, prefix).ok()?;
    items.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Some((
        items,
        json!({"files":sources.files().map(|(path, _, _)| path).collect::<Vec<_>>()}),
    ))
}

pub(super) fn complete(
    response: &mut Value,
    original: &Result<Loaded, String>,
    request: &Request,
) -> Result<bool, String> {
    let Some(query) = &request.query else {
        return Ok(false);
    };
    let text = request.files.get(&query.file).cloned().or_else(|| {
        // The normal user-source bound also applies to a query outside the loaded graph.
        let mut file = std::fs::File::open(&query.file).ok()?;
        let mut bytes = vec![];
        Read::by_ref(&mut file)
            .take(crate::parser::SOURCE_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > crate::parser::SOURCE_LIMIT {
            return None;
        }
        String::from_utf8(bytes).ok()
    });
    let Some(text) = text else { return Ok(false) };
    let high = query.file.extension().is_none_or(|s| s != "low");
    let Some(recognized) = recognize(query, &text, high) else {
        return Ok(false);
    };
    if let Ok(loaded) = original {
        let index = crate::symbols::index_with_types(
            &loaded.sources,
            &[&loaded.primary, &loaded.native],
            None,
        )?;
        // Import slots carry navigation facts, not unverified signature/type
        // details from a program whose checker may have rejected a definition.
        response["symbols"] = json!({"format":index["format"],"files":index["files"],
            "references":index["references"],"standard_sources":index["standard_sources"]});
    }
    let Recognition::Query(import) = recognized else {
        return Ok(true);
    };
    let mut completion = json!({"kind":"names","access":"read","context":if import.names {"import-names"} else {"import-source"},
        "replacement":import.replacement,"incomplete":false,"items":[]});
    if import.names {
        if let Some((items, target_index)) = import
            .source
            .as_ref()
            .and_then(|source| exports(query, source, &import.prefix, high, &request.files))
        {
            completion["items"] = json!(items);
            if response.get("symbols").is_none() {
                response["symbols"] = json!({"format":"nagi-symbols-v1","files":[],"references":[],"definitions":[],"locals":[],"roles":[]});
            }
            let paths = response["symbols"]["files"]
                .as_array_mut()
                .expect("symbol files");
            let mut combined = paths.clone();
            for path in target_index["files"].as_array().into_iter().flatten() {
                if !combined.contains(path) {
                    combined.push(path.clone());
                }
            }
            if combined.len() > 128 {
                completion["items"] = json!([]);
                completion["incomplete"] = json!(true);
            } else {
                *paths = combined;
            }
            // The loaded target graph supplies only dependency paths and declared
            // export locations. Its unverified signatures/types are not published.
        }
    } else {
        let (items, incomplete) = catalog(query, &import.prefix, high);
        completion["items"] = json!(items);
        completion["incomplete"] = json!(incomplete);
        completion["directories"] = json!([query.file.parent()]);
    }
    response["completion"] = completion;
    response["semantic_status"] = json!("editor-partial");
    if response["completion"]["items"]
        .as_array()
        .is_some_and(|items| items.len() > CATALOG_BUDGET)
        || response.to_string().len() > 8_000_000
    {
        response["completion"]["items"] = json!([]);
        response["completion"]["incomplete"] = json!(true);
    }
    Ok(true)
}
