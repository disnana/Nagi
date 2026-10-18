//! Finite compiler-registry projections, never document or checker facts.
use super::*;
use std::sync::OnceLock;

#[derive(Default)]
struct Identities {
    resources: Vec<(String, Resource)>,
    operations: Vec<(String, Operation)>,
    names: Vec<(&'static str, &'static str, Resource)>,
    definitions: Vec<DefId>,
}

static IDENTITIES: OnceLock<Identities> = OnceLock::new();
static DECLARATIONS: OnceLock<Vec<Declaration>> = OnceLock::new();

fn definition_key(id: &DefId) -> (&str, &str, bool) {
    (&id.module.0, &id.name, id.kind == DefKind::Function)
}

fn retain_first_symbols<T>(entries: &mut Vec<(String, T)>) {
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries.dedup_by(|a, b| a.0 == b.0);
}

fn retain_first_names(entries: &mut Vec<(&'static str, &'static str, Resource)>) {
    entries.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    entries.dedup_by(|a, b| (a.0, a.1) == (b.0, b.1));
}

#[cfg(test)]
pub(super) fn first_match_fixture() {
    let mut entries = vec![("same".into(), 1), ("a".into(), 2), ("same".into(), 3)];
    retain_first_symbols(&mut entries);
    assert_eq!(entries, [("a".into(), 2), ("same".into(), 1)]);
    let mut names = vec![
        ("m", "x", Resource::Request),
        ("a", "x", Resource::Status),
        ("m", "x", Resource::Response),
    ];
    retain_first_names(&mut names);
    assert_eq!(
        names,
        [("a", "x", Resource::Status), ("m", "x", Resource::Request)]
    );
}

fn identities() -> &'static Identities {
    IDENTITIES.get_or_init(|| {
        let mut index = Identities::default();
        for &resource in RESOURCES {
            let id = resource_id(resource);
            let info = resource_info(resource);
            index
                .resources
                .push((crate::modules::symbol(&id), resource));
            index
                .names
                .push((module_info(info.module).id, info.name, resource));
            index.definitions.push(id);
        }
        for &operation in OPERATIONS {
            let id = function_id(operation);
            index
                .operations
                .push((crate::modules::symbol(&id), operation));
            index.definitions.push(id);
        }
        // Stable sort retains the old registry's first match for duplicate keys.
        retain_first_symbols(&mut index.resources);
        retain_first_symbols(&mut index.operations);
        retain_first_names(&mut index.names);
        index
            .definitions
            .sort_by(|a, b| definition_key(a).cmp(&definition_key(b)));
        index.definitions.dedup();
        index
    })
}

pub(super) fn resource_named(module: &ModuleId, name: &str) -> Option<Resource> {
    #[cfg(test)]
    if legacy_enabled() {
        return resource_named_uncached(module, name);
    }
    let index = &identities().names;
    index
        .binary_search_by(|item| (item.0, item.1).cmp(&(module.0.as_str(), name)))
        .ok()
        .map(|position| index[position].2)
}

pub(super) fn resource(symbol: &str) -> Option<Resource> {
    #[cfg(test)]
    if legacy_enabled() {
        return resource_uncached(symbol);
    }
    let index = &identities().resources;
    index
        .binary_search_by(|item| item.0.as_str().cmp(symbol))
        .ok()
        .map(|position| index[position].1)
}

pub(super) fn operation(symbol: &str) -> Option<Operation> {
    #[cfg(test)]
    if legacy_enabled() {
        return operation_uncached(symbol);
    }
    let index = &identities().operations;
    index
        .binary_search_by(|item| item.0.as_str().cmp(symbol))
        .ok()
        .map(|position| index[position].1)
}

pub(super) fn definition(id: &DefId) -> bool {
    #[cfg(test)]
    if legacy_enabled() {
        return definition_uncached(id);
    }
    is_registered_module(&id.module)
        && matches!(id.kind, DefKind::Resource | DefKind::Function)
        && identities()
            .definitions
            .binary_search_by(|item| definition_key(item).cmp(&definition_key(id)))
            .is_ok()
}

struct Declaration {
    module: &'static str,
    source: String,
    definitions: Vec<DefinitionInfo>,
    members: Vec<(Resource, &'static str, Option<usize>)>,
}

fn declarations() -> &'static Vec<Declaration> {
    DECLARATIONS.get_or_init(|| {
        MODULES
            .iter()
            .map(|&module| {
                let module = module_info(module).id;
                let id = ModuleId(module.into());
                // These builders never call the public declaration cache entries.
                // Rendering field types can consult the independent identity index.
                let source = module_source_uncached(&id);
                let definitions = definitions_in_source(&id, &source);
                let mut members = Vec::new();
                for &resource in RESOURCES {
                    if module_info(resource_info(resource).module).id != module {
                        continue;
                    }
                    for name in fields(resource)
                        .into_iter()
                        .map(|(name, _)| name)
                        .chain(constants(resource).iter().map(|constant| constant.name))
                    {
                        members.push((
                            resource,
                            name,
                            member_line_in_source(resource, name, &source),
                        ));
                    }
                }
                Declaration {
                    module,
                    source,
                    definitions,
                    members,
                }
            })
            .collect()
    })
}

fn declaration(module: &str) -> Option<&'static Declaration> {
    // Preserve MODULES order and its first-match rule; ten compiled modules.
    declarations().iter().find(|item| item.module == module)
}

pub(super) fn module_source(module: &ModuleId) -> String {
    #[cfg(test)]
    if legacy_enabled() {
        return module_source_uncached(module);
    }
    if !is_registered_module(module) {
        return String::new();
    }
    declaration(&module.0)
        .map(|item| item.source.clone())
        .unwrap_or_default()
}

pub(super) fn definitions(module: &ModuleId) -> Vec<DefinitionInfo> {
    #[cfg(test)]
    if legacy_enabled() {
        return definitions_uncached(module);
    }
    if !is_registered_module(module) {
        return vec![];
    }
    declaration(&module.0)
        .map(|item| item.definitions.clone())
        .unwrap_or_default()
}

pub(super) fn member_line(resource: Resource, name: &str) -> Option<usize> {
    #[cfg(test)]
    if legacy_enabled() {
        return member_line_uncached(resource, name);
    }
    let module = module_info(resource_info(resource).module).id;
    let item = declaration(module)?;
    if let Some((_, _, line)) = item
        .members
        .iter()
        .find(|(r, field, _)| *r == resource && *field == name)
    {
        return *line;
    }
    // Unknown request names are never inserted or retained. Preserve the old
    // search grammar even for unusual names, using only the static declaration.
    member_line_in_source(resource, name, &item.source)
}

#[cfg(test)]
thread_local! {
    static LEGACY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) fn legacy_enabled() -> bool {
    LEGACY.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn with_legacy<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            LEGACY.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(LEGACY.with(|flag| flag.replace(true)));
    body()
}

#[cfg(test)]
pub(super) fn cold() -> bool {
    IDENTITIES.get().is_none() && DECLARATIONS.get().is_none()
}

#[cfg(test)]
pub(super) fn initialized() -> bool {
    IDENTITIES.get().is_some() && DECLARATIONS.get().is_some()
}

#[cfg(test)]
pub(super) fn retained_capacity() -> (usize, usize, usize) {
    use std::mem::size_of;
    fn id_bytes(id: &DefId) -> usize {
        id.module.0.capacity() + id.name.capacity()
    }
    let identity = identities();
    let mut bytes = size_of::<OnceLock<Identities>>()
        + size_of::<OnceLock<Vec<Declaration>>>()
        + identity.resources.capacity() * size_of::<(String, Resource)>()
        + identity.operations.capacity() * size_of::<(String, Operation)>()
        + identity.names.capacity() * size_of::<(&str, &str, Resource)>()
        + identity.definitions.capacity() * size_of::<DefId>();
    bytes += identity
        .resources
        .iter()
        .map(|item| item.0.capacity())
        .sum::<usize>();
    bytes += identity
        .operations
        .iter()
        .map(|item| item.0.capacity())
        .sum::<usize>();
    bytes += identity.definitions.iter().map(id_bytes).sum::<usize>();
    let declarations = declarations();
    bytes += declarations.capacity() * size_of::<Declaration>();
    for item in declarations {
        bytes += item.source.capacity()
            + item.definitions.capacity() * size_of::<DefinitionInfo>()
            + item.members.capacity() * size_of::<(Resource, &str, Option<usize>)>();
        bytes += item
            .definitions
            .iter()
            .map(|definition| id_bytes(&definition.id) + definition.symbol.capacity())
            .sum::<usize>();
    }
    (
        identity.resources.len() + identity.operations.len(),
        declarations.len(),
        bytes,
    )
}
