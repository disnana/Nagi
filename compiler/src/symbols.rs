//! Definition locations for the files and AST actually loaded by the compiler.
use crate::{
    ast::*,
    lexer::{lex, Token, K},
    source::Sources,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug, Serialize)]
struct Location {
    file: String,
    line: usize,
    column: usize,
    length: usize,
}
#[derive(Serialize)]
struct Definition {
    name: String,
    kind: &'static str,
    location: Location,
}
#[derive(Serialize)]
struct Reference {
    location: Location,
    target: Location,
}

struct File<'a> {
    path: &'a Path,
    lines: Vec<&'a str>,
    start: usize,
    tokens: Vec<Token>,
}
impl File<'_> {
    fn follows_call(&self, index: usize) -> bool {
        let Some(token) = self.tokens.get(index + 1) else {
            return false;
        };
        if matches!(&token.kind, K::Sym(s) if s == "(") {
            return true;
        }
        if !matches!(&token.kind, K::Sym(s) if s == "[") {
            return false;
        }
        let mut depth = 0usize;
        for (i, token) in self.tokens.iter().enumerate().skip(index + 1) {
            match &token.kind {
                K::Sym(s) if s == "[" => depth += 1,
                K::Sym(s) if s == "]" => {
                    depth -= 1;
                    if depth == 0 {
                        return self
                            .tokens
                            .get(i + 1)
                            .is_some_and(|t| matches!(&t.kind, K::Sym(s) if s == "("));
                    }
                }
                _ => {}
            }
        }
        false
    }
    fn string_length(&self, token: &Token) -> usize {
        let mut chars = self.lines[token.line - 1].chars().skip(token.col - 1);
        let quote = chars.next().unwrap();
        let mut length = 1;
        let mut escaped = false;
        for c in chars {
            length += c.len_utf16();
            if c == quote && !escaped {
                break;
            }
            escaped = c == '\\' && !escaped;
        }
        length
    }
    fn location(&self, token: &Token, length: usize) -> Location {
        // VS Code uses UTF-16 columns; the lexer counts Unicode characters.
        let prefix = self.lines.get(token.line - 1).copied().unwrap_or("");
        let column = prefix
            .chars()
            .take(token.col - 1)
            .map(char::len_utf16)
            .sum::<usize>()
            + 1;
        Location {
            file: self.path.display().to_string(),
            line: token.line,
            column,
            length,
        }
    }
}

pub fn index(sources: &Sources, programs: &[&Program]) -> Result<serde_json::Value, String> {
    let files = sources
        .files()
        .map(|(path, text, start)| {
            Ok(File {
                path,
                lines: text.lines().collect(),
                start,
                tokens: lex(text, path.extension().is_none_or(|x| x != "low"))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut definitions = vec![];
    let mut targets = BTreeMap::new();
    for p in programs {
        for (name, kind, line, keyword) in p
            .classes
            .iter()
            .map(|c| (&c.name, "class", c.line, "class"))
            .chain(
                p.functions
                    .iter()
                    .map(|f| (&f.name, "function", f.line, "def")),
            )
        {
            let Some(file) = files
                .iter()
                .find(|f| line >= f.start && line < f.start + f.lines.len().max(1))
            else {
                continue;
            };
            let local = line - file.start + 1;
            let low_keyword = if keyword == "def" { "fn" } else { "record" };
            let Some(pair) = file.tokens.windows(2).find(|pair| {
                pair[0].line == local
                    && matches!(&pair[0].kind, K::Id(n) if n == keyword || n == low_keyword)
                    && matches!(&pair[1].kind, K::Id(n) if n == name)
            }) else {
                continue;
            };
            let location = file.location(&pair[1], name.len());
            // A Low replacement navigates to the High declaration when both are present.
            targets
                .entry(name.clone())
                .or_insert_with(|| location.clone());
            definitions.push(Definition {
                name: name.clone(),
                kind,
                location,
            });
        }
    }
    let mut calls = BTreeSet::new();
    fn expr(e: &Expr, calls: &mut BTreeSet<(usize, String)>) {
        match &e.kind {
            E::Call(name, _, args) => {
                calls.insert((e.line, name.clone()));
                for a in args {
                    expr(a, calls);
                }
            }
            E::Record(name, fields) => {
                calls.insert((e.line, name.clone()));
                for (_, a) in fields {
                    expr(a, calls);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expr(a, calls);
                expr(b, calls);
            }
            E::Unary(_, a) | E::Try(a) | E::Await(a) | E::Field(a, _) => expr(a, calls),
            E::List(args) => {
                for a in args {
                    expr(a, calls);
                }
            }
            _ => {}
        }
    }
    fn block(ss: &[Stmt], calls: &mut BTreeSet<(usize, String)>) {
        for s in ss {
            match &s.kind {
                S::Assign { value, .. }
                | S::Return(Some(value))
                | S::Expr(value)
                | S::Spawn(value) => expr(value, calls),
                S::If(value, a, b) => {
                    expr(value, calls);
                    block(a, calls);
                    block(b, calls);
                }
                S::While(value, body) | S::For(_, value, body) => {
                    expr(value, calls);
                    block(body, calls);
                }
                S::Match(value, arms) => {
                    expr(value, calls);
                    for arm in arms {
                        block(&arm.body, calls);
                    }
                }
                S::Scope(body) => block(body, calls),
                S::Return(None) => {}
            }
        }
    }
    for p in programs {
        for f in &p.functions {
            block(&f.body, &mut calls);
        }
    }
    let mut references = vec![];
    let classes: BTreeSet<_> = definitions
        .iter()
        .filter(|d| d.kind == "class")
        .map(|d| d.name.as_str())
        .collect();
    for file in &files {
        let mut reading_type = false;
        let mut type_depth = 0usize;
        for (i, token) in file.tokens.iter().enumerate() {
            match &token.kind {
                K::Sym(s) if s == ":" || s == "->" => reading_type = true,
                K::Sym(s) if s == "[" && reading_type => type_depth += 1,
                K::Sym(s) if s == "]" && type_depth > 0 => type_depth -= 1,
                K::Sym(s) if s == "," && type_depth > 0 => {}
                K::Sym(_) | K::Newline | K::Indent | K::Dedent if type_depth == 0 => {
                    reading_type = false
                }
                _ => {}
            }
            if let K::Id(name) = &token.kind {
                let call = file.follows_call(i);
                if (call && calls.contains(&(file.start + token.line - 1, name.clone())))
                    || (reading_type && classes.contains(name.as_str()))
                {
                    if let Some(target) = targets.get(name) {
                        references.push(Reference {
                            location: file.location(token, name.len()),
                            target: target.clone(),
                        });
                    }
                }
            }
            if let K::Str(import) = &token.kind {
                if i > 0 && matches!(&file.tokens[i - 1].kind, K::Id(n) if n == "import") {
                    let target = file.path.parent().unwrap().join(import);
                    let target = std::fs::canonicalize(target).map_err(|e| e.to_string())?;
                    references.push(Reference {
                        location: file.location(token, file.string_length(token)),
                        target: Location {
                            file: target.display().to_string(),
                            line: 1,
                            column: 1,
                            length: 0,
                        },
                    });
                }
            }
        }
    }
    for def in &definitions {
        references.push(Reference {
            location: def.location.clone(),
            target: def.location.clone(),
        });
    }
    Ok(
        serde_json::json!({ "format": "nagi-symbols-v1", "definitions": definitions, "references": references }),
    )
}
