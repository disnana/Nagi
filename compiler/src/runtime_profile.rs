//! Conservative dependency selection after complete checking, never a check cache.
use crate::{
    ast::*,
    checked::{CheckedProgram, ExpressionKey, FunctionPlan},
    project::RuntimeProfile,
};
use std::{collections::BTreeSet, path::Path};

pub(crate) struct Selection {
    pub core: bool,
    pub reason: &'static str,
}
impl Selection {
    fn full(reason: &'static str) -> Self {
        Self {
            core: false,
            reason,
        }
    }
    pub fn name(&self) -> &'static str {
        if self.core {
            "core"
        } else {
            "full"
        }
    }
}

fn capability(runtime: &Path) -> bool {
    let Some(manifest) = std::fs::read_to_string(runtime.join("Cargo.toml"))
        .ok()
        .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
    else {
        return false;
    };
    let Some(metadata) = manifest
        .get("package")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("nagi"))
        .and_then(|v| v.get("runtime-profiles"))
    else {
        return false;
    };
    if metadata.get("schema").and_then(toml::Value::as_integer) != Some(1)
        || metadata.get("core-feature").and_then(toml::Value::as_str) != Some("core")
        || metadata.get("core-package").and_then(toml::Value::as_str) != Some("nagi-runtime-core")
        || metadata.get("core-path").and_then(toml::Value::as_str) != Some("../runtime-core")
    {
        return false;
    }
    let core = runtime.join("../runtime-core");
    let Some(core_manifest) = std::fs::read_to_string(core.join("Cargo.toml"))
        .ok()
        .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
    else {
        return false;
    };
    core_manifest
        .get("package")
        .and_then(|v| v.get("name"))
        .and_then(toml::Value::as_str)
        == Some("nagi-runtime-core")
        && core_manifest
            .get("package")
            .and_then(|v| v.get("build"))
            .and_then(toml::Value::as_str)
            == Some("build.rs")
        && core_manifest
            .get("lib")
            .and_then(|v| v.get("path"))
            .and_then(toml::Value::as_str)
            == Some("../runtime/src/lib.rs")
        && runtime.join("src/lib.rs").is_file()
        && core.join("build.rs").is_file()
        && core_manifest
            .get("features")
            .and_then(|f| f.get("core"))
            .and_then(toml::Value::as_array)
            .is_some_and(Vec::is_empty)
}

pub(crate) fn select(
    checked: &CheckedProgram,
    requested: RuntimeProfile,
    runtime: &Path,
    incremental: bool,
    native: bool,
    adapter: bool,
    dependencies: bool,
) -> Selection {
    let reason = if requested == RuntimeProfile::Full {
        Some("full_requested")
    } else if incremental {
        Some("incremental_requested")
    } else if native {
        Some("native_input")
    } else if adapter {
        Some("rust_adapter")
    } else if dependencies {
        Some("rust_dependencies")
    } else if !capability(runtime) {
        Some("runtime_unsupported")
    } else if !eligible(checked) {
        Some("unsupported_ir")
    } else {
        None
    };
    reason.map(Selection::full).unwrap_or(Selection {
        core: true,
        reason: "eligible_core",
    })
}

fn primitive(t: &Type) -> bool {
    t.1.is_empty()
        && matches!(
            t.0.as_str(),
            "i8" | "i16"
                | "i32"
                | "i64"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "f32"
                | "f64"
                | "bool"
                | "str"
                | "unit"
        )
}
fn allowed(t: &Type, classes: &BTreeSet<&str>) -> bool {
    primitive(t) || (t.1.is_empty() && classes.contains(t.0.as_str()))
}
fn expression(
    e: &Expr,
    plan: &FunctionPlan,
    classes: &BTreeSet<&str>,
    functions: &BTreeSet<&str>,
) -> bool {
    let Some(decision) = plan.expressions.get(&ExpressionKey::of(e)) else {
        return false;
    };
    if !e.ty.as_ref().is_some_and(|t| allowed(t, classes))
        || decision.operation.is_some()
        || decision.task.is_some()
    {
        return false;
    }
    let child = |e: &Expr| expression(e, plan, classes, functions);
    match &e.kind {
        E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) => true,
        E::Name(_) => e.resolution == Some(NameResolution::Local),
        E::Unary(op, value) => matches!(op.as_str(), "-" | "not") && child(value),
        E::Binary(a, op, b) => {
            matches!(
                op.as_str(),
                "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "and" | "or"
            ) && child(a)
                && child(b)
        }
        E::Call(name, types, arguments) => {
            decision.argument_resolution == e.resolution
                && types.is_empty()
                && arguments.iter().all(child)
                && ((e.resolution == Some(NameResolution::Function)
                    && functions.contains(name.as_str()))
                    || (e.resolution == Some(NameResolution::Builtin)
                        && name == "print"
                        && arguments.len() == 1))
        }
        E::Record(name, fields) => {
            classes.contains(name.as_str()) && fields.iter().all(|(_, e)| child(e))
        }
        E::Field(receiver, _) => {
            decision.field.is_none()
                && e.resolution != Some(NameResolution::ResourceField)
                && receiver
                    .ty
                    .as_ref()
                    .is_some_and(|t| classes.contains(t.0.as_str()))
                && child(receiver)
        }
        _ => false,
    }
}
fn eligible(checked: &CheckedProgram) -> bool {
    let plan = &checked.emission;
    let program = &plan.program;
    if !program.enums.is_empty()
        || !plan.native_paths.is_empty()
        || !plan.native_reexports.is_empty()
        || program.classes.iter().any(|c| {
            primitive(&Type::named(&c.name)) || c.fields.iter().any(|(_, t)| !primitive(t))
        })
    {
        return false;
    }
    let classes = program.classes.iter().map(|c| c.name.as_str()).collect();
    let functions = program.functions.iter().map(|f| f.name.as_str()).collect();
    program.functions.len() == plan.functions.len()
        && program
            .functions
            .iter()
            .zip(&plan.functions)
            .all(|(f, decisions)| {
                !f.asynchronous
                    && !f.external
                    && f.attrs.is_empty()
                    && allowed(&f.ret, &classes)
                    && f.params.iter().all(|(_, t)| allowed(t, &classes))
                    && decisions
                        .body
                        .shadow
                        .iter()
                        .all(|(_, t)| allowed(t, &classes))
                    && f.body.len() == decisions.body.statements.len()
                    && f.body.iter().zip(&decisions.body.statements).all(|(s, d)| {
                        if d.scope.is_some()
                            || d.task_bridge
                            || d.spawn_result
                            || d.iterator.is_some()
                            || !d.children.is_empty()
                            || s.binding_type
                                .as_ref()
                                .is_some_and(|t| !allowed(t, &classes))
                        {
                            return false;
                        }
                        let expr = |e| expression(e, decisions, &classes, &functions);
                        match &s.kind {
                            S::Assign {
                                annotation, value, ..
                            } => {
                                annotation.as_ref().is_none_or(|t| allowed(t, &classes))
                                    && expr(value)
                            }
                            S::Return(e) => e.as_ref().is_none_or(expr),
                            S::Expr(e) => expr(e),
                            _ => false,
                        }
                    })
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checked(text: &str, high: bool) -> CheckedProgram {
        let mut program = crate::parser::parse(text, high).unwrap();
        crate::check::check(&mut program).unwrap();
        crate::check::finalize(
            program,
            Program::default(),
            crate::source::SourceProvenance::user_low_unmapped(),
        )
        .unwrap()
    }
    #[test]
    fn closed_high_low_programs_and_scalar_records_are_eligible() {
        for (text, high) in [("def main():\n    print(\"Hello\")\n", true),
            ("fn main() { print(\"Hello\"); }", false),
            ("class Point:\n    x: f64\n    y: f64\ndef main():\n    p = Point(x=1.5, y=2.5)\n    print(p.x + p.y)\n", true)] {
            assert!(eligible(&checked(text, high)), "{text}");
        }
    }
    #[test]
    fn unused_async_and_unknown_constructs_cannot_escape_the_closure() {
        assert!(!eligible(&checked(
            "async def unused():\n    return\ndef main():\n    print(1)\n",
            true
        )));
        assert!(!eligible(&checked(
            "def main():\n    xs = [1, 2]\n    print(xs[0])\n",
            true
        )));
    }
    #[test]
    fn legacy_or_missing_runtime_capability_falls_back() {
        let p = checked("def main():\n    print(1)\n", true);
        assert_eq!(
            select(
                &p,
                RuntimeProfile::Auto,
                Path::new("/nonexistent/nagi-runtime"),
                false,
                false,
                false,
                false
            )
            .reason,
            "runtime_unsupported"
        );
        assert_eq!(
            select(
                &p,
                RuntimeProfile::Auto,
                Path::new("/nonexistent"),
                true,
                false,
                false,
                false
            )
            .reason,
            "incremental_requested"
        );
    }
    #[test]
    fn capability_and_external_boundaries_are_explicit() {
        let root = std::env::temp_dir().join(format!(
            "nagi-runtime-capability-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let runtime = root.join("runtime");
        std::fs::create_dir_all(runtime.join("src")).unwrap();
        std::fs::write(runtime.join("src/lib.rs"), "").unwrap();
        let core = root.join("runtime-core");
        std::fs::create_dir(&core).unwrap();
        std::fs::write(core.join("build.rs"), "").unwrap();
        std::fs::write(core.join("Cargo.toml"), "[package]\nname='nagi-runtime-core'\nbuild='build.rs'\n[lib]\npath='../runtime/src/lib.rs'\n[features]\ncore=[]\n").unwrap();
        let p = checked("def main():\n    print(1)\n", true);
        std::fs::write(runtime.join("Cargo.toml"), "[package]\nname='legacy'\n").unwrap();
        assert_eq!(
            select(
                &p,
                RuntimeProfile::Auto,
                &runtime,
                false,
                false,
                false,
                false
            )
            .reason,
            "runtime_unsupported"
        );
        std::fs::write(runtime.join("Cargo.toml"), "[package.metadata.nagi.runtime-profiles]\nschema=1\ncore-feature='core'\ncore-package='nagi-runtime-core'\ncore-path='../runtime-core'\n").unwrap();
        let selected = select(
            &p,
            RuntimeProfile::Auto,
            &runtime,
            false,
            false,
            false,
            false,
        );
        assert!(
            selected.core,
            "reason={}, capability={}, program={:?}",
            selected.reason,
            capability(&runtime),
            p.program()
        );
        for (native, adapter, dependencies, reason) in [
            (true, false, false, "native_input"),
            (false, true, false, "rust_adapter"),
            (false, false, true, "rust_dependencies"),
        ] {
            assert_eq!(
                select(
                    &p,
                    RuntimeProfile::Auto,
                    &runtime,
                    false,
                    native,
                    adapter,
                    dependencies
                )
                .reason,
                reason
            );
        }
        assert_eq!(
            select(
                &p,
                RuntimeProfile::Full,
                &runtime,
                false,
                false,
                false,
                false
            )
            .reason,
            "full_requested"
        );
        std::fs::write(core.join("Cargo.toml"), "[features]\ncore=['full']\n").unwrap();
        assert_eq!(
            select(
                &p,
                RuntimeProfile::Auto,
                &runtime,
                false,
                false,
                false,
                false
            )
            .reason,
            "runtime_unsupported"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cli_preserves_release_default_and_rejects_unsupported_requests() {
        let cwd = Path::new(env!("CARGO_MANIFEST_DIR"));
        let args = |xs: &[&str]| xs.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let selected = crate::project::resolve(
            &args(&[
                "build",
                "main.nagi",
                "--no-project",
                "--runtime-profile",
                "auto",
            ]),
            cwd,
        )
        .unwrap();
        assert_eq!(selected.runtime_profile, RuntimeProfile::Auto);
        assert_eq!(selected.profile, crate::project::BuildProfile::Release);
        for xs in [
            vec!["build", "--runtime-profile"],
            vec!["build", "--runtime-profile", "core"],
            vec![
                "build",
                "--runtime-profile",
                "auto",
                "--runtime-profile",
                "full",
            ],
            vec!["check", "--runtime-profile", "auto"],
        ] {
            assert!(crate::project::resolve(&args(&xs), cwd).is_err());
        }
    }
}
