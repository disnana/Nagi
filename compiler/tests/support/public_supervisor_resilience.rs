//! Public Nagi Supervisor calls, unmodified emission and real production runtime.
use super::native_triple::Fixture;

#[test]
fn public_supervisor_restart_budget_and_cleanup_in_three_forms() {
    let high = include_str!("../fixtures/public-supervisor/contract.nagi");
    let low = include_str!("../fixtures/public-supervisor/contract.low");
    let observer = include_str!("../fixtures/public-supervisor/observer.rs");
    let adapter = format!("mod native {{ {observer} }}\nimpl Drop for Context {{ fn drop(&mut self) {{ native::context_dropped(self.slot); }} }}\nimpl Drop for State {{ fn drop(&mut self) {{ native::state_dropped(self.slot); }} }}");
    Fixture::new().run_three_with_assertions(
        "public-supervisor-resilience", high, low, &adapter,
        |program| {
            let exercise = program.modules.definitions.iter()
                .find(|d| d.id.name == "exercise").unwrap();
            format!("#[test] fn actual_public_supervisor_contract() {{ nagi_runtime::block_on({}()).unwrap(); }}", exercise.symbol)
        },
    );
}
