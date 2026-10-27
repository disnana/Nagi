// Observation only. Nagi owns every public Actor/Supervisor and every client.
use nagi_runtime::serde_json::{json, Value};
use std::io::{self, Write};
use std::sync::Mutex;
use std::time::Instant;

type Samples = (Vec<i64>, Vec<i64>, Vec<bool>);
struct Phase {
    started: Option<Instant>,
    cpu: i64,
    clients: Vec<Option<Samples>>,
    fault: [Option<i64>; 2],
    recovery: Option<[i64; 4]>,
}
static PHASE: Mutex<Phase> = Mutex::new(Phase {
    started: None, cpu: 0, clients: Vec::new(), fault: [None, None], recovery: None,
});

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
fn cpu_ns() -> i64 {
    #[repr(C)] struct Timespec { sec: i64, nsec: i64 }
    unsafe extern "C" { fn clock_gettime(clock: i32, value: *mut Timespec) -> i32; }
    let mut value = Timespec { sec: 0, nsec: 0 };
    // SAFETY: Linux64 CLOCK_PROCESS_CPUTIME_ID writes a valid aligned timespec.
    assert_eq!(unsafe { clock_gettime(2, &mut value) }, 0);
    value.sec * 1_000_000_000 + value.nsec
}
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
fn cpu_ns() -> i64 { panic!("public Actor probe requires Linux64"); }

fn emit(value: Value) {
    println!("{value}");
    io::stdout().flush().unwrap();
}
pub fn checkpoint(stage: i64) {
    assert!((0..=3).contains(&stage));
    emit(json!({"checkpoint": stage,
        "alive_tasks": tokio::runtime::Handle::current().metrics().num_alive_tasks()}));
    let mut line = String::new();
    assert!(io::stdin().read_line(&mut line).unwrap() > 0, "observer disconnected");
    assert_eq!(line.trim(), "continue");
}
pub fn begin_phase(clients: i64) {
    assert!((1..=128).contains(&clients));
    let mut state = PHASE.lock().unwrap();
    *state = Phase { started: Some(Instant::now()), cpu: cpu_ns(),
        clients: (0..clients).map(|_| None).collect(), fault: [None, None], recovery: None };
}
pub fn record_client(client: i64, starts: Vec<i64>, ends: Vec<i64>, successes: Vec<bool>) {
    assert!(starts.len() <= 5000 && starts.len() == ends.len() && ends.len() == successes.len());
    let mut state = PHASE.lock().unwrap();
    let target = &mut state.clients[usize::try_from(client).unwrap()];
    assert!(target.is_none(), "duplicate client completion");
    *target = Some((starts, ends, successes));
}
pub fn fault_mark(complete: bool, time: i64) {
    let mut state = PHASE.lock().unwrap();
    let target = &mut state.fault[usize::from(complete)];
    assert!(target.is_none());
    *target = Some(time);
}
pub fn recovery(failures: i64, restarts: i64, generation: i64, state: i64) {
    let mut phase = PHASE.lock().unwrap();
    assert!(phase.recovery.is_none());
    phase.recovery = Some([failures, restarts, generation, state]);
}
pub fn end_phase() {
    let mut state = PHASE.lock().unwrap();
    let elapsed = state.started.unwrap().elapsed().as_nanos() as u64;
    let cpu = cpu_ns() - state.cpu;
    // JSON construction and pipe writes follow the measured interval. During
    // calls, timing is per-client Nagi storage; one lock per completed client.
    let clients = state.clients.iter().map(|samples| {
        let (starts, ends, successes) = samples.as_ref().expect("missing client completion");
        starts.iter().zip(ends).zip(successes)
            .map(|((a,b),ok)| json!([a,b,ok])).collect::<Vec<_>>()
    }).collect::<Vec<_>>();
    emit(json!({"phase":0,"elapsed_ns":elapsed,"cpu_ns":cpu,
        "clients":clients,"fault_interval_ns":state.fault,"recovery":state.recovery}));
    state.clients.clear();
}
