//! Flow-sensitive lowering for the narrow owning-view return case.
//!
//! A `Vec<&str>` assignment must drop its previous allocation at the
//! assignment point, while Rust needs separate local lifetimes for a
//! short-lived borrowed version and a later restored version. This planner
//! gives each checked assignment a slot at the original binding's lexical
//! position and joins continuing branches with move-only phi transfers.

use crate::ast::{
    block_returns, BindingId, Expr, FlowOrigin, Function, NameResolution, Stmt, Type,
    ViewListBindingSnapshot, E, S,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Debug)]
pub(crate) enum Action {
    DeclareSlot {
        name: String,
        ty: Type,
    },
    /// Reset-and-drop is valid even when Rust's drop flag says the slot was
    /// moved on the current path. `Vec::new` does not allocate a buffer.
    Retire {
        slot: String,
    },
    /// A moved value is installed in a path-join slot. This leaves the source
    /// slot uninitialized, so normal Rust drop flags suppress a second drop.
    Transfer {
        from: String,
        to: String,
    },
    /// A branch moved the current value before a join. Initialize the phi with
    /// an empty Vec so later transfers and retires have a definite slot.
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
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Block {
    pub statements: Vec<Node>,
    pub tail: Vec<Action>,
    pub terminal: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Plan {
    pub body: Block,
    pub parameter_actions: BTreeMap<String, Vec<Action>>,
}

#[derive(Clone)]
struct Candidate {
    name: String,
    parameter: bool,
}

#[derive(Default)]
struct CandidateFacts {
    name: Option<String>,
    saw_short_owner: bool,
    saw_safe_direct_return: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValueState {
    Active {
        slot: String,
        retained: BTreeSet<String>,
    },
    /// The slot is initialized on every path, but may contain the empty Vec
    /// on paths where the source binding was moved.
    MaybeMoved {
        slot: String,
        retained: BTreeSet<String>,
    },
    /// Checker moves include short-circuit consumption. These slots can still
    /// own a buffer on runtime paths that skipped the consuming expression.
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

fn view_list_type() -> Type {
    Type::generic(
        "List",
        vec![Type::generic("view", vec![Type::named("str")])],
    )
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

fn collect_candidates(
    statements: &[Stmt],
    function_ret: &Type,
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
        for snapshot in &flow.after {
            if has_transient_origin(&snapshot.origins, parameter_views) {
                let entry = facts.entry(snapshot.binding).or_default();
                entry.name = Some(snapshot.name.clone());
                entry.saw_short_owner = true;
            }
        }
        for mutation in &flow.content_mutations {
            if has_transient_origin(&mutation.added_origins, parameter_views) {
                let entry = facts.entry(mutation.binding).or_default();
                entry.name = Some(mutation.name.clone());
                entry.saw_short_owner = true;
            }
        }
        match &statement.kind {
            S::Assign { .. } => {
                if let Some(assignment) = flow.assignment {
                    if let Some(snapshot) = flow
                        .after
                        .iter()
                        .find(|snapshot| snapshot.binding == assignment.target)
                    {
                        let entry = facts.entry(assignment.target).or_default();
                        entry.name = Some(snapshot.name.clone());
                        entry.saw_short_owner |=
                            has_transient_origin(&snapshot.origins, parameter_views);
                    }
                }
            }
            S::Return(Some(expr)) if function_ret.is_view_string_list() => {
                if let (E::Name(name), Some(NameResolution::Local)) = (&expr.kind, expr.resolution)
                {
                    if let Some(snapshot) = snapshot_named(&flow.before, name) {
                        let entry = facts.entry(snapshot.binding).or_default();
                        entry.name = Some(snapshot.name.clone());
                        if !snapshot.moved
                            && !has_transient_origin(&snapshot.origins, parameter_views)
                        {
                            entry.saw_safe_direct_return = true;
                        }
                    }
                }
            }
            S::If(_, then_body, else_body) => {
                collect_candidates(then_body, function_ret, parameter_views, facts);
                collect_candidates(else_body, function_ret, parameter_views, facts);
            }
            S::Return(None)
            | S::Return(Some(_))
            | S::Expr(_)
            | S::Spawn(_)
            | S::Match(..)
            | S::While(..)
            | S::For(..)
            | S::Scope(..) => {}
        }
    }
}

fn find_declarations(statements: &[Stmt], declarations: &mut HashSet<BindingId>) {
    for statement in statements {
        match &statement.kind {
            S::Assign { declare: true, .. } => {
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
            S::Assign { .. }
            | S::Return(_)
            | S::Expr(_)
            | S::Spawn(_)
            | S::Match(..)
            | S::While(..)
            | S::For(..)
            | S::Scope(..) => {}
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

    fn merge_states(
        &mut self,
        binding: BindingId,
        left: ValueState,
        right: ValueState,
        left_tail: &mut Vec<Action>,
        right_tail: &mut Vec<Action>,
    ) -> ValueState {
        if left == right {
            return left;
        }
        let retained: BTreeSet<_> = left.retained().union(right.retained()).cloned().collect();
        let both_active =
            matches!(left, ValueState::Active { .. }) && matches!(right, ValueState::Active { .. });
        if Self::state_slot(&left) == Self::state_slot(&right) {
            return match Self::state_slot(&left) {
                Some(slot) if both_active => ValueState::Active {
                    slot: slot.to_owned(),
                    retained,
                },
                Some(slot) => ValueState::MaybeMoved {
                    slot: slot.to_owned(),
                    retained,
                },
                None => ValueState::Moved { retained },
            };
        }

        let phi = self.fresh_slot(binding);
        let transfer_edge = |state: ValueState, tail: &mut Vec<Action>| match state {
            ValueState::Active { slot: from, .. } | ValueState::MaybeMoved { slot: from, .. } => {
                tail.push(Action::Transfer {
                    from,
                    to: phi.clone(),
                });
            }
            ValueState::Moved { .. } => tail.push(Action::InitEmpty { slot: phi.clone() }),
        };
        transfer_edge(left.clone(), left_tail);
        transfer_edge(right.clone(), right_tail);
        if both_active {
            ValueState::Active {
                slot: phi,
                retained,
            }
        } else {
            ValueState::MaybeMoved {
                slot: phi,
                retained,
            }
        }
    }

    fn block(
        &mut self,
        statements: &[Stmt],
        mut state: HashMap<BindingId, ValueState>,
    ) -> Result<(Block, HashMap<BindingId, ValueState>), ()> {
        let mut planned = Block {
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
            };

            // Mutating contents changes the lifetime carried by the Vec type
            // without replacing its allocation. Move it to an independent
            // slot before evaluating the expression, at the same cleanup
            // anchor. This also preserves changes overwritten by an assignment
            // later in the same RHS. Branch statements own separate events.
            let mutated: BTreeSet<_> = facts
                .content_mutations
                .iter()
                .map(|mutation| mutation.binding)
                .filter(|binding| self.candidates.contains_key(binding))
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
                        facts.condition_after.as_deref().ok_or(())?,
                        &state,
                    );
                    let (then_plan, then_state) = self.block(then_body, branch_entry.clone())?;
                    let (else_plan, else_state) = self.block(else_body, branch_entry.clone())?;
                    let then_continues = !then_plan.terminal;
                    let else_continues = !else_plan.terminal;
                    node.children = vec![then_plan, else_plan];

                    match (then_continues, else_continues) {
                        (true, true) => {
                            let visible: BTreeSet<_> = facts
                                .after
                                .iter()
                                .map(|snapshot| snapshot.binding)
                                .filter(|binding| self.candidates.contains_key(binding))
                                .collect();
                            let ids: Vec<_> = visible.iter().copied().collect();
                            for binding in ids {
                                let left = then_state
                                    .get(&binding)
                                    .cloned()
                                    .or_else(|| branch_entry.get(&binding).cloned());
                                let right = else_state
                                    .get(&binding)
                                    .cloned()
                                    .or_else(|| branch_entry.get(&binding).cloned());
                                let (Some(left), Some(right)) = (left, right) else {
                                    continue;
                                };
                                let (left_children, right_children) = node.children.split_at_mut(1);
                                let merged = self.merge_states(
                                    binding,
                                    left,
                                    right,
                                    &mut left_children[0].tail,
                                    &mut right_children[0].tail,
                                );
                                state.insert(binding, merged);
                            }
                            state.retain(|binding, _| visible.contains(binding));
                        }
                        (true, false) => {
                            state = then_state
                                .into_iter()
                                .filter(|(binding, _)| {
                                    facts
                                        .after
                                        .iter()
                                        .any(|snapshot| snapshot.binding == *binding)
                                })
                                .collect();
                        }
                        (false, true) => {
                            state = else_state
                                .into_iter()
                                .filter(|(binding, _)| {
                                    facts
                                        .after
                                        .iter()
                                        .any(|snapshot| snapshot.binding == *binding)
                                })
                                .collect();
                        }
                        (false, false) => {
                            state.clear();
                        }
                    }
                }
                S::Match(..) | S::While(..) | S::For(..) | S::Scope(..) => return Err(()),
            }

            planned.statements.push(node);
        }

        Ok((planned, state))
    }

    fn condition_state(
        &self,
        before: &[ViewListBindingSnapshot],
        condition_after: &[ViewListBindingSnapshot],
        state: &HashMap<BindingId, ValueState>,
    ) -> HashMap<BindingId, ValueState> {
        let mut result = state.clone();
        self.apply_after_facts(before, condition_after, &mut result);
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
                                ty: view_list_type(),
                            }
                        }));
                    }
                }
            }
        }
        for child in &mut node.children {
            local_declarations(child, builder);
        }
    }
}

pub(crate) fn plan(function: &Function) -> Option<Plan> {
    if function.asynchronous || !function.ret.is_view_string_list() {
        return None;
    }
    if !crate::ast::supports_view_flow(&function.body) {
        return None;
    }

    let parameter_ids = parameter_bindings(function);
    let parameter_views = parameter_view_ids(function);
    let mut facts = BTreeMap::new();
    collect_candidates(&function.body, &function.ret, &parameter_views, &mut facts);
    let mut declarations = HashSet::new();
    find_declarations(&function.body, &mut declarations);

    let candidates: HashMap<_, _> = facts
        .into_iter()
        .filter_map(|(binding, fact)| {
            if !fact.saw_short_owner || !fact.saw_safe_direct_return {
                return None;
            }
            let parameter = parameter_ids.contains_key(&binding);
            if !parameter && !declarations.contains(&binding) {
                return None;
            }
            Some((
                binding,
                Candidate {
                    name: fact.name?,
                    parameter,
                },
            ))
        })
        .collect();
    if candidates.is_empty() {
        return None;
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
            ty: view_list_type(),
        });
        actions.push(Action::Transfer {
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

    let (mut body, _) = builder.block(&function.body, state).ok()?;
    local_declarations(&mut body, &builder);
    for (binding, slots) in &builder.slots {
        if !builder.candidates[binding].parameter {
            continue;
        }
        let name = &builder.candidates[binding].name;
        let actions = parameter_actions.get_mut(name)?;
        actions.extend(
            slots
                .iter()
                .skip(1)
                .cloned()
                .map(|name| Action::DeclareSlot {
                    name,
                    ty: view_list_type(),
                }),
        );
    }

    Some(Plan {
        body,
        parameter_actions,
    })
}
