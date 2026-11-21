use super::{error, go_type as backend_type, statement_matches, Emitter};
use crate::{
    ast::{Expr, NameResolution, Program, Type, E},
    stdlib::{self, Operation, Resource},
};
use std::collections::BTreeSet;

pub(super) fn operation_name(operation: Operation) -> Option<&'static str> {
    Some(match operation {
        Operation::ResponseContext => "nagiHTTPContext",
        Operation::Text => "nagiHTTPText",
        Operation::Bytes => "nagiHTTPBytes",
        Operation::DefaultOptions => "nagiHTTPDefaultOptions",
        Operation::Options => "nagiHTTPOptionsWithBody",
        Operation::Authority => "nagiHTTPAuthority",
        Operation::Capacity => "nagiHTTPCapacity",
        Operation::ResponseLimits => "nagiHTTPResponseBudget",
        Operation::WithResponseLimits => "nagiHTTPWithResponseLimits",
        Operation::RateLimits => "nagiHTTPRateBudget",
        Operation::WithRateLimits => "nagiHTTPWithRateLimits",
        Operation::RatePolicy => "nagiHTTPMakeRatePolicy",
        Operation::RateRoute => "nagiHTTPRateRoute",
        Operation::AppDefault => "nagiHTTPAppDefault",
        Operation::PublicPolicy => "nagiHTTPPublicPolicy",
        Operation::Route => "nagiHTTPRoute",
        Operation::CorsPolicy => "nagiHTTPCORSConfig",
        Operation::CorsRoute => "nagiHTTPCORSRoute",
        Operation::Serve => "nagiHTTPServe",
        _ => return None,
    })
}

pub(super) fn go_type(
    ty: &Type,
    classes: &BTreeSet<String>,
    enums: &BTreeSet<String>,
    line: usize,
) -> Result<Option<String>, String> {
    let native = match (stdlib::resource(&ty.0), ty.1.as_slice()) {
        (Some(Resource::Request), []) => "nagiHTTPRequest",
        (Some(Resource::Response), []) => "nagiHTTPResponse",
        (Some(Resource::Method), []) => "nagiHTTPMethod",
        (Some(Resource::Status), []) => "nagiHTTPStatus",
        (Some(Resource::CorsPolicy), []) => "nagiHTTPCORSPolicy",
        (Some(Resource::CorsCredentials), []) => "nagiHTTPCORSCredentials",
        (Some(Resource::Options), []) => "nagiHTTPOptions",
        (Some(Resource::ResponseLimits), []) => "nagiHTTPResponseLimits",
        (Some(Resource::RatePolicy), []) => "nagiHTTPRatePolicy",
        (Some(Resource::RateLimits), []) => "nagiHTTPRateLimits",
        (Some(Resource::App), [state, problem]) if *problem == Type::named("Error") => {
            let state = backend_type(state, classes, enums, line)?;
            return Ok(Some(format!("nagiHTTPApp[{state}]")));
        }
        (Some(Resource::HttpPolicy), [state, authority]) if *authority == Type::named("unit") => {
            let state = backend_type(state, classes, enums, line)?;
            return Ok(Some(format!("nagiHTTPPolicy[{state}]")));
        }
        _ => return Ok(None),
    };
    Ok(Some(native.into()))
}

pub(super) fn local_view(ty: &Type) -> Option<&'static str> {
    if matches!((ty.0.as_str(), ty.1.as_slice()), ("view", [Type(head, args)]) if head == "bytes" && args.is_empty())
    {
        return Some("[]byte");
    }
    match stdlib::resource(&stdlib::native_view_element(ty)?.0) {
        Some(Resource::Request) => Some("nagiHTTPRequest"),
        Some(Resource::ResponseContext) => Some("*nagiHTTPResponseContext"),
        _ => None,
    }
}

// No mutable Go backing storage is shared by route callbacks in this slice.
fn state_is_value(program: &Program, ty: &Type, visiting: &mut BTreeSet<String>) -> bool {
    if !ty.1.is_empty() {
        return false;
    }
    if matches!(
        ty.0.as_str(),
        "str"
            | "bool"
            | "unit"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
    ) {
        return true;
    }
    let Some(record) = program.classes.iter().find(|record| record.name == ty.0) else {
        return false;
    };
    if !visiting.insert(record.name.clone()) {
        return false;
    }
    let supported = record
        .fields
        .iter()
        .all(|(_, field)| state_is_value(program, field, visiting));
    visiting.remove(&record.name);
    supported
}

pub(super) fn validate_call(
    emitter: &Emitter<'_>,
    operation: Operation,
    expression: &Expr,
    arguments: &[Expr],
    direct_return: bool,
) -> Result<(), String> {
    let line = expression.line;
    match operation {
        Operation::AppDefault => {
            let state = arguments[0].ty.as_ref().expect("checked HTTP state");
            if !emitter.classes.contains(&state.0)
                || !state_is_value(emitter.program, state, &mut BTreeSet::new())
            {
                return Err(error(
                    line,
                    "Go HTTP state requires a record of scalar or string values",
                ));
            }
        }
        Operation::Route => {
            validate_route(emitter, line, arguments)?;
        }
        Operation::Serve if emitter.current_function.as_deref() != Some("main") => {
            return Err(error(line, "Go HTTP serve is supported only in main"));
        }
        Operation::Bytes | Operation::Text if !direct_return => {
            return Err(error(
                line,
                "Go HTTP response construction requires a direct return",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_route(emitter: &Emitter<'_>, line: usize, arguments: &[Expr]) -> Result<(), String> {
    let name = match &arguments[4].kind {
        E::Name(name) if arguments[4].resolution == Some(NameResolution::Function) => name,
        _ => return Err(error(line, "Go HTTP route requires a named handler")),
    };
    let handler = emitter
        .program
        .functions
        .iter()
        .find(|function| function.name == *name)
        .ok_or_else(|| {
            error(
                line,
                "internal compiler error: missing checked HTTP handler",
            )
        })?;
    if handler.body.iter().any(|statement| {
        statement_matches(statement, &|expression| {
            matches!(expression.kind, E::Await(_))
        })
    }) {
        return Err(error(line, "Go HTTP handler must complete without await"));
    }
    Ok(())
}

pub(super) fn call_type_arguments(
    emitter: &Emitter<'_>,
    operation: Operation,
    expression: &Expr,
    arguments: &[Expr],
) -> Result<String, String> {
    let state = match operation {
        Operation::PublicPolicy => &expression.ty.as_ref().expect("checked HTTP policy").1[0],
        Operation::AppDefault => arguments[0].ty.as_ref().expect("checked HTTP state"),
        _ => return Ok(String::new()),
    };
    backend_type(state, &emitter.classes, &emitter.enums, expression.line)
        .map(|state| format!("[{state}]"))
}

pub(super) fn constant(owner: &Expr, name: &str, line: usize) -> Result<&'static str, String> {
    let resource = match &owner.kind {
        E::Name(symbol) => stdlib::resource(symbol),
        _ => None,
    };
    match (resource, name) {
        (Some(Resource::Method), "GET") => Ok("nagiHTTPGet"),
        (Some(Resource::Method), "POST") => Ok("nagiHTTPPost"),
        (Some(Resource::CorsCredentials), "ALLOW") => Ok("nagiHTTPCORSCredentials(true)"),
        (Some(Resource::CorsCredentials), "OMIT") => Ok("nagiHTTPCORSCredentials(false)"),
        (Some(Resource::Status), "OK") => Ok("nagiHTTPOK"),
        _ => Err(error(
            line,
            format!("HTTP constant {name} is not supported by the Go backend"),
        )),
    }
}

pub(super) fn field(owner: &Expr, name: &str, line: usize) -> Result<&'static str, String> {
    let ty = owner.ty.as_ref().expect("checked resource field");
    let ty = stdlib::native_view_element(ty).unwrap_or(ty);
    match (stdlib::resource(&ty.0), name) {
        (Some(Resource::Request), "body") => Ok("body"),
        _ => Err(error(
            line,
            format!("resource field {name} is not supported by the Go backend"),
        )),
    }
}

pub(super) fn runtime(program: &Program, imports: &mut Vec<&'static str>) -> String {
    if !program
        .modules
        .definitions
        .iter()
        .any(|definition| definition.id.module.0 == stdlib::MODULE_ID)
    {
        return String::new();
    }
    let http = runtime_fragment(include_str!("http.go"), imports);
    let cors = runtime_fragment(include_str!("cors.go"), imports);
    format!("{http}\n{cors}")
}

fn runtime_fragment(source: &'static str, imports: &mut Vec<&'static str>) -> &'static str {
    let (runtime_imports, definitions) = source
        .strip_prefix("package main\n\nimport (\n")
        .and_then(|source| source.split_once("\n)\n"))
        .expect("Go HTTP runtime source with an import block");
    for import in runtime_imports
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !imports.contains(&import) {
            imports.push(import);
        }
    }
    definitions
}
