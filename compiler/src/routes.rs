use crate::ast::{Function, Program, Type};
use std::collections::{HashMap, HashSet};

pub(crate) fn attribute(function: &Function) -> Option<(&str, &str)> {
    function
        .attrs
        .iter()
        .find(|(name, _)| ["get", "post", "put", "delete"].contains(&name.as_str()))
        .map(|(name, path)| (name.as_str(), path.as_str()))
}

pub(crate) fn has_capture(path: &str) -> bool {
    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
            } else {
                return true;
            }
        }
    }
    false
}

pub(crate) fn validate(program: &Program) -> Result<(), String> {
    let mut registered = HashSet::new();
    let mut patterns = HashMap::new();
    for function in &program.functions {
        let error = |message: String| format!("line {}: {message}", function.line);
        if function.name == "main" && !function.params.is_empty() {
            return Err(error("mainは引数を取りません".into()));
        }
        let Some((method, path)) = attribute(function) else {
            continue;
        };
        if !function.asynchronous
            || function.ret.0 != "Result"
            || function.ret.1.len() != 2
            || function.ret.1[1] != Type::named("Error")
        {
            return Err(error(format!(
                "HTTP handler {}はasyncで定義し、Result[..., Error]を返してください",
                function.name
            )));
        }
        if !path.starts_with('/') {
            return Err(error("HTTPのpathは / で始めてください".into()));
        }
        if method == "get" && ["/health", "/stream", "/ws"].contains(&path) {
            return Err(error(format!("GET {path}は組み込みHTTP endpointです")));
        }
        if let Some(previous) = patterns.insert(path_pattern(path), path) {
            if previous != path {
                return Err(error(format!(
                    "HTTPのpathが競合しています: {previous} と {path}（capture名を揃えてください）"
                )));
            }
        }
        if !registered.insert((method, path)) {
            return Err(error(format!(
                "HTTPの定義が重複しています: {} {path}",
                method.to_uppercase()
            )));
        }
        let mut bodies = 0;
        for (name, ty) in &function.params {
            if ty == &Type::generic("view", vec![Type::named("bytes")])
                || program.classes.iter().any(|class| class.name == ty.0)
            {
                bodies += 1;
            } else if !["Db", "str", "i64", "i32", "u64", "bool", "f64"].contains(&ty.0.as_str()) {
                return Err(error(format!("HTTPの引数 {name}: {ty} は未対応です")));
            }
        }
        if bodies > 1 {
            return Err(error(
                "HTTPのbodyを受け取る引数は1つにしてください（classまたはview[bytes]）".into(),
            ));
        }
    }
    Ok(())
}

fn path_pattern(path: &str) -> String {
    let mut pattern = String::new();
    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '{' {
            pattern.push(ch);
        } else if chars.peek() == Some(&'{') {
            pattern.push_str("{{");
            chars.next();
        } else {
            let wildcard = chars.peek() == Some(&'*');
            while let Some(ch) = chars.next() {
                if ch == '}' {
                    if chars.peek() == Some(&'}') {
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
            pattern.push_str(if wildcard { "{*}" } else { "{}" });
        }
    }
    pattern
}
