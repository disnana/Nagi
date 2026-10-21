//! Conservative partitioning of one complete sealed emission plan.
//! These edges never authorize skipping Nagi checks or invoking a cached exe.
use crate::{
    ast::*,
    checked::{CheckedProgram, ExpressionKey, FunctionPlan},
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Unit {
    pub id: String,
    pub functions: BTreeSet<usize>,
    pub dependencies: BTreeSet<usize>,
}

pub(crate) struct Partition {
    pub units: Vec<Unit>,
}
pub(crate) struct Selection {
    pub functions: BTreeSet<usize>,
    pub imports: BTreeMap<String, Vec<String>>,
    pub facade: bool,
}

fn primitive(t: &Type) -> bool {
    t.1.is_empty() && matches!(t.0.as_str(), "i32" | "i64" | "bool" | "unit")
}

fn expression(
    e: &Expr,
    plan: &FunctionPlan,
    names: &BTreeMap<String, usize>,
    edges: &mut BTreeSet<usize>,
    main: bool,
) -> Result<(), &'static str> {
    if !e.ty.as_ref().is_some_and(primitive) {
        return Err("nonprimitive_expression");
    }
    match &e.kind {
        E::Int(_) | E::Bool(_) => {}
        E::Name(_) if e.resolution == Some(NameResolution::Local) => {}
        E::Binary(a, op, b)
            if [
                "+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">=", "and", "or",
            ]
            .contains(&op.as_str()) =>
        {
            expression(a, plan, names, edges, false)?;
            expression(b, plan, names, edges, false)?;
        }
        E::Unary(op, a) if ["-", "not"].contains(&op.as_str()) => {
            expression(a, plan, names, edges, false)?
        }
        E::Call(name, ts, args) if ts.is_empty() => {
            let resolution = plan
                .expressions
                .get(&ExpressionKey::of(e))
                .ok_or("missing_expression_plan")?
                .argument_resolution;
            if resolution != e.resolution {
                return Err("inconsistent_resolution");
            }
            if resolution == Some(NameResolution::Function) {
                if name == "main" || name == "__nagi_main" {
                    return Err("entry_dependency");
                }
                edges.insert(*names.get(name).ok_or("unknown_function")?);
                for a in args {
                    expression(a, plan, names, edges, false)?;
                }
            } else if resolution == Some(NameResolution::Builtin)
                && name == "i64"
                && args.len() == 1
                && args[0]
                    .ty
                    .as_ref()
                    .is_some_and(|t| *t == Type::named("i32"))
            {
                expression(&args[0], plan, names, edges, false)?;
            } else if main
                && resolution == Some(NameResolution::Builtin)
                && name == "print"
                && args.len() == 1
            {
                // Strings are allowed only as a direct print literal in main.
                if !matches!(args[0].kind, E::Str(_)) {
                    expression(&args[0], plan, names, edges, false)?;
                }
            } else {
                return Err("indirect_or_standard_call");
            }
        }
        _ => return Err("unsupported_expression"),
    }
    Ok(())
}

fn block(
    ss: &[Stmt],
    plan: &FunctionPlan,
    names: &BTreeMap<String, usize>,
    edges: &mut BTreeSet<usize>,
    main: bool,
) -> Result<(), &'static str> {
    for s in ss {
        if s.binding_type.as_ref().is_some_and(|t| !primitive(t)) {
            return Err("nonprimitive_binding");
        }
        match &s.kind {
            S::Assign {
                annotation, value, ..
            } => {
                if annotation.as_ref().is_some_and(|t| !primitive(t)) {
                    return Err("nonprimitive_annotation");
                }
                expression(value, plan, names, edges, false)?;
            }
            S::Expr(e) => expression(e, plan, names, edges, main)?,
            S::Return(e) => {
                if let Some(e) = e {
                    expression(e, plan, names, edges, false)?;
                }
            }
            S::If(e, a, b) => {
                expression(e, plan, names, edges, false)?;
                block(a, plan, names, edges, main)?;
                block(b, plan, names, edges, main)?;
            }
            S::While(e, a) => {
                expression(e, plan, names, edges, false)?;
                block(a, plan, names, edges, main)?;
            }
            _ => return Err("unsupported_statement"),
        }
    }
    Ok(())
}

// Iterative Kosaraju avoids making source-controlled graph depth a Rust stack
// depth. Caller -> callee edges; return components in dependency-first order.
fn components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = edges.len();
    let mut seen = vec![false; n];
    let mut order = Vec::new();
    for root in 0..n {
        if seen[root] {
            continue;
        }
        seen[root] = true;
        let mut stack = vec![(root, 0)];
        while let Some((node, next)) = stack.last_mut() {
            if let Some(&child) = edges[*node].get(*next) {
                *next += 1;
                if !seen[child] {
                    seen[child] = true;
                    stack.push((child, 0));
                }
            } else {
                order.push(*node);
                stack.pop();
            }
        }
    }
    let mut reverse = vec![vec![]; n];
    for (from, ds) in edges.iter().enumerate() {
        for &to in ds {
            reverse[to].push(from);
        }
    }
    seen.fill(false);
    let mut result = Vec::new();
    for root in order.into_iter().rev() {
        if seen[root] {
            continue;
        }
        seen[root] = true;
        let mut stack = vec![root];
        let mut group = Vec::new();
        while let Some(node) = stack.pop() {
            group.push(node);
            for &child in &reverse[node] {
                if !seen[child] {
                    seen[child] = true;
                    stack.push(child);
                }
            }
        }
        group.sort_unstable();
        result.push(group);
    }
    result.reverse();
    result
}

impl Partition {
    pub fn new(checked: &CheckedProgram) -> Result<Self, &'static str> {
        let p = &checked.emission.program;
        // Direct parser clients can retain primitive-spelled user types.
        // Their spelling alone cannot prove an intrinsic Copy interface.
        if p.classes.iter().any(|c| primitive(&Type::named(&c.name)))
            || p.enums.iter().any(|e| primitive(&Type::named(&e.name)))
        {
            return Err("primitive_type_shadow");
        }
        let names: BTreeMap<_, _> = p
            .functions
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.clone(), i))
            .collect();
        if names.is_empty() {
            return Err("no_functions");
        }
        let mut all_edges = BTreeMap::new();
        for (i, f) in p.functions.iter().enumerate() {
            if f.name == "main"
                || f.name == "__nagi_main"
                || f.asynchronous
                || f.external
                || !f.attrs.is_empty()
                || !primitive(&f.ret)
                || f.params.iter().any(|(_, t)| !primitive(t))
            {
                continue;
            }
            let mut edges = BTreeSet::new();
            if block(
                &f.body,
                &checked.emission.functions[i],
                &names,
                &mut edges,
                false,
            )
            .is_ok()
            {
                all_edges.insert(i, edges);
            }
        }
        // Unsupported functions and their transitive callers stay together
        // in the facade. A unit may never depend back on that facade. Unknown
        // incoming edges are safe: the facade depends on every unit.
        let mut reverse = vec![Vec::new(); p.functions.len()];
        for (&caller, deps) in &all_edges {
            for &callee in deps {
                reverse[callee].push(caller);
            }
        }
        let mut excluded: Vec<_> = (0..p.functions.len())
            .filter(|i| !all_edges.contains_key(i))
            .collect();
        while let Some(callee) = excluded.pop() {
            for &caller in &reverse[callee] {
                if all_edges.remove(&caller).is_some() {
                    excluded.push(caller);
                }
            }
        }
        if all_edges.is_empty() {
            return Err("no_closed_functions");
        }
        if all_edges
            .values()
            .flatten()
            .any(|d| !all_edges.contains_key(d))
        {
            return Err("unclosed_partition");
        }
        // Symbol order, rather than source declaration order, controls SCC
        // traversal and bucket membership.
        let indices: Vec<_> = names
            .values()
            .filter(|i| all_edges.contains_key(i))
            .copied()
            .collect();
        let positions: BTreeMap<_, _> = indices.iter().enumerate().map(|(i, &f)| (f, i)).collect();
        let edges: Vec<Vec<_>> = indices
            .iter()
            .map(|i| {
                let mut deps: Vec<_> = all_edges[i].iter().map(|d| positions[d]).collect();
                deps.sort_unstable();
                deps
            })
            .collect();
        let sccs = components(&edges);
        // Merge contiguous topological components only: merging cannot create
        // a cycle, and small functions cannot create unbounded Cargo packages.
        let width = sccs.len().div_ceil(8);
        let mut units = Vec::new();
        let mut membership = BTreeMap::new();
        let mut identities = BTreeSet::new();
        for chunk in sccs.chunks(width) {
            let functions: BTreeSet<_> = chunk.iter().flatten().map(|&i| indices[i]).collect();
            let mut symbols: Vec<_> = functions
                .iter()
                .map(|&i| p.functions[i].name.as_str())
                .collect();
            symbols.sort_unstable();
            let id = crate::build_analysis::digest(
                &serde_json::to_vec(&symbols).expect("symbols serialize"),
            )[..16]
                .to_owned();
            if !identities.insert(id.clone()) {
                return Err("unit_identity_collision");
            }
            for &f in &functions {
                membership.insert(f, units.len());
            }
            units.push(Unit {
                id,
                functions,
                dependencies: BTreeSet::new(),
            });
        }
        for (i, unit) in units.iter_mut().enumerate() {
            for f in &unit.functions {
                for d in &all_edges[f] {
                    let dep = membership[d];
                    if dep != i {
                        unit.dependencies.insert(dep);
                    }
                }
            }
            if unit.dependencies.iter().any(|&d| d >= i) {
                return Err("non_topological_partition");
            }
        }
        for unit in &units {
            let alias = format!("nagi_unit_{}", unit.id);
            if p.functions.iter().any(|f| f.name == alias)
                || p.classes.iter().any(|c| c.name == alias)
                || p.enums.iter().any(|e| e.name == alias)
                || p.modules
                    .root_bindings()
                    .any(|b| checked.emission.names.source_name(&b.name) == alias)
            {
                return Err("unit_alias_collision");
            }
        }
        Ok(Self { units })
    }

    pub fn selection(&self, checked: &CheckedProgram, unit: Option<usize>) -> Selection {
        let p = &checked.emission.program;
        let dependencies = unit
            .map(|i| self.units[i].dependencies.clone())
            .unwrap_or_else(|| (0..self.units.len()).collect());
        Selection {
            functions: unit
                .map(|i| self.units[i].functions.clone())
                .unwrap_or_else(|| {
                    p.functions
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !self.units.iter().any(|u| u.functions.contains(i)))
                        .map(|(i, _)| i)
                        .collect()
                }),
            imports: dependencies
                .into_iter()
                .map(|i| {
                    let mut symbols: Vec<_> = self.units[i]
                        .functions
                        .iter()
                        .map(|&f| p.functions[f].name.clone())
                        .collect();
                    symbols.sort_unstable();
                    (format!("nagi_unit_{}", self.units[i].id), symbols)
                })
                .collect(),
            facade: unit.is_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(text: &str, high: bool) -> CheckedProgram {
        let mut p = crate::parser::parse(text, high).unwrap();
        crate::check::check(&mut p).unwrap();
        crate::check::finalize(
            p,
            Program::default(),
            crate::source::SourceProvenance::user_low_unmapped(),
        )
        .unwrap()
    }

    #[test]
    fn primitive_spelled_user_types_and_unit_aliases_fail_closed() {
        for name in ["i32", "i64", "bool", "unit"] {
            let field = if name == "bool" { "i64" } else { "bool" };
            let high = format!("class {name}:\n    value: {field}\ndef leaf():\n    return\n");
            let p = checked(&high, true);
            assert_eq!(Partition::new(&p).err(), Some("primitive_type_shadow"));
            let low = crate::emit::low(&p.emission.program);
            let p = checked(&low, false);
            assert_eq!(Partition::new(&p).err(), Some("primitive_type_shadow"));
        }
        let source = "def leaf() -> i64:\n    return 7\n";
        let p = checked(source, true);
        let id = Partition::new(&p).unwrap().units[0].id.clone();
        for definition in [
            format!("class nagi_unit_{id}:\n    value: bool\n"),
            format!("enum nagi_unit_{id}:\n    Value\n"),
        ] {
            let p = checked(&format!("{definition}{source}"), true);
            assert_eq!(Partition::new(&p).err(), Some("unit_alias_collision"));
        }
    }
}
