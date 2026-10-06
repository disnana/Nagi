//! Structured lowering for owning values that carry borrowed views.
//!
//! Each checked version has private optional storage at its original cleanup
//! anchor, with an independent Rust lifetime. Ownership moves and borrowed
//! places use checker-decided operand roles. Ordinary Rust assignment/drop
//! handles conditional initialization, destruction, and unwinding. Structured
//! branches and loop edges transfer storage without cloning its payload.

use crate::ast::{
    block_returns, BindingId, Expr, ExprUseId, ExprUseMode, FlowExprUse, FlowOrigin, Function,
    NameResolution, Stmt, Type, ViewListBindingSnapshot, E, S,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Debug)]
pub(crate) enum Action {
    DeclareSlot {
        name: String,
        ty: Type,
    },
    /// Reset-and-drop is valid even when Rust's drop flag says the slot was
    /// moved on the current path. `None` requires no payload or allocation.
    Retire {
        slot: String,
    },
    /// A moved value is installed in a path-join slot. This leaves the source
    /// slot uninitialized, so normal Rust drop flags suppress a second drop.
    Transfer {
        from: String,
        to: String,
    },
    /// Put an ordinary checked value into private optional storage.
    Install {
        from: String,
        to: String,
    },
    /// A branch moved the current value before a join. Initialize the phi with
    /// absent storage so later transfers and retires have a definite slot.
    InitEmpty {
        slot: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Assignment {
    pub retire_slots: Vec<String>,
    pub new_slot: String,
    pub rhs_temp: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub stmt: Stmt,
    pub before: Vec<Action>,
    pub after: Vec<Action>,
    pub children: Vec<Block>,
    pub assignment: Option<Assignment>,
    pub condition_before: Vec<Action>,
    pub condition_exit: Vec<Action>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Block {
    pub entry: Vec<Action>,
    pub statements: Vec<Node>,
    pub tail: Vec<Action>,
    pub terminal: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Plan {
    pub body: Block,
    pub parameter_actions: BTreeMap<String, Vec<Action>>,
    pub storage_slots: BTreeSet<String>,
    pub expression_uses: BTreeMap<ExprUseId, ExprUseMode>,
}

#[derive(Clone)]
struct Candidate {
    ty: Type,
    name: String,
    parameter: bool,
}

#[derive(Default)]
struct CandidateFacts {
    name: Option<String>,
    ty: Option<Type>,
    saw_short_owner: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValueState {
    Active {
        slot: String,
        retained: BTreeSet<String>,
    },
    /// The slot is initialized on every path, but may contain absent storage
    /// on paths where the source binding was moved.
    MaybeMoved {
        slot: String,
        retained: BTreeSet<String>,
    },
    /// Checker moves include short-circuit consumption. These slots can still
    /// own a value on runtime paths that skipped the consuming expression.
    Moved { retained: BTreeSet<String> },
}

impl ValueState {
    fn active(slot: String) -> Self {
        Self::Active {
            slot,
            retained: BTreeSet::new(),
        }
    }

    fn retained(&self) -> &BTreeSet<String> {
        match self {
            Self::Active { retained, .. }
            | Self::MaybeMoved { retained, .. }
            | Self::Moved { retained } => retained,
        }
    }

    fn retirement_slots(&self) -> BTreeSet<String> {
        let mut slots = self.retained().clone();
        if let Self::Active { slot, .. } | Self::MaybeMoved { slot, .. } = self {
            slots.insert(slot.clone());
        }
        slots
    }

    fn mark_moved(&mut self) {
        *self = Self::Moved {
            retained: self.retirement_slots(),
        };
    }
}

struct Builder {
    candidates: HashMap<BindingId, Candidate>,
    slots: BTreeMap<BindingId, Vec<String>>,
    next_temp: usize,
}

struct LoopPlan {
    body: Block,
    exit_state: HashMap<BindingId, ValueState>,
    condition_before: Vec<Action>,
    condition_exit: Vec<Action>,
}

fn transient_origin(origin: &FlowOrigin, parameter_views: &HashSet<BindingId>) -> bool {
    origin.owner_loan && !origin.static_origin && !parameter_views.contains(&origin.binding)
}

fn has_transient_origin(origins: &[FlowOrigin], parameter_views: &HashSet<BindingId>) -> bool {
    origins
        .iter()
        .any(|origin| transient_origin(origin, parameter_views))
}

fn snapshot_named<'a>(
    snapshots: &'a [ViewListBindingSnapshot],
    name: &str,
) -> Option<&'a ViewListBindingSnapshot> {
    snapshots.iter().find(|snapshot| snapshot.name == name)
}

fn escaping_bindings(statements: &[Stmt]) -> HashSet<BindingId> {
    fn visit(
        statements: &[Stmt],
        observed: &mut HashSet<BindingId>,
        dependencies: &mut HashMap<BindingId, BTreeSet<BindingId>>,
    ) {
        for statement in statements {
            if let Some(flow) = &statement.flow {
                observed.extend(flow.return_observers.iter().copied());
                for edge in &flow.value_dependencies {
                    dependencies
                        .entry(edge.target)
                        .or_default()
                        .extend(&edge.inputs);
                }
                for mutation in &flow.content_mutations {
                    dependencies
                        .entry(mutation.binding)
                        .or_default()
                        .extend(&mutation.inputs);
                }
            }
            match &statement.kind {
                S::If(_, a, b) => {
                    visit(a, observed, dependencies);
                    visit(b, observed, dependencies);
                }
                S::Match(_, arms) => {
                    for arm in arms {
                        visit(&arm.body, observed, dependencies);
                    }
                }
                S::While(_, body) | S::For(_, _, body) | S::Scope(body) => {
                    visit(body, observed, dependencies)
                }
                _ => {}
            }
        }
    }
    let mut observed = HashSet::new();
    let mut dependencies = HashMap::new();
    visit(statements, &mut observed, &mut dependencies);
    let mut pending: Vec<_> = observed.iter().copied().collect();
    while let Some(binding) = pending.pop() {
        for input in dependencies.remove(&binding).unwrap_or_default() {
            if observed.insert(input) {
                pending.push(input);
            }
        }
    }
    observed
}

fn collect_candidates(
    statements: &[Stmt],
    parameter_views: &HashSet<BindingId>,
    facts: &mut BTreeMap<BindingId, CandidateFacts>,
) {
    for statement in statements {
        let Some(flow) = &statement.flow else {
            continue;
        };
        // Mutating operations such as append can introduce the same local
        // owner loan as an assignment. Candidate selection follows the checked
        // contents at every statement edge, not only assignment targets.
        for snapshot in flow.before.iter().chain(&flow.after) {
            if has_transient_origin(&snapshot.origins, parameter_views) {
                let entry = facts.entry(snapshot.binding).or_default();
                entry.name = Some(snapshot.name.clone());
                entry.ty = Some(snapshot.ty.clone());
                entry.saw_short_owner = true;
            }
        }
        for mutation in &flow.content_mutations {
            if has_transient_origin(&mutation.added_origins, parameter_views) {
                let entry = facts.entry(mutation.binding).or_default();
                entry.name = Some(mutation.name.clone());
                entry.ty = flow
                    .before
                    .iter()
                    .find(|snapshot| snapshot.binding == mutation.binding)
                    .map(|snapshot| snapshot.ty.clone());
                entry.saw_short_owner = true;
            }
        }
        match &statement.kind {
            S::Assign { .. } | S::SpawnBind { .. } => {
                if let Some(assignment) = flow.assignment {
                    if let Some(snapshot) = flow
                        .after
                        .iter()
                        .find(|snapshot| snapshot.binding == assignment.target)
                    {
                        let entry = facts.entry(assignment.target).or_default();
                        entry.name = Some(snapshot.name.clone());
                        entry.ty = Some(snapshot.ty.clone());
                        entry.saw_short_owner |=
                            has_transient_origin(&snapshot.origins, parameter_views);
                    }
                }
            }
            S::If(_, then_body, else_body) => {
                collect_candidates(then_body, parameter_views, facts);
                collect_candidates(else_body, parameter_views, facts);
            }
            S::Match(_, arms) => {
                for arm in arms {
                    collect_candidates(&arm.body, parameter_views, facts);
                }
            }
            S::Return(None) | S::Return(Some(_)) | S::Expr(_) | S::Spawn(_) => {}
            S::While(_, body) | S::For(_, _, body) | S::Scope(body) => {
                collect_candidates(body, parameter_views, facts)
            }
        }
    }
}

fn find_declarations(statements: &[Stmt], declarations: &mut HashSet<BindingId>) {
    for statement in statements {
        match &statement.kind {
            S::Assign { declare: true, .. } | S::SpawnBind { declare: true, .. } => {
                if let Some(target) = statement
                    .flow
                    .as_ref()
                    .and_then(|flow| flow.assignment)
                    .map(|assignment| assignment.target)
                {
                    declarations.insert(target);
                }
            }
            S::If(_, then_body, else_body) => {
                find_declarations(then_body, declarations);
                find_declarations(else_body, declarations);
            }
            S::Match(_, arms) => {
                for arm in arms {
                    for binding in arm.pattern.bindings() {
                        declarations.insert(BindingId {
                            line: arm.line,
                            token: binding.span.start,
                        });
                    }
                    find_declarations(&arm.body, declarations);
                }
            }
            S::For(_, _, body) => {
                declarations.insert(BindingId {
                    line: statement.line,
                    token: statement.binding_span.unwrap_or_default().start,
                });
                find_declarations(body, declarations);
            }
            S::Assign { .. } | S::SpawnBind { .. } | S::Return(_) | S::Expr(_) | S::Spawn(_) => {}
            S::While(_, body) | S::Scope(body) => find_declarations(body, declarations),
        }
    }
}

impl Builder {
    fn new(candidates: HashMap<BindingId, Candidate>) -> Self {
        Self {
            candidates,
            slots: BTreeMap::new(),
            next_temp: 0,
        }
    }

    fn fresh_slot(&mut self, binding: BindingId) -> String {
        let slots = self.slots.entry(binding).or_default();
        let slot = format!(
            "__nagi_view_flow_{}_{}_v{}",
            binding.line,
            binding.token,
            slots.len()
        );
        slots.push(slot.clone());
        slot
    }

    fn fresh_temp(&mut self) -> String {
        let temp = format!("__nagi_view_flow_rhs_{}", self.next_temp);
        self.next_temp += 1;
        temp
    }

    fn state_slot(state: &ValueState) -> Option<&str> {
        match state {
            ValueState::Active { slot, .. } | ValueState::MaybeMoved { slot, .. } => Some(slot),
            ValueState::Moved { .. } => None,
        }
    }

    fn current_names(
        &self,
        snapshots: &[ViewListBindingSnapshot],
        state: &HashMap<BindingId, ValueState>,
    ) -> HashMap<String, String> {
        snapshots
            .iter()
            .filter(|snapshot| self.candidates.contains_key(&snapshot.binding))
            .filter_map(|snapshot| {
                Self::state_slot(state.get(&snapshot.binding)?)
                    .map(|slot| (snapshot.name.clone(), slot.to_owned()))
            })
            .collect()
    }

    fn apply_before_facts(
        &self,
        snapshots: &[ViewListBindingSnapshot],
        state: &mut HashMap<BindingId, ValueState>,
    ) {
        for snapshot in snapshots {
            if !self.candidates.contains_key(&snapshot.binding) || !snapshot.moved {
                continue;
            }
            if let Some(value @ ValueState::Active { .. }) = state.get_mut(&snapshot.binding) {
                value.mark_moved();
            }
        }
    }

    fn apply_after_facts(
        &self,
        before: &[ViewListBindingSnapshot],
        after: &[ViewListBindingSnapshot],
        state: &mut HashMap<BindingId, ValueState>,
    ) {
        for snapshot in after {
            if !self.candidates.contains_key(&snapshot.binding) || !snapshot.moved {
                continue;
            }
            let was_moved = snapshot_named(before, &snapshot.name)
                .is_some_and(|previous| previous.binding == snapshot.binding && previous.moved);
            if !was_moved {
                if let Some(value @ ValueState::Active { .. }) = state.get_mut(&snapshot.binding) {
                    value.mark_moved();
                }
            }
        }
    }

    fn rewrite_expr(expr: &mut Expr, names: &HashMap<String, String>) {
        fn visit(expr: &mut Expr, names: &HashMap<String, String>) {
            match &mut expr.kind {
                E::Name(name) if expr.resolution == Some(NameResolution::Local) => {
                    if let Some(replacement) = names.get(name) {
                        *name = replacement.clone();
                    }
                }
                E::Binary(left, _, right) | E::Index(left, right) => {
                    visit(left, names);
                    visit(right, names);
                }
                E::Unary(_, value) | E::Field(value, _) | E::Await(value) | E::Try(value) => {
                    visit(value, names)
                }
                E::Call(_, _, args) | E::List(args) => {
                    for arg in args {
                        visit(arg, names);
                    }
                }
                E::Record(_, fields) => {
                    for (_, value) in fields {
                        visit(value, names);
                    }
                }
                E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null | E::Name(_) => {}
            }
        }
        visit(expr, names);
    }

    fn join_states(
        &mut self,
        entry: &HashMap<BindingId, ValueState>,
        visible: &[ViewListBindingSnapshot],
        predecessors: &[(usize, HashMap<BindingId, ValueState>)],
        children: &mut [Block],
    ) -> HashMap<BindingId, ValueState> {
        let mut result = HashMap::new();
        if predecessors.is_empty() {
            return result;
        }
        for snapshot in visible {
            let binding = snapshot.binding;
            if !self.candidates.contains_key(&binding) {
                continue;
            }
            let values: Vec<_> = predecessors
                .iter()
                .filter_map(|(index, state)| {
                    state
                        .get(&binding)
                        .or_else(|| entry.get(&binding))
                        .cloned()
                        .map(|value| (*index, value))
                })
                .collect();
            let Some((_, first)) = values.first() else {
                continue;
            };
            if values.iter().all(|(_, value)| value == first) {
                result.insert(binding, first.clone());
                continue;
            }
            let retained = values
                .iter()
                .flat_map(|(_, value)| value.retained().iter().cloned())
                .collect();
            let active = values
                .iter()
                .all(|(_, value)| matches!(value, ValueState::Active { .. }));
            let slot = Self::state_slot(first);
            if values
                .iter()
                .all(|(_, value)| Self::state_slot(value) == slot)
            {
                let joined = match slot {
                    Some(slot) if active => ValueState::Active {
                        slot: slot.to_owned(),
                        retained,
                    },
                    Some(slot) => ValueState::MaybeMoved {
                        slot: slot.to_owned(),
                        retained,
                    },
                    None => ValueState::Moved { retained },
                };
                result.insert(binding, joined);
                continue;
            }
            let phi = self.fresh_slot(binding);
            for (index, value) in values {
                children[index].tail.push(match value {
                    ValueState::Active { slot: from, .. }
                    | ValueState::MaybeMoved { slot: from, .. } => Action::Transfer {
                        from,
                        to: phi.clone(),
                    },
                    ValueState::Moved { .. } => Action::InitEmpty { slot: phi.clone() },
                });
            }
            result.insert(
                binding,
                if active {
                    ValueState::Active {
                        slot: phi,
                        retained,
                    }
                } else {
                    ValueState::MaybeMoved {
                        slot: phi,
                        retained,
                    }
                },
            );
        }
        result
    }

    fn block(
        &mut self,
        statements: &[Stmt],
        mut state: HashMap<BindingId, ValueState>,
    ) -> Result<(Block, HashMap<BindingId, ValueState>), ()> {
        let mut planned = Block {
            entry: Vec::new(),
            statements: Vec::with_capacity(statements.len()),
            tail: Vec::new(),
            terminal: block_returns(statements),
        };

        for source in statements {
            let facts = source.flow.as_ref().ok_or(())?;
            self.apply_before_facts(&facts.before, &mut state);
            let mut node = Node {
                stmt: source.clone(),
                before: Vec::new(),
                after: Vec::new(),
                children: Vec::new(),
                assignment: None,
                condition_before: Vec::new(),
                condition_exit: Vec::new(),
            };

            // Mutating contents changes the lifetime carried by the checked type
            // without replacing its allocation. Move its storage to an independent
            // slot before evaluating the expression, at the same cleanup
            // anchor. This also preserves changes overwritten by an assignment
            // later in the same RHS. Branch statements own separate events.
            let mutated: BTreeSet<_> = facts
                .content_mutations
                .iter()
                .filter(|mutation| !mutation.added_origins.is_empty())
                .map(|mutation| mutation.binding)
                .filter(|binding| self.candidates.contains_key(binding))
                .filter(|_| !matches!(source.kind, S::While(..)))
                .collect();
            for binding in mutated {
                let old = state.get(&binding).cloned().ok_or(())?;
                let from = Self::state_slot(&old).ok_or(())?.to_owned();
                let slot = self.fresh_slot(binding);
                node.before.push(Action::Transfer {
                    from,
                    to: slot.clone(),
                });
                let retained = old.retained().clone();
                state.insert(
                    binding,
                    match old {
                        ValueState::Active { .. } => ValueState::Active { slot, retained },
                        ValueState::MaybeMoved { .. } => ValueState::MaybeMoved { slot, retained },
                        ValueState::Moved { .. } => return Err(()),
                    },
                );
            }
            let names = self.current_names(&facts.before, &state);

            match &mut node.stmt.kind {
                S::Assign {
                    name,
                    value,
                    declare,
                    ..
                }
                | S::SpawnBind {
                    name,
                    value,
                    declare,
                    ..
                } => {
                    Self::rewrite_expr(value, &names);
                    let target = facts.assignment.ok_or(())?.target;
                    if self.candidates.contains_key(&target) {
                        let old = state.get(&target).cloned();
                        // A checker move fact is conservative: a short-circuit
                        // RHS may leave the previous slot initialized at runtime.
                        // Joins retain only versions that can still own a value.
                        // Each source reassignment resets that set, so linear
                        // assignment chains do not repeatedly retire old slots.
                        let retire_slots = old
                            .as_ref()
                            .map(|state| state.retirement_slots().into_iter().collect())
                            .unwrap_or_default();
                        let new_slot = self.fresh_slot(target);
                        *name = new_slot.clone();
                        if old.is_some() {
                            let rhs_temp = self.fresh_temp();
                            node.assignment = Some(Assignment {
                                retire_slots,
                                new_slot: new_slot.clone(),
                                rhs_temp,
                            });
                        } else if !*declare {
                            return Err(());
                        }
                        state.insert(target, ValueState::active(new_slot));
                    }
                    // Even a versioned target's RHS can consume a different
                    // candidate. This must reach branch joins without relying
                    // on a later statement to observe the move in before facts.
                    self.apply_after_facts(&facts.before, &facts.after, &mut state);
                }
                S::Return(Some(expr)) | S::Expr(expr) | S::Spawn(expr) => {
                    Self::rewrite_expr(expr, &names);
                    self.apply_after_facts(&facts.before, &facts.after, &mut state);
                }
                S::Return(None) => {}
                S::If(condition, then_body, else_body) => {
                    Self::rewrite_expr(condition, &names);
                    let branch_entry = self.condition_state(
                        &facts.before,
                        facts.branch_entry.as_deref().ok_or(())?,
                        &state,
                    );
                    let (then_plan, then_state) = self.block(then_body, branch_entry.clone())?;
                    let (else_plan, else_state) = self.block(else_body, branch_entry.clone())?;
                    node.children = vec![then_plan, else_plan];
                    let predecessors = [(0, then_state), (1, else_state)]
                        .into_iter()
                        .filter(|(index, _)| !node.children[*index].terminal)
                        .collect::<Vec<_>>();
                    state = self.join_states(
                        &branch_entry,
                        &facts.after,
                        &predecessors,
                        &mut node.children,
                    );
                }
                S::Match(value, arms) => {
                    Self::rewrite_expr(value, &names);
                    let branch_entry = self.condition_state(
                        &facts.before,
                        facts.branch_entry.as_deref().ok_or(())?,
                        &state,
                    );
                    let mut predecessors = Vec::new();
                    for (index, arm) in arms.iter().enumerate() {
                        let mut arm_entry = branch_entry.clone();
                        for binding in arm.pattern.bindings() {
                            let id = BindingId {
                                line: arm.line,
                                token: binding.span.start,
                            };
                            if self.candidates.contains_key(&id) {
                                let slot = self.fresh_slot(id);
                                arm_entry.insert(id, ValueState::active(slot));
                            }
                        }
                        let (child, child_state) = self.block(&arm.body, arm_entry)?;
                        if !child.terminal {
                            predecessors.push((index, child_state));
                        }
                        node.children.push(child);
                    }
                    state = self.join_states(
                        &branch_entry,
                        &facts.after,
                        &predecessors,
                        &mut node.children,
                    );
                }
                S::While(condition, body) => {
                    let region =
                        self.loop_block(source, condition, body, &mut node.before, state)?;
                    node.children.push(region.body);
                    node.condition_before = region.condition_before;
                    node.condition_exit = region.condition_exit;
                    state = region.exit_state;
                }
                S::For(_, iterator, body) => {
                    self.apply_after_facts(
                        &facts.before,
                        &facts.loop_entry.as_ref().ok_or(())?.header,
                        &mut state,
                    );
                    let mut unused = iterator.clone();
                    let region =
                        self.loop_block(source, &mut unused, body, &mut node.before, state)?;
                    // Header moves precede evaluation of the Rust iterator.
                    // Borrow its header place; consumed inputs instead keep
                    // their preheader place until the iterator consumes it.
                    let mut iterator_names = names.clone();
                    iterator_names.extend(self.current_names(&facts.before, &region.exit_state));
                    Self::rewrite_expr(iterator, &iterator_names);
                    node.children.push(region.body);
                    state = region.exit_state;
                }
                S::Scope(body) => {
                    let entry = state.clone();
                    let (child, child_state) = self.block(body, entry.clone())?;
                    node.children.push(child);
                    state = self.join_states(
                        &entry,
                        &facts.after,
                        &[(0, child_state)],
                        &mut node.children,
                    );
                }
            }

            planned.statements.push(node);
        }

        Ok((planned, state))
    }

    fn loop_block(
        &mut self,
        source: &Stmt,
        condition: &mut Expr,
        body: &[Stmt],
        before: &mut Vec<Action>,
        state: HashMap<BindingId, ValueState>,
    ) -> Result<LoopPlan, ()> {
        let facts = source.flow.as_ref().ok_or(())?;
        let loop_facts = facts.loop_entry.as_ref().ok_or(())?;
        let outer: BTreeSet<_> = loop_facts
            .header
            .iter()
            .map(|snapshot| snapshot.binding)
            .filter(|binding| state.contains_key(binding))
            .collect();
        let mut header_state = state.clone();
        let mut headers = BTreeMap::new();
        let mut slot_start = BTreeMap::new();
        for binding in &outer {
            slot_start.insert(*binding, self.slots.get(binding).map_or(0, Vec::len));
            let old = state.get(binding).ok_or(())?;
            let header = self.fresh_slot(*binding);
            before.push(match Self::state_slot(old) {
                Some(from) => Action::Transfer {
                    from: from.to_owned(),
                    to: header.clone(),
                },
                None => Action::InitEmpty {
                    slot: header.clone(),
                },
            });
            headers.insert(*binding, header.clone());
            header_state.insert(
                *binding,
                match old {
                    ValueState::Active { retained, .. } => ValueState::Active {
                        slot: header,
                        retained: retained.clone(),
                    },
                    ValueState::MaybeMoved { retained, .. } => ValueState::MaybeMoved {
                        slot: header,
                        retained: retained.clone(),
                    },
                    ValueState::Moved { retained } => ValueState::Moved {
                        retained: retained.clone(),
                    },
                },
            );
        }
        let mut condition_before = Vec::new();
        if matches!(source.kind, S::While(..)) {
            let mutations: BTreeSet<_> = facts
                .content_mutations
                .iter()
                .filter(|mutation| !mutation.added_origins.is_empty())
                .map(|mutation| mutation.binding)
                .filter(|binding| self.candidates.contains_key(binding))
                .collect();
            for binding in mutations {
                let old = header_state.get(&binding).ok_or(())?.clone();
                let from = Self::state_slot(&old).ok_or(())?.to_owned();
                let slot = self.fresh_slot(binding);
                condition_before.push(Action::Transfer {
                    from,
                    to: slot.clone(),
                });
                header_state.insert(
                    binding,
                    ValueState::Active {
                        slot,
                        retained: old.retained().clone(),
                    },
                );
            }
            Self::rewrite_expr(
                condition,
                &self.current_names(&loop_facts.header, &header_state),
            );
        }
        let mut entry = self.condition_state(&loop_facts.header, &loop_facts.body, &header_state);
        let mut binding_actions = Vec::new();
        if let S::For(name, _, _) = &source.kind {
            let binding = BindingId {
                line: source.line,
                token: source.binding_span.unwrap_or_default().start,
            };
            if self.candidates.contains_key(&binding) {
                let slot = self.fresh_slot(binding);
                entry.insert(binding, ValueState::active(slot.clone()));
                binding_actions.push(Action::DeclareSlot {
                    name: slot.clone(),
                    ty: self.candidates[&binding].ty.clone(),
                });
                binding_actions.push(Action::Install {
                    from: name.clone(),
                    to: slot.clone(),
                });
            }
        }
        let (mut child, backedge) = self.block(body, entry.clone())?;
        child.entry.extend(binding_actions);
        for binding in &outer {
            if child.terminal {
                continue;
            }
            let value = backedge
                .get(binding)
                .or_else(|| entry.get(binding))
                .ok_or(())?;
            let header = &headers[binding];
            if Self::state_slot(value) != Some(header.as_str()) {
                child.tail.push(match Self::state_slot(value) {
                    Some(from) => Action::Transfer {
                        from: from.to_owned(),
                        to: header.clone(),
                    },
                    None => Action::InitEmpty {
                        slot: header.clone(),
                    },
                });
            }
        }
        let mut exit_actions = Vec::new();
        let mut exit_state = if matches!(source.kind, S::While(..)) {
            entry
        } else {
            header_state
        };
        if matches!(source.kind, S::While(..)) {
            for binding in &outer {
                let value = exit_state.get(binding).ok_or(())?.clone();
                if let Some(from) = Self::state_slot(&value) {
                    let slot = self.fresh_slot(*binding);
                    exit_actions.push(Action::Transfer {
                        from: from.to_owned(),
                        to: slot.clone(),
                    });
                    exit_state.insert(
                        *binding,
                        ValueState::Active {
                            slot,
                            retained: value.retained().clone(),
                        },
                    );
                }
            }
        }
        // A source statement is reused on every iteration. A conservatively
        // moved value can remain in any earlier iteration's storage when a
        // short-circuit consumer was skipped. Reassignment resets those places
        // using Rust's ordinary drop flags, without a runtime loop-state model.
        let reused: BTreeMap<_, _> = outer
            .iter()
            .map(|binding| {
                (
                    *binding,
                    self.slots[binding][slot_start[binding]..].to_vec(),
                )
            })
            .collect();
        fn retire_reused(block: &mut Block, reused: &BTreeMap<BindingId, Vec<String>>) {
            for node in &mut block.statements {
                if let Some(assignment) = &mut node.assignment {
                    if let Some(slots) = node
                        .stmt
                        .flow
                        .as_ref()
                        .and_then(|flow| flow.assignment)
                        .and_then(|fact| reused.get(&fact.target))
                    {
                        assignment.retire_slots.extend(
                            slots
                                .iter()
                                .filter(|slot| **slot != assignment.new_slot)
                                .cloned(),
                        );
                        assignment.retire_slots.sort();
                        assignment.retire_slots.dedup();
                    }
                }
                for child in &mut node.children {
                    retire_reused(child, reused);
                }
            }
        }
        retire_reused(&mut child, &reused);
        for (binding, slots) in reused {
            if let Some(value) = exit_state.get_mut(&binding) {
                let current = Self::state_slot(value).map(str::to_owned);
                match value {
                    ValueState::Active { retained, .. }
                    | ValueState::MaybeMoved { retained, .. }
                    | ValueState::Moved { retained } => {
                        retained.extend(
                            slots
                                .into_iter()
                                .filter(|slot| Some(slot) != current.as_ref()),
                        );
                    }
                }
            }
        }
        self.apply_after_facts(&loop_facts.body, &facts.after, &mut exit_state);
        Ok(LoopPlan {
            body: child,
            exit_state,
            condition_before,
            condition_exit: exit_actions,
        })
    }

    fn condition_state(
        &self,
        before: &[ViewListBindingSnapshot],
        branch_entry: &[ViewListBindingSnapshot],
        state: &HashMap<BindingId, ValueState>,
    ) -> HashMap<BindingId, ValueState> {
        let mut result = state.clone();
        self.apply_after_facts(before, branch_entry, &mut result);
        result
    }
}

fn parameter_bindings(function: &Function) -> HashMap<BindingId, String> {
    function
        .params
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            (
                BindingId {
                    line: function.line,
                    token: function.parameter_spans[index].start,
                },
                name.clone(),
            )
        })
        .collect()
}

fn parameter_view_ids(function: &Function) -> HashSet<BindingId> {
    function
        .params
        .iter()
        .enumerate()
        .filter(|(_, (_, ty))| ty.contains_view())
        .map(|(index, _)| BindingId {
            line: function.line,
            token: function.parameter_spans[index].start,
        })
        .collect()
}

fn local_declarations(block: &mut Block, builder: &Builder) {
    for node in &mut block.statements {
        if let S::Assign { declare: true, .. } = &node.stmt.kind {
            if let Some(binding) = node
                .stmt
                .flow
                .as_ref()
                .and_then(|flow| flow.assignment)
                .map(|assignment| assignment.target)
            {
                if builder.candidates.contains_key(&binding)
                    && !builder.candidates[&binding].parameter
                {
                    if let Some(slots) = builder.slots.get(&binding) {
                        node.after.extend(slots.iter().skip(1).cloned().map(|name| {
                            Action::DeclareSlot {
                                name,
                                ty: builder.candidates[&binding].ty.clone(),
                            }
                        }));
                    }
                }
            }
        }
        if let S::Match(_, arms) = &node.stmt.kind {
            for (arm, child) in arms.iter().zip(&mut node.children) {
                let bindings = arm.pattern.bindings();
                let needs_anchors = bindings.iter().any(|binding| {
                    builder.candidates.contains_key(&BindingId {
                        line: arm.line,
                        token: binding.span.start,
                    })
                });
                if needs_anchors {
                    for binding in bindings {
                        let Some(name) = &binding.name else {
                            continue;
                        };
                        let ty = binding.ty.as_ref().expect("checked pattern type");
                        let id = BindingId {
                            line: arm.line,
                            token: binding.span.start,
                        };
                        if let Some(slots) = builder.slots.get(&id) {
                            child.entry.push(Action::DeclareSlot {
                                name: slots[0].clone(),
                                ty: ty.clone(),
                            });
                            child.entry.push(Action::Install {
                                from: name.clone(),
                                to: slots[0].clone(),
                            });
                            child
                                .entry
                                .extend(slots.iter().skip(1).cloned().map(|name| {
                                    Action::DeclareSlot {
                                        name,
                                        ty: ty.clone(),
                                    }
                                }));
                        }
                    }
                }
            }
        }
        if let S::For(..) = &node.stmt.kind {
            let binding = BindingId {
                line: node.stmt.line,
                token: node.stmt.binding_span.unwrap_or_default().start,
            };
            if let Some(slots) = builder.slots.get(&binding) {
                node.children[0]
                    .entry
                    .extend(
                        slots
                            .iter()
                            .skip(1)
                            .cloned()
                            .map(|name| Action::DeclareSlot {
                                name,
                                ty: builder.candidates[&binding].ty.clone(),
                            }),
                    );
            }
        }
        for child in &mut node.children {
            local_declarations(child, builder);
        }
    }
}

pub(crate) fn plan(function: &Function) -> Result<Option<Plan>, String> {
    if !function.ret.contains_view() {
        return Ok(None);
    }
    if !crate::ast::supports_view_flow(&function.body) {
        return Ok(None);
    }

    let parameter_ids = parameter_bindings(function);
    let parameter_views = parameter_view_ids(function);
    let mut facts = BTreeMap::new();
    collect_candidates(&function.body, &parameter_views, &mut facts);
    let mut declarations = HashSet::new();
    find_declarations(&function.body, &mut declarations);

    let escaping = escaping_bindings(&function.body);
    let candidates: HashMap<_, _> = facts
        .into_iter()
        .filter_map(|(binding, fact)| {
            if !fact.saw_short_owner || !escaping.contains(&binding) {
                return None;
            }
            let parameter = parameter_ids.contains_key(&binding);
            if !parameter && !declarations.contains(&binding) {
                return None;
            }
            Some((
                binding,
                Candidate {
                    ty: fact.ty?,
                    name: fact.name?,
                    parameter,
                },
            ))
        })
        .collect();
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut builder = Builder::new(candidates);
    let mut parameter_actions = BTreeMap::new();
    let mut params: Vec<_> = builder
        .candidates
        .iter()
        .filter_map(|(binding, candidate)| {
            candidate
                .parameter
                .then_some((*binding, candidate.name.clone()))
        })
        .collect();
    params.sort_by_key(|(binding, _)| *binding);
    for (binding, name) in params {
        let slot = builder.fresh_slot(binding);
        let actions = parameter_actions
            .entry(name.clone())
            .or_insert_with(Vec::new);
        actions.push(Action::DeclareSlot {
            name: slot.clone(),
            ty: builder.candidates[&binding].ty.clone(),
        });
        actions.push(Action::Install {
            from: name,
            to: slot.clone(),
        });
    }

    let mut state = HashMap::new();
    for binding in builder.candidates.keys().copied() {
        if builder.candidates[&binding].parameter {
            let slot = builder.slots[&binding][0].clone();
            state.insert(binding, ValueState::active(slot));
        }
    }

    let internal_error = || {
        format!(
            "line {}: internal owning-view lowering error in {}: incomplete checked flow facts",
            function.line, function.name
        )
    };
    let (mut body, _) = builder
        .block(&function.body, state)
        .map_err(|()| internal_error())?;
    local_declarations(&mut body, &builder);
    for (binding, slots) in &builder.slots {
        if !builder.candidates[binding].parameter {
            continue;
        }
        let name = &builder.candidates[binding].name;
        let actions = parameter_actions.get_mut(name).ok_or_else(internal_error)?;
        actions.extend(
            slots
                .iter()
                .skip(1)
                .cloned()
                .map(|name| Action::DeclareSlot {
                    name,
                    ty: builder.candidates[binding].ty.clone(),
                }),
        );
    }

    let storage_bindings: BTreeMap<_, _> = builder
        .slots
        .iter()
        .flat_map(|(binding, slots)| slots.iter().map(|slot| (slot.clone(), *binding)))
        .collect();
    let storage_slots = storage_bindings.keys().cloned().collect();
    let mut expression_uses = BTreeMap::new();
    fn uses(statements: &[Stmt], result: &mut BTreeMap<ExprUseId, FlowExprUse>) {
        for stmt in statements {
            if let Some(flow) = &stmt.flow {
                result.extend(
                    flow.expression_uses
                        .iter()
                        .map(|use_| (use_.expression, use_.clone())),
                );
            }
            match &stmt.kind {
                S::If(_, a, b) => {
                    uses(a, result);
                    uses(b, result);
                }
                S::Match(_, arms) => {
                    for arm in arms {
                        uses(&arm.body, result);
                    }
                }
                S::While(_, body) | S::For(_, _, body) | S::Scope(body) => uses(body, result),
                _ => {}
            }
        }
    }
    uses(&function.body, &mut expression_uses);
    fn validate_expr(
        expr: &Expr,
        storage: &BTreeMap<String, BindingId>,
        uses: &BTreeMap<ExprUseId, FlowExprUse>,
    ) -> Result<(), ()> {
        match &expr.kind {
            E::Name(name)
                if storage.get(name).is_some_and(|binding| {
                    uses.get(&ExprUseId::of(expr))
                        .is_none_or(|use_| use_.binding != *binding)
                }) =>
            {
                return Err(())
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                validate_expr(a, storage, uses)?;
                validate_expr(b, storage, uses)?;
            }
            E::Unary(_, value) | E::Field(value, _) | E::Await(value) | E::Try(value) => {
                validate_expr(value, storage, uses)?
            }
            E::Call(_, _, values) | E::List(values) => {
                for value in values {
                    validate_expr(value, storage, uses)?;
                }
            }
            E::Record(_, fields) => {
                for (_, value) in fields {
                    validate_expr(value, storage, uses)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn validate(
        block: &Block,
        storage: &BTreeMap<String, BindingId>,
        uses: &BTreeMap<ExprUseId, FlowExprUse>,
    ) -> Result<(), ()> {
        for node in &block.statements {
            match &node.stmt.kind {
                S::Assign { value, .. }
                | S::SpawnBind { value, .. }
                | S::Expr(value)
                | S::Spawn(value)
                | S::Return(Some(value))
                | S::If(value, ..)
                | S::Match(value, ..)
                | S::While(value, ..)
                | S::For(_, value, _) => validate_expr(value, storage, uses)?,
                _ => {}
            }
            for child in &node.children {
                validate(child, storage, uses)?;
            }
        }
        Ok(())
    }
    validate(&body, &storage_bindings, &expression_uses).map_err(|()| internal_error())?;
    let expression_uses = expression_uses
        .into_iter()
        .map(|(id, use_)| (id, use_.mode))
        .collect();
    Ok(Some(Plan {
        body,
        parameter_actions,
        storage_slots,
        expression_uses,
    }))
}
