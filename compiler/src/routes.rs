use crate::ast::{Function, Program};

pub(crate) fn attribute(function: &Function) -> Option<(&str, &str)> {
    function
        .attrs
        .iter()
        .find(|(name, _)| ["get", "post", "put", "delete"].contains(&name.as_str()))
        .map(|(name, path)| (name.as_str(), path.as_str()))
}

pub(crate) fn validate(program: &Program) -> Result<(), String> {
    for function in &program.functions {
        if attribute(function).is_some() {
            return Err(format!("line {}: SF01 migration: @get/@post/@put/@deleteは廃止しました。std.http.serverの明示Policy付きrouteへ移行してください", function.line));
        }
        if function.name == "main" && !function.params.is_empty() {
            return Err(format!("line {}: mainは引数を取りません", function.line));
        }
    }
    Ok(())
}
