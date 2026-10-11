//! File-scoped bindings and definition identities shared by all compiler stages.
use crate::{
    ast::*,
    lexer::{Token, K},
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

const PREFIX: &str = "__nagi_def_";
pub const NAMESPACE_PREFIX: &str = "module::";

fn builtin_type(name: &str) -> bool {
    matches!(
        name,
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
            | "bytes"
            | "unit"
            | "Error"
            | "Db"
            | "Html"
            | "UUID"
            | "timestamp"
            | "List"
            | "view"
            | "owned"
            | "shared"
            | "Option"
            | "Map"
            | "Result"
            | "Future"
            | "fn"
            | "Range"
    )
}

fn hex(text: &str) -> String {
    text.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(text: &str) -> Option<String> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let bytes = text
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|digits| u8::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

/// This encoding is injective, including for non-ASCII file and definition names.
pub fn symbol(id: &DefId) -> String {
    format!(
        "{PREFIX}{}_{}_{}",
        hex(&id.module.0),
        match id.kind {
            DefKind::Class => "c",
            DefKind::Enum => "e",
            DefKind::Function => "f",
            DefKind::Resource => "r",
        },
        hex(&id.name)
    )
}

/// A readable identity for diagnostics even when no Sources object is available.
pub fn display_symbol(name: &str) -> String {
    let Some(tail) = name.strip_prefix(PREFIX) else {
        return name.to_owned();
    };
    let mut parts = tail.split('_');
    let (Some(module), Some(kind), Some(def), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return name.to_owned();
    };
    if !matches!(kind, "c" | "e" | "f" | "r") {
        return name.to_owned();
    }
    match (unhex(module), unhex(def)) {
        (Some(module), Some(def)) => format!("{module}::{def}"),
        _ => name.to_owned(),
    }
}

fn visit_types(program: &mut Program, visitor: &mut impl FnMut(&mut Type)) {
    fn expression(expr: &mut Expr, visitor: &mut impl FnMut(&mut Type)) {
        if let Some(ty) = &mut expr.ty {
            visitor(ty);
        }
        match &mut expr.kind {
            E::Call(_, types, args) => {
                for ty in types {
                    visitor(ty);
                }
                for arg in args {
                    expression(arg, visitor);
                }
            }
            E::Record(_, fields) => {
                for (_, value) in fields {
                    expression(value, visitor);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expression(a, visitor);
                expression(b, visitor);
            }
            E::Unary(_, value) | E::Field(value, _) | E::Await(value) | E::Try(value) => {
                expression(value, visitor);
            }
            E::List(values) => {
                for value in values {
                    expression(value, visitor);
                }
            }
            _ => {}
        }
    }
    fn statements(body: &mut [Stmt], visitor: &mut impl FnMut(&mut Type)) {
        for stmt in body {
            if let Some(ty) = &mut stmt.binding_type {
                visitor(ty);
            }
            match &mut stmt.kind {
                S::Assign {
                    annotation, value, ..
                }
                | S::SpawnBind {
                    annotation, value, ..
                } => {
                    if let Some(ty) = annotation {
                        visitor(ty);
                    }
                    expression(value, visitor);
                }
                S::Expr(value) | S::Spawn(value) | S::Return(Some(value)) => {
                    expression(value, visitor)
                }
                S::If(condition, yes, no) => {
                    expression(condition, visitor);
                    statements(yes, visitor);
                    statements(no, visitor);
                }
                S::While(condition, body) | S::For(_, condition, body) => {
                    expression(condition, visitor);
                    statements(body, visitor);
                }
                S::Scope(body) => statements(body, visitor),
                S::Match(value, arms) => {
                    expression(value, visitor);
                    for arm in arms {
                        for binding in arm.pattern.bindings_mut() {
                            if let Some(ty) = &mut binding.ty {
                                visitor(ty);
                            }
                        }
                        statements(&mut arm.body, visitor);
                    }
                }
                S::Return(None) => {}
            }
        }
    }
    for class in &mut program.classes {
        for (_, ty) in &mut class.fields {
            visitor(ty);
        }
    }
    for enumeration in &mut program.enums {
        for variant in &mut enumeration.variants {
            for (_, ty) in &mut variant.fields {
                visitor(ty);
            }
        }
    }
    for function in &mut program.functions {
        for (_, ty) in &mut function.params {
            visitor(ty);
        }
        visitor(&mut function.ret);
        statements(&mut function.body, visitor);
    }
}

/// Keep inferred intrinsic types distinct from same-spelled source aliases in
/// independently parsed Low. Ordinary programs retain their existing spelling.
pub fn prepare_low_types(program: &Program) -> Program {
    let mut out = program.clone();
    if out.modules.root.is_none() {
        return out;
    }
    let binding_names: HashSet<_> = out
        .modules
        .bindings
        .iter()
        .map(|b| b.name.clone())
        .collect();
    fn conflicting(ty: &Type, bindings: &HashSet<String>) -> bool {
        builtin_type(&ty.0) && bindings.contains(&ty.0)
            || ty.1.iter().any(|arg| conflicting(arg, bindings))
    }
    let mut needed = false;
    visit_types(&mut out, &mut |ty| {
        needed |= conflicting(ty, &binding_names)
    });
    if !needed {
        return out;
    }
    let namespace = (0usize..)
        .map(|n| format!("__nagi_builtin_{n}"))
        .find(|name| !binding_names.contains(name))
        .expect("finite binding names");
    fn qualify(ty: &mut Type, namespace: &str) {
        if builtin_type(&ty.0) {
            ty.0 = format!("{namespace}.{}", ty.0);
        }
        for arg in &mut ty.1 {
            qualify(arg, namespace);
        }
    }
    visit_types(&mut out, &mut |ty| qualify(ty, &namespace));
    out.modules.builtin_types = Some(namespace);
    out
}

/// Direct parser/check users need the same intrinsic identity decoding as the
/// source resolver. This recognizes only the metadata's exact builtin scope.
pub fn decode_low_types(program: &mut Program) -> Result<(), String> {
    let Some(namespace) = program.modules.builtin_types.clone() else {
        return Ok(());
    };
    validate(program)?;
    fn decode(ty: &mut Type, namespace: &str) -> Result<(), String> {
        if let Some(head) =
            ty.0.strip_prefix(namespace)
                .and_then(|name| name.strip_prefix('.'))
        {
            if !builtin_type(head) {
                return Err(format!("未定義のbuiltin型: {head}"));
            }
            ty.0 = head.to_owned();
        }
        for arg in &mut ty.1 {
            decode(arg, namespace)?;
        }
        Ok(())
    }
    let mut result = Ok(());
    visit_types(program, &mut |ty| {
        if result.is_ok() {
            result = decode(ty, &namespace);
        }
    });
    result?;
    program.modules.builtin_types = None;
    Ok(())
}

impl ModuleMetadata {
    pub fn is_empty(&self) -> bool {
        self.builtin_types.is_none()
            && self.root.is_none()
            && self.modules.is_empty()
            && self.definitions.is_empty()
            && self.bindings.is_empty()
            && self.references.is_empty()
    }
    pub fn definition(&self, name: &str) -> Option<&DefinitionInfo> {
        self.definitions.iter().find(|d| d.symbol == name)
    }
    pub fn definition_id(&self, id: &DefId) -> Option<&DefinitionInfo> {
        self.definitions.iter().find(|d| &d.id == id)
    }
    pub fn binding(&self, module: &ModuleId, name: &str) -> Option<&ModuleBinding> {
        self.bindings
            .iter()
            .find(|b| &b.module == module && b.name == name)
    }
    pub fn exports<'a>(&'a self, module: &'a ModuleId) -> impl Iterator<Item = &'a DefinitionInfo> {
        self.definitions
            .iter()
            .filter(move |d| &d.id.module == module)
    }
    pub fn root_bindings(&self) -> impl Iterator<Item = &ModuleBinding> {
        self.bindings
            .iter()
            .filter(|b| Some(&b.module) == self.root.as_ref())
    }
    pub fn root_public_bindings(&self) -> impl Iterator<Item = &ModuleBinding> {
        self.root_bindings().filter(|binding| {
            binding.public
                || matches!(&binding.target,
            BindingTarget::Definition(id) if id.module == binding.module && id.name == binding.name)
        })
    }
    pub fn resolve_root_path(&self, path: &str) -> Option<&DefinitionInfo> {
        let path = path.strip_prefix("generated::").unwrap_or(path);
        let parts: Vec<_> = path.split("::").flat_map(|p| p.split('.')).collect();
        let root = self.root.as_ref()?;
        match parts.as_slice() {
            [name] => match &self.binding(root, name)?.target {
                BindingTarget::Definition(id) => self.definition_id(id),
                _ => None,
            },
            [alias, name] => match &self.binding(root, alias)?.target {
                BindingTarget::Module(module) => self.exports(module).find(|d| d.id.name == *name),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn merge(&mut self, other: ModuleMetadata) -> Result<(), String> {
        if self.root.is_none() {
            self.root = other.root.clone();
        }
        for module in other.modules {
            if let Some(old) = self.modules.iter().find(|m| m.id == module.id) {
                if old.path != module.path {
                    return Err("module metadataのpathが一致しません".into());
                }
            } else {
                self.modules.push(module);
            }
        }
        for def in other.definitions {
            if let Some(old) = self.definition_id(&def.id) {
                if old.symbol != def.symbol {
                    return Err("同じ定義IDのsymbolが一致しません".into());
                }
            } else if self.definition(&def.symbol).is_some() {
                return Err("異なる定義IDが同じsymbolを使っています".into());
            } else {
                self.definitions.push(def);
            }
        }
        for binding in other.bindings {
            if let Some(old) = self.binding(&binding.module, &binding.name) {
                if old.target != binding.target {
                    return Err(format!("import名の衝突: {}", binding.name));
                }
                if binding.public {
                    if let Some(old) = self
                        .bindings
                        .iter_mut()
                        .find(|b| b.module == binding.module && b.name == binding.name)
                    {
                        old.public = true;
                    }
                }
            } else {
                self.bindings.push(binding);
            }
        }
        self.references.extend(other.references);
        Ok(())
    }
    /// Explicit native input files share one legacy root namespace.
    pub fn merge_native(&mut self, other: ModuleMetadata) -> Result<(), String> {
        let public: Vec<_> = other.root_public_bindings().cloned().collect();
        self.merge(other)?;
        if let Some(root) = self.root.clone() {
            for mut binding in public {
                binding.module = root.clone();
                insert_binding(self, binding)?;
            }
        }
        Ok(())
    }
}

/// A parsed source file. Imports point to canonical files or registered IDs.
pub(crate) struct ModuleUnit {
    pub id: ModuleId,
    pub program: Program,
    pub imports: Vec<(ModuleImport, ModuleId)>,
    pub tokens: Vec<Token>,
    pub offset: usize,
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn canonical_identity(path: &str) -> bool {
    // Saved IDs are logical data. They need not be paths on the current host:
    // generated Low can be copied between Unix and Windows without changing IDs.
    if path.len() > 32768 || path.contains('\0') {
        return false;
    }
    let clean = |parts: &[&str]| {
        parts
            .iter()
            .all(|s| !s.is_empty() && *s != "." && *s != "..")
    };
    let drive = |text: &str, separator: char| {
        let bytes = text.as_bytes();
        bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == separator as u8
            && !text[3..].contains(if separator == '\\' { '/' } else { '\\' })
            && (text.len() == 3 || clean(&text[3..].split(separator).collect::<Vec<_>>()))
    };
    let unc = |text: &str, separator: char| {
        let parts: Vec<_> = text.split(separator).collect();
        parts.len() >= 2
            && clean(&parts)
            && !text.contains(if separator == '\\' { '/' } else { '\\' })
    };
    if let Some(text) = path.strip_prefix(r"\\?\UNC\") {
        return unc(text, '\\');
    }
    if let Some(text) = path.strip_prefix(r"\\?\") {
        return drive(text, '\\');
    }
    if let Some(text) = path.strip_prefix(r"\\") {
        return unc(text, '\\');
    }
    if let Some(text) = path.strip_prefix("//") {
        return unc(text, '/');
    }
    if drive(path, '\\') || drive(path, '/') {
        return true;
    }
    path == "/"
        || path
            .strip_prefix('/')
            .is_some_and(|text| clean(&text.split('/').collect::<Vec<_>>()))
}

/// Serialized Low metadata is data, never authority to rename arbitrary code.
pub fn validate(program: &Program) -> Result<(), String> {
    let m = &program.modules;
    if m.is_empty() {
        return Ok(());
    }
    if m.modules
        .iter()
        .filter(|module| !crate::stdlib::is_registered_module(&module.id))
        .count()
        > 128
        || m.definitions.len() > 100_000
        || m.bindings.len() > 100_000
        || m.references.len() > 500_000
    {
        return Err("module metadataの上限を超えました".into());
    }
    if let Some(namespace) = &m.builtin_types {
        if !namespace
            .strip_prefix("__nagi_builtin_")
            .is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix.len() <= 20
                    && suffix.bytes().all(|b| b.is_ascii_digit())
            })
            || m.bindings.iter().any(|binding| &binding.name == namespace)
        {
            return Err("module metadataのbuiltin型namespaceが不正です".into());
        }
    }
    let mut modules = HashSet::new();
    for module in &m.modules {
        if !(canonical_identity(&module.id.0) || crate::stdlib::is_registered_module(&module.id))
            || module.path != module.id.0
            || !modules.insert(module.id.clone())
        {
            return Err("不正または重複したmodule ID/pathです".into());
        }
    }
    let root = m
        .root
        .as_ref()
        .filter(|id| modules.contains(*id) && !crate::stdlib::is_registered_module(id))
        .ok_or("module metadataのrootが不正です")?;
    let mut ids = HashSet::new();
    let mut symbols = HashSet::new();
    for def in &m.definitions {
        if !modules.contains(&def.id.module)
            || !valid_name(&def.id.name)
            || !ids.insert(def.id.clone())
            || !symbols.insert(def.symbol.clone())
            || def.line == 0
        {
            return Err("module metadataの定義IDが不正または重複しています".into());
        }
        if crate::stdlib::is_registered_module(&def.id.module)
            && !crate::stdlib::definition(&def.id)
        {
            return Err("module metadataに未登録のstd定義があります".into());
        }
        let main = def.id.kind == DefKind::Function
            && !crate::stdlib::is_registered_module(&def.id.module)
            && m.binding(root, "main")
                .is_some_and(|b| b.target == BindingTarget::Definition(def.id.clone()));
        if def.symbol
            != if main {
                "main".to_owned()
            } else {
                symbol(&def.id)
            }
        {
            return Err("module metadataの内部symbolが定義IDと一致しません".into());
        }
    }
    let mut ast_symbols = HashSet::new();
    for (name, kind) in program
        .classes
        .iter()
        .map(|c| (&c.name, DefKind::Class))
        .chain(program.enums.iter().map(|e| (&e.name, DefKind::Enum)))
        .chain(
            program
                .functions
                .iter()
                .map(|f| (&f.name, DefKind::Function)),
        )
    {
        if !ast_symbols.insert(name)
            || !m.definition(name).is_some_and(|d| {
                d.id.kind == kind && !crate::stdlib::is_registered_module(&d.id.module)
            })
        {
            return Err("module metadataと実際の定義が一致しません".into());
        }
    }
    if ast_symbols.len()
        != m.definitions
            .iter()
            .filter(|d| !crate::stdlib::is_registered_module(&d.id.module))
            .count()
    {
        return Err("module metadataに実際の定義がないIDがあります".into());
    }
    for module in &m.modules {
        if crate::stdlib::is_registered_module(&module.id)
            && crate::stdlib::definitions(&module.id)
                .iter()
                .any(|def| m.definition_id(&def.id).is_none())
        {
            return Err("module metadataのstd定義がregistryと一致しません".into());
        }
    }
    let mut bindings = HashSet::new();
    for binding in &m.bindings {
        let target_valid = match &binding.target {
            BindingTarget::Definition(id) => ids.contains(id),
            BindingTarget::Module(id) => modules.contains(id),
        };
        if !modules.contains(&binding.module)
            || !valid_name(&binding.name)
            || !target_valid
            || binding.line == 0
            || binding.span.start > binding.span.end
            || !bindings.insert((&binding.module, &binding.name))
        {
            return Err("module metadataのbindingが不正または重複しています".into());
        }
        if crate::stdlib::is_registered_module(&binding.module)
            && !matches!(&binding.target, BindingTarget::Definition(id)
                if id.module == binding.module && id.name == binding.name
                    && crate::stdlib::definition(id) && binding.public)
        {
            return Err("module metadataのstd bindingがregistryと一致しません".into());
        }
    }
    for def in &m.definitions {
        if !m
            .binding(&def.id.module, &def.id.name)
            .is_some_and(|b| b.target == BindingTarget::Definition(def.id.clone()))
        {
            return Err("module metadataの定義bindingがありません".into());
        }
    }
    for reference in &m.references {
        if !modules.contains(&reference.module)
            || !ids.contains(&reference.target)
            || reference.line == 0
            || reference.span.start > reference.span.end
            || reference.spelling.len() > 8192
        {
            return Err("module metadataのreferenceが不正です".into());
        }
    }
    Ok(())
}

fn own_span(unit: &ModuleUnit, name: &str, kind: DefKind, line: usize) -> Span {
    unit.tokens
        .iter()
        .enumerate()
        .find(|(i, t)| {
            t.line + unit.offset == line
                && matches!(&t.kind, K::Id(n) if n == name)
                && i.checked_sub(1)
                    .and_then(|i| unit.tokens.get(i))
                    .is_some_and(|previous| {
                        matches!(&previous.kind, K::Id(keyword) if match kind {
                            DefKind::Class => keyword == "class" || keyword == "record",
                            DefKind::Enum => keyword == "enum",
                            DefKind::Function => keyword == "def" || keyword == "fn",
                            DefKind::Resource => false,
                        })
                    })
        })
        .map(|(i, _)| Span {
            start: i,
            end: i + 1,
        })
        .unwrap_or_default()
}

fn insert_binding(metadata: &mut ModuleMetadata, binding: ModuleBinding) -> Result<(), String> {
    if let Some(old) = metadata.binding(&binding.module, &binding.name) {
        if old.target == binding.target {
            if binding.public {
                if let Some(old) = metadata
                    .bindings
                    .iter_mut()
                    .find(|b| b.module == binding.module && b.name == binding.name)
                {
                    old.public = true;
                }
            }
            return Ok(());
        }
        return Err(format!(
            "line {}: import名の衝突: {}",
            binding.line, binding.name
        ));
    }
    metadata.bindings.push(binding);
    Ok(())
}

pub(crate) fn register_standard_module(
    metadata: &mut ModuleMetadata,
    id: &ModuleId,
) -> Result<(), String> {
    if !crate::stdlib::is_registered_module(id) {
        return Err("未登録のstd moduleです".into());
    }
    // Cross-module types in standard signatures must survive saved Low without
    // requiring applications to import those modules just to infer a result.
    // These are canonical module definitions, not aliases in the user's scope.
    if id.0 == crate::stdlib::AUTH_SESSION_MODULE_ID {
        for dependency in [
            crate::stdlib::AUTH_MODULE_ID,
            crate::stdlib::MODULE_ID,
            crate::stdlib::SQLITE_MODULE_ID,
        ] {
            register_standard_module(metadata, &ModuleId(dependency.into()))?;
        }
    }
    let definitions = crate::stdlib::definitions(id);
    let bindings = definitions
        .iter()
        .map(|definition| ModuleBinding {
            module: id.clone(),
            name: definition.id.name.clone(),
            target: BindingTarget::Definition(definition.id.clone()),
            public: true,
            line: definition.line,
            span: Span::default(),
        })
        .collect();
    metadata.merge(ModuleMetadata {
        modules: vec![ModuleInfo {
            id: id.clone(),
            path: id.0.clone(),
        }],
        definitions,
        bindings,
        ..Default::default()
    })
}

/// Resolve each file against its own namespace before assembling a checked AST.
pub(crate) fn resolve(mut units: Vec<ModuleUnit>, root: ModuleId) -> Result<Program, String> {
    for unit in &mut units {
        if !unit.program.modules.is_empty() {
            validate(&unit.program)?;
            rebase_loaded(&mut unit.program, &unit.tokens, unit.offset);
            if unit.program.modules.root.as_ref() != Some(&root) {
                if let Some(def) = unit.program.modules.definition("main").cloned() {
                    let internal = symbol(&def.id);
                    remap_definition(&mut unit.program, "main", &internal);
                }
            }
        }
    }
    let mut metadata = ModuleMetadata {
        root: Some(root.clone()),
        ..Default::default()
    };
    if units.iter().any(|u| {
        u.program
            .functions
            .iter()
            .any(|f| contains_task_binding(&f.body))
    }) {
        register_standard_module(
            &mut metadata,
            &ModuleId(crate::stdlib::TASK_MODULE_ID.into()),
        )?;
    }
    for unit in &units {
        if !unit.program.modules.is_empty() {
            metadata.merge(unit.program.modules.clone())?;
            continue;
        }
        metadata.modules.push(ModuleInfo {
            id: unit.id.clone(),
            path: unit.id.0.clone(),
        });
        for (name, kind, line) in unit
            .program
            .classes
            .iter()
            .map(|c| (&c.name, DefKind::Class, c.line))
            .chain(
                unit.program
                    .enums
                    .iter()
                    .map(|e| (&e.name, DefKind::Enum, e.line)),
            )
            .chain(
                unit.program
                    .functions
                    .iter()
                    .map(|f| (&f.name, DefKind::Function, f.line)),
            )
        {
            let id = DefId {
                module: unit.id.clone(),
                kind,
                name: name.clone(),
            };
            if metadata.binding(&unit.id, name).is_some() {
                return Err(format!("line {line}: 定義の重複: {name}"));
            }
            let internal = if unit.id == root && kind == DefKind::Function && name == "main" {
                "main".into()
            } else {
                symbol(&id)
            };
            metadata.definitions.push(DefinitionInfo {
                id: id.clone(),
                symbol: internal,
                line,
            });
            metadata.bindings.push(ModuleBinding {
                module: unit.id.clone(),
                name: name.clone(),
                target: BindingTarget::Definition(id),
                public: true,
                line,
                span: own_span(unit, name, kind, line),
            });
        }
    }
    for unit in &units {
        for (import, id) in &unit.imports {
            if import.source == ImportSource::Standard {
                register_standard_module(&mut metadata, id)?;
            }
        }
    }
    // Dependencies precede importing files. Flat imports retain their transitive
    // definition visibility but do not re-export namespace/from aliases.
    let mut flat_exports: HashMap<ModuleId, Vec<DefId>> = HashMap::new();
    for unit in &units {
        let id = unit.program.modules.root.as_ref().unwrap_or(&unit.id);
        let mut exposed: Vec<_> = metadata.exports(id).map(|d| d.id.clone()).collect();
        if !unit.program.modules.is_empty() {
            for binding in unit.program.modules.root_public_bindings() {
                if let BindingTarget::Definition(def) = &binding.target {
                    if !exposed.contains(def) {
                        exposed.push(def.clone());
                    }
                }
            }
        }
        for (import, dependency) in &unit.imports {
            let target = units
                .iter()
                .find(|u| u.id == *dependency)
                .and_then(|u| u.program.modules.root.as_ref())
                .unwrap_or(dependency);
            match &import.kind {
                ImportKind::Flat => {
                    let defs = flat_exports.get(dependency).cloned().unwrap_or_else(|| {
                        metadata.exports(target).map(|d| d.id.clone()).collect()
                    });
                    for def in defs {
                        insert_binding(
                            &mut metadata,
                            ModuleBinding {
                                module: id.clone(),
                                name: def.name.clone(),
                                target: BindingTarget::Definition(def.clone()),
                                public: true,
                                line: import.line,
                                span: import.span,
                            },
                        )?;
                        if !exposed.contains(&def) {
                            exposed.push(def);
                        }
                    }
                }
                ImportKind::Module { alias, alias_span } => {
                    insert_binding(
                        &mut metadata,
                        ModuleBinding {
                            module: id.clone(),
                            name: alias.clone(),
                            target: BindingTarget::Module(target.clone()),
                            public: false,
                            line: import.line,
                            span: *alias_span,
                        },
                    )?;
                }
                ImportKind::Names(names) => {
                    for name in names {
                        let def = metadata
                            .exports(target)
                            .find(|d| d.id.name == name.name)
                            .ok_or_else(|| {
                                format!(
                                    "line {}: moduleに定義がありません: {}",
                                    import.line, name.name
                                )
                            })?
                            .id
                            .clone();
                        insert_binding(
                            &mut metadata,
                            ModuleBinding {
                                module: id.clone(),
                                name: name.alias.clone(),
                                target: BindingTarget::Definition(def.clone()),
                                public: false,
                                line: import.line,
                                span: name.alias_span,
                            },
                        )?;
                        metadata.references.push(ModuleReference {
                            module: id.clone(),
                            line: import.line,
                            span: name.name_span,
                            target: def.clone(),
                            spelling: name.name.clone(),
                        });
                        if name.alias_span.start != name.name_span.start {
                            metadata.references.push(ModuleReference {
                                module: id.clone(),
                                line: import.line,
                                span: name.alias_span,
                                target: def,
                                spelling: name.alias.clone(),
                            });
                        }
                    }
                }
            }
        }
        flat_exports.insert(unit.id.clone(), exposed);
    }
    // Entry selection follows the root namespace. A legacy flat dependency can
    // supply main; a module-only import never makes its main the entrypoint.
    if let Some(BindingTarget::Definition(id)) = metadata.binding(&root, "main").map(|b| &b.target)
    {
        let id = id.clone();
        if id.kind == DefKind::Function && !crate::stdlib::is_registered_module(&id.module) {
            if let Some(def) = metadata.definitions.iter_mut().find(|d| d.id == id) {
                let old = def.symbol.clone();
                def.symbol = "main".into();
                if old != "main" {
                    for unit in &mut units {
                        if !unit.program.modules.is_empty() {
                            remap_definition(&mut unit.program, &old, "main");
                        }
                    }
                }
            }
        }
    }
    let flat = FlatBindings::new(&metadata)?;
    let mut out = Program::default();
    for unit in &mut units {
        let intern = unit.program.modules.is_empty();
        let scope = unit
            .program
            .modules
            .root
            .as_ref()
            .unwrap_or(&unit.id)
            .clone();
        let mut resolver = Resolver::new(&metadata, &flat, &scope, &unit.tokens, unit.offset);
        resolver.program(&mut unit.program, intern)?;
        out.modules.references.extend(resolver.references);
        crate::source::resolve_assets(&mut unit.program, Path::new(&unit.id.0))?;
        out.classes.append(&mut unit.program.classes);
        out.enums.append(&mut unit.program.enums);
        out.functions.append(&mut unit.program.functions);
    }
    metadata.references.append(&mut out.modules.references);
    out.modules = metadata;
    Ok(out)
}

/// Renaming a resolved definition changes its uses, never field or local names.
pub fn remap_definition(program: &mut Program, old: &str, new: &str) {
    fn ty(t: &mut Type, old: &str, new: &str) {
        if t.0 == old {
            t.0 = new.to_owned();
        }
        for arg in &mut t.1 {
            ty(arg, old, new);
        }
    }
    fn name(n: &mut String, old: &str, new: &str) {
        if n == old {
            *n = new.to_owned();
        } else if let Some(suffix) = n.strip_prefix(old).and_then(|rest| rest.strip_prefix('.')) {
            *n = format!("{new}.{suffix}");
        }
    }
    fn expr(e: &mut Expr, locals: &HashSet<String>, old: &str, new: &str) {
        if let Some(t) = &mut e.ty {
            ty(t, old, new);
        }
        match &mut e.kind {
            E::Name(n) => {
                if !locals.contains(n) {
                    name(n, old, new);
                }
            }
            E::Call(n, ts, args) => {
                if !locals.contains(n) {
                    name(n, old, new);
                }
                for t in ts {
                    ty(t, old, new);
                }
                for arg in args {
                    expr(arg, locals, old, new);
                }
            }
            E::Record(n, fields) => {
                name(n, old, new);
                for (_, e) in fields {
                    expr(e, locals, old, new);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expr(a, locals, old, new);
                expr(b, locals, old, new);
            }
            E::Unary(_, e) | E::Field(e, _) | E::Try(e) | E::Await(e) => expr(e, locals, old, new),
            E::List(args) => {
                for arg in args {
                    expr(arg, locals, old, new);
                }
            }
            _ => {}
        }
    }
    fn block(stmts: &mut [Stmt], locals: &mut HashSet<String>, old: &str, new: &str) {
        for stmt in stmts {
            if let Some(t) = &mut stmt.binding_type {
                ty(t, old, new);
            }
            match &mut stmt.kind {
                S::Assign {
                    name,
                    annotation,
                    value,
                    ..
                }
                | S::SpawnBind {
                    name,
                    annotation,
                    value,
                    ..
                } => {
                    if let Some(t) = annotation {
                        ty(t, old, new);
                    }
                    expr(value, locals, old, new);
                    locals.insert(name.clone());
                }
                S::Return(Some(e)) | S::Expr(e) | S::Spawn(e) => expr(e, locals, old, new),
                S::If(e, a, b) => {
                    expr(e, locals, old, new);
                    block(a, &mut locals.clone(), old, new);
                    block(b, &mut locals.clone(), old, new);
                }
                S::While(e, b) => {
                    expr(e, locals, old, new);
                    block(b, &mut locals.clone(), old, new);
                }
                S::For(name, e, b) => {
                    expr(e, locals, old, new);
                    let mut child = locals.clone();
                    child.insert(name.clone());
                    block(b, &mut child, old, new);
                }
                S::Scope(b) => block(b, &mut locals.clone(), old, new),
                S::Match(e, arms) => {
                    expr(e, locals, old, new);
                    for arm in arms {
                        if let MatchPattern::Enum { name: path, .. } = &mut arm.pattern {
                            name(path, old, new);
                        }
                        let mut child = locals.clone();
                        for binding in arm.pattern.bindings_mut() {
                            if let Some(t) = &mut binding.ty {
                                ty(t, old, new);
                            }
                            if let Some(n) = &binding.name {
                                child.insert(n.clone());
                            }
                        }
                        block(&mut arm.body, &mut child, old, new);
                    }
                }
                S::Return(None) => {}
            }
        }
    }
    for c in &mut program.classes {
        if c.name == old {
            c.name = new.to_owned();
        }
        for (_, t) in &mut c.fields {
            ty(t, old, new);
        }
    }
    for enumeration in &mut program.enums {
        if enumeration.name == old {
            enumeration.name = new.to_owned();
        }
        for variant in &mut enumeration.variants {
            for (_, t) in &mut variant.fields {
                ty(t, old, new);
            }
        }
    }
    for f in &mut program.functions {
        if f.name == old {
            f.name = new.to_owned();
        }
        for (_, t) in &mut f.params {
            ty(t, old, new);
        }
        ty(&mut f.ret, old, new);
        block(
            &mut f.body,
            &mut f.params.iter().map(|(n, _)| n.clone()).collect(),
            old,
            new,
        );
    }
    for def in &mut program.modules.definitions {
        if def.symbol == old {
            def.symbol = new.to_owned();
        }
    }
}

fn rebase_loaded(program: &mut Program, tokens: &[Token], offset: usize) {
    synchronize(program);
    // Import aliases have no declaration token in flattened generated Low.
    for binding in &mut program.modules.bindings {
        binding.span = Span::default();
        if let BindingTarget::Definition(id) = &binding.target {
            if let Some(def) = program.modules.definitions.iter().find(|d| &d.id == id) {
                binding.line = def.line;
            }
        } else {
            binding.line = offset + 1;
        }
    }
    program.modules.references.clear();
    for (i, token) in tokens.iter().enumerate() {
        if let K::Id(name) = &token.kind {
            if let Some(def) = program.modules.definition(name) {
                let variant = if matches!(def.id.kind, DefKind::Enum | DefKind::Resource)
                    && matches!(tokens.get(i + 1).map(|t| &t.kind), Some(K::Sym(dot)) if dot == ".")
                {
                    tokens.get(i + 2).and_then(|t| match &t.kind {
                        K::Id(variant) => Some(variant.as_str()),
                        _ => None,
                    })
                } else {
                    None
                };
                program.modules.references.push(ModuleReference {
                    module: if crate::stdlib::is_registered_module(&def.id.module) {
                        program.modules.root.clone().expect("validated root")
                    } else {
                        def.id.module.clone()
                    },
                    line: token.line + offset,
                    span: Span {
                        start: i,
                        end: i + if variant.is_some() { 3 } else { 1 },
                    },
                    target: def.id.clone(),
                    spelling: variant
                        .map(|v| format!("{name}.{v}"))
                        .unwrap_or_else(|| name.clone()),
                });
            }
        }
    }
}

/// Keep metadata aligned with a projected or integrated set of AST definitions.
/// This does not resolve or rename code, and preserves the preferred root ID.
pub fn synchronize(program: &mut Program) {
    let ast: HashMap<_, _> = program
        .classes
        .iter()
        .map(|c| (c.name.clone(), (DefKind::Class, c.line)))
        .chain(
            program
                .enums
                .iter()
                .map(|e| (e.name.clone(), (DefKind::Enum, e.line))),
        )
        .chain(
            program
                .functions
                .iter()
                .map(|f| (f.name.clone(), (DefKind::Function, f.line))),
        )
        .collect();
    program.modules.definitions.retain_mut(|def| {
        if crate::stdlib::definition(&def.id) && def.symbol == symbol(&def.id) {
            if let Some(registered) = crate::stdlib::definitions(&def.id.module)
                .into_iter()
                .find(|registered| registered.id == def.id)
            {
                def.line = registered.line;
            }
            return true;
        }
        if let Some((kind, line)) = ast.get(&def.symbol) {
            if *kind == def.id.kind {
                def.line = *line;
                return true;
            }
        }
        false
    });
    let ids: HashSet<_> = program
        .modules
        .definitions
        .iter()
        .map(|d| d.id.clone())
        .collect();
    program
        .modules
        .bindings
        .retain(|binding| match &binding.target {
            BindingTarget::Definition(id) => ids.contains(id),
            BindingTarget::Module(_) => true,
        });
    program
        .modules
        .references
        .retain(|reference| ids.contains(&reference.target));
}

/// Legacy flat imports shared public definitions across their full connected
/// group, including callers defined in an importing file. Private import
/// aliases never join those groups. The existing serialized binding provenance
/// reconstructs these links without duplicating every name into every file.
struct FlatBindings {
    modules: HashMap<ModuleId, usize>,
    groups: Vec<HashMap<String, DefId>>,
}
impl FlatBindings {
    fn new(metadata: &ModuleMetadata) -> Result<Self, String> {
        let mut links: HashMap<ModuleId, Vec<ModuleId>> = HashMap::new();
        for binding in &metadata.bindings {
            if let BindingTarget::Definition(id) = &binding.target {
                if binding.public && binding.module != id.module && binding.name == id.name {
                    links
                        .entry(binding.module.clone())
                        .or_default()
                        .push(id.module.clone());
                    links
                        .entry(id.module.clone())
                        .or_default()
                        .push(binding.module.clone());
                }
            }
        }
        let mut out = Self {
            modules: HashMap::new(),
            groups: vec![],
        };
        for module in &metadata.modules {
            if out.modules.contains_key(&module.id) {
                continue;
            }
            let group = out.groups.len();
            out.groups.push(HashMap::new());
            let mut pending = vec![module.id.clone()];
            while let Some(module) = pending.pop() {
                if out.modules.contains_key(&module) {
                    continue;
                }
                out.modules.insert(module.clone(), group);
                if let Some(neighbors) = links.get(&module) {
                    pending.extend(neighbors.iter().cloned());
                }
            }
        }
        for definition in &metadata.definitions {
            if let Some(group) = out.modules.get(&definition.id.module) {
                if let Some(old) =
                    out.groups[*group].insert(definition.id.name.clone(), definition.id.clone())
                {
                    if old != definition.id {
                        return Err(format!(
                            "line {}: flat importの定義名が重複しています: {}",
                            definition.line, definition.id.name
                        ));
                    }
                }
            }
        }
        Ok(out)
    }
    fn target(&self, module: &ModuleId, name: &str) -> Option<&DefId> {
        self.groups.get(*self.modules.get(module)?)?.get(name)
    }
}

struct Resolver<'a> {
    metadata: &'a ModuleMetadata,
    flat: &'a FlatBindings,
    module: &'a ModuleId,
    tokens: &'a [Token],
    offset: usize,
    references: Vec<ModuleReference>,
    locals: HashMap<String, String>,
    used: HashSet<String>,
    serial: usize,
    extra: HashMap<String, DefId>,
    raw: bool,
    builtin_types: Option<String>,
}
impl<'a> Resolver<'a> {
    fn new(
        metadata: &'a ModuleMetadata,
        flat: &'a FlatBindings,
        module: &'a ModuleId,
        tokens: &'a [Token],
        offset: usize,
    ) -> Self {
        let used = metadata
            .definitions
            .iter()
            .map(|d| d.symbol.clone())
            .chain(tokens.iter().filter_map(|t| match &t.kind {
                K::Id(name) => Some(name.clone()),
                _ => None,
            }))
            .collect();
        Self {
            metadata,
            flat,
            module,
            tokens,
            offset,
            references: vec![],
            locals: HashMap::new(),
            used,
            serial: 0,
            extra: HashMap::new(),
            raw: false,
            builtin_types: None,
        }
    }
    fn target(&self, name: &str, line: usize) -> Result<Option<DefId>, String> {
        if self.raw
            && crate::stdlib::contains_symbol(name)
            && self.metadata.definition(name).is_none()
        {
            return Err(format!(
                "line {line}: このファイルから参照できないstd定義です: {}",
                display_symbol(name)
            ));
        }
        if !self.raw && self.metadata.definition(name).is_some() {
            return Ok(None);
        }
        if self.raw
            && self.metadata.definition(name).is_some()
            && self.metadata.binding(self.module, name).is_none()
            && self.flat.target(self.module, name).is_none()
            && !self.extra.contains_key(name)
        {
            return Err(format!(
                "line {line}: このファイルから参照できない定義です: {}",
                display_symbol(name)
            ));
        }
        let parts: Vec<_> = name.split('.').collect();
        if parts.len() > 1 {
            if self.locals.contains_key(parts[0]) {
                return Ok(None);
            }
            let Some(binding) = self.metadata.binding(self.module, parts[0]) else {
                return Ok(None);
            };
            if let BindingTarget::Module(module) = &binding.target {
                if parts.len() != 2 {
                    return Err(format!(
                        "line {line}: moduleの名前は再公開されません: {name}"
                    ));
                }
                return self
                    .metadata
                    .exports(module)
                    .find(|d| d.id.name == parts[1])
                    .map(|d| Some(d.id.clone()))
                    .ok_or_else(|| format!("line {line}: moduleに定義がありません: {name}"));
            }
            return Ok(None);
        }
        match self.metadata.binding(self.module, name).map(|b| &b.target) {
            Some(BindingTarget::Definition(id)) => Ok(Some(id.clone())),
            Some(BindingTarget::Module(_)) => Ok(None),
            None => Ok(self
                .flat
                .target(self.module, name)
                .or_else(|| self.extra.get(name))
                .cloned()),
        }
    }
    fn reference(&mut self, spelling: String, target: DefId, line: usize, span: Span) {
        self.references.push(ModuleReference {
            module: self.module.clone(),
            line,
            span,
            target,
            spelling,
        });
    }
    fn legacy_binding(&self, name: &str, target: &DefId) -> bool {
        !name.contains('.')
            && name == target.name
            && self
                .metadata
                .binding(self.module, name)
                .map(|binding| binding.public || binding.module == target.module)
                .unwrap_or_else(|| {
                    self.flat.target(self.module, name) == Some(target)
                        || self.extra.get(name) == Some(target)
                })
    }
    fn resolve_name(
        &mut self,
        name: &mut String,
        line: usize,
        span: Span,
        kind: Option<DefKind>,
    ) -> Result<(), String> {
        // Named fields identify a record constructor, whose class lookup has
        // always been independent from same-spelled local values.
        if !matches!(kind, Some(DefKind::Class | DefKind::Enum)) {
            if let Some(local) = self.locals.get(name) {
                *name = local.clone();
                return Ok(());
            }
        }
        if let Some(target) = self.target(name, line)? {
            if kind == Some(DefKind::Function)
                && target.kind == DefKind::Class
                && self.legacy_binding(name, &target)
                && (!self.raw || self.metadata.definition(name).is_none())
            {
                return Ok(());
            }
            let spelling = name.clone();
            *name = self
                .metadata
                .definition_id(&target)
                .expect("binding target validated")
                .symbol
                .clone();
            self.reference(spelling, target, line, span);
        }
        Ok(())
    }
    fn type_tokens(&mut self, start: usize, end: usize) -> Result<(), String> {
        let mut i = start;
        while i < end.min(self.tokens.len()) {
            let K::Id(mut name) = self.tokens[i].kind.clone() else {
                i += 1;
                continue;
            };
            let first = i;
            i += 1;
            while i + 1 < end && matches!(&self.tokens[i].kind, K::Sym(s) if s == ".") {
                let K::Id(member) = &self.tokens[i + 1].kind else {
                    break;
                };
                name.push('.');
                name.push_str(member);
                i += 2;
            }
            if matches!(
                name.as_str(),
                "List"
                    | "view"
                    | "owned"
                    | "shared"
                    | "Option"
                    | "Map"
                    | "Result"
                    | "Future"
                    | "fn"
            ) && matches!(self.tokens.get(i).map(|t| &t.kind), Some(K::Sym(s)) if s == "[")
            {
                continue;
            }
            let line = self.tokens[first].line + self.offset;
            if let Some(target) = self.target(&name, line)? {
                if matches!(
                    target.kind,
                    DefKind::Class | DefKind::Enum | DefKind::Resource
                ) {
                    self.reference(
                        name,
                        target,
                        line,
                        Span {
                            start: first,
                            end: i,
                        },
                    );
                }
            }
        }
        Ok(())
    }
    fn annotation_references(&mut self) -> Result<(), String> {
        // Colons and return arrows introduce type syntax. Nested generic commas
        // stay in the same range, and every repeated occurrence keeps its span.
        for start in 0..self.tokens.len() {
            if !matches!(&self.tokens[start].kind, K::Sym(s) if s == ":" || s == "->") {
                continue;
            }
            if !self.raw {
                let line = self.tokens[start].line + self.offset;
                if let Some(definition) = self
                    .metadata
                    .definitions
                    .iter()
                    .filter(|definition| {
                        !crate::stdlib::is_registered_module(&definition.id.module)
                    })
                    .filter(|definition| definition.line <= line)
                    .max_by_key(|definition| definition.line)
                {
                    self.module = &definition.id.module;
                }
            }
            let mut depth = 0usize;
            let mut end = start + 1;
            while end < self.tokens.len() {
                match &self.tokens[end].kind {
                    K::Sym(s) if s == "[" => depth += 1,
                    K::Sym(s) if s == "]" && depth > 0 => depth -= 1,
                    K::Sym(s)
                        if depth == 0
                            && matches!(s.as_str(), "=" | ";" | "," | ")" | "{" | "}" | ":") =>
                    {
                        break
                    }
                    K::Newline | K::Indent | K::Dedent | K::Eof => break,
                    _ => {}
                }
                end += 1;
            }
            self.type_tokens(start + 1, end)?;
        }
        Ok(())
    }
    fn ty(&mut self, ty: &mut Type, line: usize) -> Result<(), String> {
        if let Some(namespace) = &self.builtin_types {
            if let Some(head) =
                ty.0.strip_prefix(namespace)
                    .and_then(|name| name.strip_prefix('.'))
            {
                if !builtin_type(head) {
                    return Err(format!("line {line}: 未定義のbuiltin型: {head}"));
                }
                ty.0 = head.to_owned();
                for arg in &mut ty.1 {
                    self.ty(arg, line)?;
                }
                return Ok(());
            }
        }
        // Rebinding checked programs must preserve inferred intrinsic types.
        // Unresolved ordinary helper types still use the combined context.
        if !self.raw && self.tokens.is_empty() && builtin_type(&ty.0) {
            for arg in &mut ty.1 {
                self.ty(arg, line)?;
            }
            return Ok(());
        }
        if (self.raw || self.metadata.definition(&ty.0).is_none())
            && self
                .metadata
                .binding(self.module, &ty.0)
                .is_some_and(|b| matches!(b.target, BindingTarget::Module(_)))
        {
            ty.0 = format!("{NAMESPACE_PREFIX}{}", ty.0);
        }
        // Generic builtin constructors are independent from ordinary arity-zero
        // user records (notably a record named Future).
        let builtin = !ty.1.is_empty()
            && matches!(
                ty.0.as_str(),
                "List"
                    | "view"
                    | "owned"
                    | "shared"
                    | "Option"
                    | "Map"
                    | "Result"
                    | "Future"
                    | "fn"
            );
        if !builtin {
            if let Some(target) = self.target(&ty.0, line)? {
                // Ordinary functions never replaced intrinsic type names in
                // the legacy namespace. Explicit from aliases are bindings.
                if target.kind != DefKind::Function
                    || !self.legacy_binding(&ty.0, &target)
                    || self.raw && self.metadata.definition(&ty.0).is_some()
                {
                    ty.0 = self
                        .metadata
                        .definition_id(&target)
                        .expect("binding target validated")
                        .symbol
                        .clone();
                }
            }
        }
        for arg in &mut ty.1 {
            self.ty(arg, line)?;
        }
        Ok(())
    }
    fn bind(&mut self, name: &mut String) {
        let original = name.clone();
        if let Some(local) = self.locals.get(&original) {
            *name = local.clone();
            return;
        }
        if self.metadata.definition(&original).is_some() {
            loop {
                let candidate = format!("__nagi_local_{}_{}", hex(&self.module.0), self.serial);
                self.serial += 1;
                if !self.used.contains(&candidate) {
                    self.used.insert(candidate.clone());
                    *name = candidate;
                    break;
                }
            }
        }
        self.used.insert(name.clone());
        self.locals.insert(original, name.clone());
    }
    fn resolve_variant_name(
        &mut self,
        name: &mut String,
        line: usize,
        span: Span,
        value_scope: bool,
    ) -> Result<bool, String> {
        let Some((parent, variant)) = name.rsplit_once('.') else {
            return Ok(false);
        };
        if parent.split('.').count() > 2 {
            // A field on a qualified constant is resolved after its receiver.
            // The module prefix alone does not make every later field a name.
            return Ok(false);
        }
        if value_scope
            && self
                .locals
                .contains_key(parent.split('.').next().unwrap_or(parent))
        {
            return Ok(false);
        }
        let target = if !self.raw {
            self.metadata.definition(parent).map(|d| d.id.clone())
        } else {
            None
        };
        let target = match target {
            Some(target) => Some(target),
            None => self.target(parent, line)?,
        };
        let Some(target) =
            target.filter(|target| matches!(target.kind, DefKind::Enum | DefKind::Resource))
        else {
            return Ok(false);
        };
        let spelling = name.clone();
        let symbol = &self
            .metadata
            .definition_id(&target)
            .expect("binding target validated")
            .symbol;
        *name = format!("{symbol}.{variant}");
        self.reference(spelling, target, line, span);
        Ok(true)
    }
    fn expr(&mut self, expr: &mut Expr) -> Result<(), String> {
        if let Some(ty) = &mut expr.ty {
            self.ty(ty, expr.line)?;
        }
        let bare = match &expr.kind {
            E::Name(name) | E::Call(name, _, _) | E::Record(name, _) => Some(name),
            _ => None,
        };
        // Asset resolution runs before type checking. Preserve this known
        // value binding so a callback named include_text is not read as a file.
        if matches!(&expr.kind, E::Call(name, _, _) if self.locals.contains_key(name)) {
            expr.resolution = Some(NameResolution::Local);
        }
        if bare.is_some_and(|name| {
            !self.locals.contains_key(name)
                && (self.raw || self.metadata.definition(name).is_none())
                && self
                    .metadata
                    .binding(self.module, name)
                    .is_some_and(|b| matches!(b.target, BindingTarget::Module(_)))
        }) {
            expr.resolution = Some(NameResolution::Module);
        }
        // Enum variants share their parent's definition identity. Resolve a
        // qualified type path without granting access to another file's aliases.
        fn path(expr: &Expr) -> Option<String> {
            match &expr.kind {
                E::Name(name) => Some(name.clone()),
                E::Field(base, member) => Some(format!("{}.{member}", path(base)?)),
                _ => None,
            }
        }
        if matches!(expr.kind, E::Field(_, _)) {
            if let Some(mut name) = path(expr) {
                if self.resolve_variant_name(&mut name, expr.line, expr.span, true)? {
                    let (parent, variant) = name.rsplit_once('.').expect("qualified variant");
                    let parent_span = Span {
                        start: expr.span.start,
                        end: expr.span.end.saturating_sub(2),
                    };
                    expr.kind = E::Field(
                        Box::new(Expr {
                            kind: E::Name(parent.to_owned()),
                            line: expr.line,
                            ty: None,
                            resolution: None,
                            span: parent_span,
                        }),
                        variant.to_owned(),
                    );
                    return Ok(());
                }
            }
        }
        // Field syntax becomes a definition only if its root resolves to a
        // module binding; local record fields retain their original structure.
        if let E::Field(base, member) = &expr.kind {
            if let E::Name(alias) = &base.kind {
                if !self.locals.contains_key(alias)
                    && self
                        .metadata
                        .binding(self.module, alias)
                        .is_some_and(|b| matches!(b.target, BindingTarget::Module(_)))
                {
                    let mut name = format!("{alias}.{member}");
                    self.resolve_name(&mut name, expr.line, expr.span, None)?;
                    expr.kind = E::Name(name);
                    return Ok(());
                }
            }
        }
        match &mut expr.kind {
            E::Name(name) => self.resolve_name(name, expr.line, expr.span, None)?,
            E::Call(name, types, args) => {
                if !self.resolve_variant_name(name, expr.line, expr.span, true)? {
                    self.resolve_name(name, expr.line, expr.span, Some(DefKind::Function))?;
                }
                if !types.is_empty() {
                    if let Some(start) = (expr.span.start..expr.span.end.min(self.tokens.len()))
                        .find(|i| matches!(&self.tokens[*i].kind, K::Sym(s) if s == "["))
                    {
                        let mut depth = 1;
                        let mut end = start + 1;
                        while end < expr.span.end.min(self.tokens.len()) {
                            match &self.tokens[end].kind {
                                K::Sym(s) if s == "[" => depth += 1,
                                K::Sym(s) if s == "]" => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            end += 1;
                        }
                        self.type_tokens(start + 1, end)?;
                    }
                }
                for ty in types {
                    self.ty(ty, expr.line)?;
                }
                for arg in args {
                    self.expr(arg)?;
                }
            }
            E::Record(name, fields) => {
                if !self.resolve_variant_name(name, expr.line, expr.span, false)? {
                    self.resolve_name(name, expr.line, expr.span, Some(DefKind::Class))?;
                }
                for (_, field) in fields {
                    self.expr(field)?;
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                self.expr(a)?;
                self.expr(b)?;
            }
            E::Unary(_, a) | E::Field(a, _) | E::Await(a) | E::Try(a) => self.expr(a)?,
            E::List(args) => {
                for arg in args {
                    self.expr(arg)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn child(&mut self, stmts: &mut [Stmt]) -> Result<(), String> {
        let before = self.locals.clone();
        let result = self.block(stmts);
        self.locals = before;
        result
    }
    fn block(&mut self, stmts: &mut [Stmt]) -> Result<(), String> {
        for stmt in stmts {
            if let Some(ty) = &mut stmt.binding_type {
                self.ty(ty, stmt.line)?;
            }
            match &mut stmt.kind {
                S::Assign {
                    name,
                    annotation,
                    value,
                    ..
                }
                | S::SpawnBind {
                    name,
                    annotation,
                    value,
                    ..
                } => {
                    if let Some(ty) = annotation {
                        self.ty(ty, stmt.line)?;
                    }
                    self.expr(value)?;
                    self.bind(name);
                }
                S::Expr(value) | S::Spawn(value) | S::Return(Some(value)) => self.expr(value)?,
                S::If(condition, yes, no) => {
                    self.expr(condition)?;
                    self.child(yes)?;
                    self.child(no)?;
                }
                S::While(condition, body) => {
                    self.expr(condition)?;
                    self.child(body)?;
                }
                S::For(name, iterable, body) => {
                    self.expr(iterable)?;
                    let before = self.locals.clone();
                    self.bind(name);
                    let result = self.block(body);
                    self.locals = before;
                    result?;
                }
                S::Scope(body) => self.child(body)?,
                S::Match(value, arms) => {
                    self.expr(value)?;
                    for arm in arms {
                        let before = self.locals.clone();
                        if let MatchPattern::Enum { name, span, .. } = &mut arm.pattern {
                            self.resolve_variant_name(name, arm.line, *span, false)?;
                        }
                        for binding in arm.pattern.bindings_mut() {
                            if let Some(name) = &mut binding.name {
                                self.bind(name);
                            }
                            if let Some(ty) = &mut binding.ty {
                                self.ty(ty, arm.line)?;
                            }
                        }
                        let result = self.block(&mut arm.body);
                        self.locals = before;
                        result?;
                    }
                }
                S::Return(None) => {}
            }
        }
        Ok(())
    }
    fn program(&mut self, program: &mut Program, intern: bool) -> Result<(), String> {
        self.raw = intern;
        self.builtin_types = program.modules.builtin_types.clone();
        let initial_module = self.module;
        for class in &mut program.classes {
            if !intern {
                self.module = self
                    .metadata
                    .definition(&class.name)
                    .map(|definition| &definition.id.module)
                    .unwrap_or(initial_module);
            }
            for (i, (_, ty)) in class.fields.iter_mut().enumerate() {
                self.ty(ty, class.field_lines.get(i).copied().unwrap_or(class.line))?;
            }
            if let Some(def) = self
                .metadata
                .exports(self.module)
                .find(|d| intern && d.id.kind == DefKind::Class && d.id.name == class.name)
            {
                class.name = def.symbol.clone();
            }
        }
        for enumeration in &mut program.enums {
            if !intern {
                self.module = self
                    .metadata
                    .definition(&enumeration.name)
                    .map(|definition| &definition.id.module)
                    .unwrap_or(initial_module);
            }
            for variant in &mut enumeration.variants {
                for (i, (_, ty)) in variant.fields.iter_mut().enumerate() {
                    self.ty(
                        ty,
                        variant.field_lines.get(i).copied().unwrap_or(variant.line),
                    )?;
                }
            }
            if let Some(def) = self
                .metadata
                .exports(self.module)
                .find(|d| intern && d.id.kind == DefKind::Enum && d.id.name == enumeration.name)
            {
                enumeration.name = def.symbol.clone();
            }
        }
        for function in &mut program.functions {
            if !intern {
                self.module = self
                    .metadata
                    .definition(&function.name)
                    .map(|definition| &definition.id.module)
                    .unwrap_or(initial_module);
            }
            self.locals.clear();
            // Parameter names are in scope in the body, not in type syntax.
            for (_, ty) in &mut function.params {
                self.ty(ty, function.line)?;
            }
            // The parser's omitted return annotation is intrinsic unit, even
            // when a source namespace or record happens to be named unit.
            let explicit_return = self.tokens.iter().enumerate().position(|(i, token)| {
                token.line + self.offset == function.line
                    && matches!(&token.kind, K::Id(name) if name == "def" || name == "fn")
                    && matches!(self.tokens.get(i + 1).map(|t| &t.kind), Some(K::Id(name)) if name == &function.name)
            }).is_some_and(|start| {
                let mut depth = 0usize;
                for i in start..self.tokens.len() {
                    match &self.tokens[i].kind {
                        K::Sym(s) if s == "(" => depth += 1,
                        K::Sym(s) if s == ")" && depth > 0 => {
                            depth -= 1;
                            if depth == 0 {
                                return matches!(self.tokens.get(i + 1).map(|t| &t.kind), Some(K::Sym(s)) if s == "->");
                            }
                        }
                        _ => {}
                    }
                }
                false
            });
            if !intern || function.ret != Type::named("unit") || explicit_return {
                self.ty(&mut function.ret, function.line)?;
            }
            for (name, _) in &mut function.params {
                self.bind(name);
            }
            self.block(&mut function.body)?;
            if let Some(def) = self
                .metadata
                .exports(self.module)
                .find(|d| intern && d.id.kind == DefKind::Function && d.id.name == function.name)
            {
                function.name = def.symbol.clone();
            }
        }
        self.locals.clear();
        self.module = initial_module;
        self.annotation_references()?;
        program.modules.builtin_types = None;
        Ok(())
    }
}

/// Native fragments and generated High share ordinary root bindings. Qualified
/// aliases retain the generated namespace; local variables still shadow globals.
pub fn prepare_native_fragment(native: &mut Program, generated: &Program) {
    if let Some(def) = native.modules.definition("main").cloned() {
        let replacement = native
            .functions
            .iter()
            .find(|f| f.name == "main")
            .is_some_and(|f| f.attrs.iter().any(|(name, _)| name == "replace"));
        if generated.modules.definition("main").is_some() || replacement {
            remap_definition(native, "main", &symbol(&def.id));
        }
    }
    let replacements: HashSet<_> = native
        .functions
        .iter()
        .filter(|f| f.attrs.iter().any(|(name, _)| name == "replace"))
        .filter_map(|f| native.modules.definition(&f.name).map(|def| def.id.clone()))
        .collect();
    native
        .modules
        .bindings
        .retain(|binding| match &binding.target {
            BindingTarget::Definition(id) => !replacements.contains(id),
            BindingTarget::Module(_) => true,
        });
    if let Some(root) = native.modules.root.clone() {
        // Each explicit native fragment shares the generated root context.
        // Its imported dependencies keep their own source-file namespaces.
        for binding in generated.modules.root_bindings() {
            if matches!(&binding.target, BindingTarget::Definition(id)
                if binding.public || id.module == binding.module && id.name == binding.name)
            {
                // Ordinary shared definitions use the fallback map. Keeping
                // them there preserves the legacy class/builtin namespaces.
                continue;
            }
            if native.modules.binding(&root, &binding.name).is_none() {
                let mut binding = binding.clone();
                binding.module = root.clone();
                binding.public = false;
                binding.span = Span::default();
                native.modules.bindings.push(binding);
            }
        }
    }
}

pub fn rebind_native(generated: &mut Program, native: &mut Program) -> Result<(), String> {
    prepare_native_fragment(native, generated);
    let mut combined = generated.modules.clone();
    combined.merge(native.modules.clone())?;
    let flat = FlatBindings::new(&combined)?;
    let generated_root = generated.modules.root.clone();
    let native_root = native.modules.root.clone();
    let native_public: HashMap<_, _> = native
        .modules
        .root_public_bindings()
        .filter_map(|b| match &b.target {
            BindingTarget::Definition(id) => Some((b.name.clone(), id.clone())),
            _ => None,
        })
        .collect();
    let generated_public: HashMap<_, _> = generated
        .modules
        .root_public_bindings()
        .filter_map(|b| match &b.target {
            BindingTarget::Definition(id) => Some((b.name.clone(), id.clone())),
            _ => None,
        })
        .collect();
    if let Some(root) = &generated_root {
        let mut resolver = Resolver::new(&combined, &flat, root, &[], 0);
        resolver.extra = native_public.clone();
        resolver.program(generated, false)?;
        generated.modules.references.extend(resolver.references);
        for (name, id) in &native_public {
            if generated.modules.binding(root, name).is_none() {
                if let Some(def) = combined.definition_id(id) {
                    generated.modules.bindings.push(ModuleBinding {
                        module: root.clone(),
                        name: name.clone(),
                        target: BindingTarget::Definition(id.clone()),
                        public: true,
                        line: def.line,
                        span: Span::default(),
                    });
                }
            }
        }
    }
    if let Some(root) = &native_root {
        let mut resolver = Resolver::new(&combined, &flat, root, &[], 0);
        resolver.extra = generated_public;
        resolver.program(native, false)?;
        native.modules.references.extend(resolver.references);
    }
    Ok(())
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    fn entry(path: &str) -> Program {
        let module = ModuleId(path.into());
        let id = DefId {
            module: module.clone(),
            kind: DefKind::Function,
            name: "main".into(),
        };
        Program {
            modules: ModuleMetadata {
                root: Some(module.clone()),
                modules: vec![ModuleInfo {
                    id: module.clone(),
                    path: path.into(),
                }],
                definitions: vec![DefinitionInfo {
                    id: id.clone(),
                    symbol: "main".into(),
                    line: 1,
                }],
                bindings: vec![ModuleBinding {
                    module,
                    name: "main".into(),
                    target: BindingTarget::Definition(id),
                    public: true,
                    line: 1,
                    span: Span::default(),
                }],
                references: vec![],
                builtin_types: None,
            },
            functions: vec![Function {
                name: "main".into(),
                params: vec![],
                parameter_spans: vec![],
                ret: Type::named("unit"),
                asynchronous: false,
                external: false,
                body: vec![],
                attrs: vec![],
                line: 1,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn low_identity_is_portable_between_unix_drive_and_unc_paths() {
        for path in [
            "/tmp/domain/orders.nagi",
            r"C:\work\domain\orders.nagi",
            "C:/work/domain/orders.nagi",
            r"\\server\share\domain\orders.nagi",
            "//server/share/domain/orders.nagi",
            r"\\?\C:\work\domain\orders.nagi",
            r"\\?\UNC\server\share\domain\orders.nagi",
        ] {
            let program = entry(path);
            validate(&program).unwrap_or_else(|e| panic!("{path}: {e}"));
            let parsed = crate::parser::parse(&crate::emit::low(&program), false).unwrap();
            validate(&parsed).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(parsed.modules.root, program.modules.root);
        }
    }

    #[test]
    fn metadata_rejects_relative_noncanonical_and_traversing_ids() {
        for path in [
            "orders.nagi",
            "/tmp/../orders.nagi",
            "/tmp/./orders.nagi",
            "/tmp//orders.nagi",
            "/tmp/orders.nagi/",
            "C:orders.nagi",
            r"C:\work\..\orders.nagi",
            r"C:\work/thing.nagi",
            r"\\server",
            r"\\server\share\..\orders.nagi",
            "//server",
            "/tmp/\0orders.nagi",
        ] {
            assert!(
                validate(&entry(path)).is_err(),
                "accepted invalid identity {path:?}"
            );
        }
    }

    #[test]
    fn metadata_cannot_rebind_a_definition_to_a_builtin_or_unrelated_id() {
        let mut program = entry("/tmp/main.nagi");
        program.functions[0].name = "print".into();
        program.modules.definitions[0].symbol = "print".into();
        assert!(validate(&program).is_err());
        let mut program = entry("/tmp/main.nagi");
        if let BindingTarget::Definition(id) = &mut program.modules.bindings[0].target {
            id.name = "other".into();
        }
        assert!(validate(&program).is_err());
        let mut program = entry("/tmp/main.nagi");
        program.modules.root = Some(ModuleId("/tmp/other.nagi".into()));
        assert!(validate(&program).is_err());
    }

    #[test]
    fn low_metadata_rejects_unknown_topology_and_span_fields() {
        let valid = serde_json::to_value(entry("/tmp/main.nagi").modules).unwrap();
        let mut top = valid.clone();
        top["unknown"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ModuleMetadata>(top).is_err());
        let mut nested = valid;
        nested["bindings"][0]["span"]["unknown"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ModuleMetadata>(nested).is_err());
    }

    #[test]
    fn intrinsic_type_transport_is_structural_and_avoids_real_bindings() {
        let mut program = entry("/tmp/main.nagi");
        let root = program.modules.root.clone().unwrap();
        for name in ["unit", "__nagi_builtin_0"] {
            program.modules.bindings.push(ModuleBinding {
                module: root.clone(),
                name: name.into(),
                target: BindingTarget::Module(root.clone()),
                public: false,
                line: 1,
                span: Span::default(),
            });
        }
        let mut transport = prepare_low_types(&program);
        assert_eq!(
            transport.modules.builtin_types.as_deref(),
            Some("__nagi_builtin_1")
        );
        assert_eq!(transport.functions[0].ret.0, "__nagi_builtin_1.unit");
        assert_eq!(program.functions[0].ret, Type::named("unit"));
        decode_low_types(&mut transport).unwrap();
        assert_eq!(transport.functions[0].ret, Type::named("unit"));
        assert!(transport.modules.builtin_types.is_none());
    }

    #[test]
    fn intrinsic_transport_rejects_forged_namespace_collisions_and_unknown_heads() {
        let mut program = entry("/tmp/main.nagi");
        program.modules.builtin_types = Some("main".into());
        assert!(decode_low_types(&mut program).is_err());
        program.modules.builtin_types = Some("__nagi_builtin_0".into());
        program.functions[0].ret = Type::named("__nagi_builtin_0.Secret");
        assert!(decode_low_types(&mut program).is_err());
        program.functions[0].ret = Type::named("__nagi_builtin_0.unit");
        let root = program.modules.root.clone().unwrap();
        program.modules.bindings.push(ModuleBinding {
            module: root.clone(),
            name: "__nagi_builtin_0".into(),
            target: BindingTarget::Module(root),
            public: false,
            line: 1,
            span: Span::default(),
        });
        assert!(decode_low_types(&mut program).is_err());
    }

    #[test]
    fn malformed_internal_looking_names_keep_their_original_display() {
        for name in [
            "__nagi_def_😀_c_41",
            "__nagi_def_ff_c_41",
            "__nagi_def_00_c_xyz",
        ] {
            assert_eq!(display_symbol(name), name);
        }
    }
}
