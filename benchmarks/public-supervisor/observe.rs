// Linux-only process/task/timing observations. Every actor/supervisor operation
// and worker handler lives in main.nagi. This adapter never spawns a task.
use nagi_runtime::serde_json::{json, Value};
use std::io::{self, Write};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Default)]
struct Phase {
    started: Option<Instant>,
    cpu: i64,
    clients: [Vec<(i64, i64, bool)>; 2],
    fault: [Option<i64>; 2],
}
static PHASE: Mutex<Phase> = Mutex::new(Phase {
    started: None, cpu: 0, clients: [Vec::new(), Vec::new()], fault: [None, None],
});

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
fn cpu_ns() -> i64 {
    // Linux 64-bit struct timespec ABI; CLOCK_PROCESS_CPUTIME_ID observes this
    // process's total CPU, not wall time or one thread. No production FFI.
    #[repr(C)] struct Timespec { sec: i64, nsec: i64 }
    unsafe extern "C" { fn clock_gettime(clock: i32, value: *mut Timespec) -> i32; }
    let mut value = Timespec { sec: 0, nsec: 0 };
    // SAFETY: pointer is a live aligned timespec; this clock writes only it.
    assert_eq!(unsafe { clock_gettime(2, &mut value) }, 0);
    value.sec * 1_000_000_000 + value.nsec
}
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
fn cpu_ns() -> i64 { panic!("public Supervisor process probe requires Linux 64-bit"); }

fn emit(value: Value) {
    println!("{}", value);
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
pub fn begin_phase(phase: i64) {
    assert!((0..3).contains(&phase));
    let mut state = PHASE.lock().unwrap();
    *state = Phase { started: Some(Instant::now()), cpu: cpu_ns(), ..Phase::default() };
}
pub fn record(client: i64, start: i64, end: i64, success: bool) {
    assert!((0..=1).contains(&client) && end >= start);
    let mut state = PHASE.lock().unwrap();
    let samples = &mut state.clients[client as usize];
    assert!(samples.len() < 20000);
    samples.push((start, end, success));
}
pub fn fault_mark(complete: bool, time: i64) {
    let mut state = PHASE.lock().unwrap();
    let target = &mut state.fault[usize::from(complete)];
    assert!(target.is_none());
    *target = Some(time);
}
pub fn end_phase(phase: i64) {
    let state = PHASE.lock().unwrap();
    let elapsed = state.started.unwrap().elapsed().as_nanos() as u64;
    let cpu = cpu_ns() - state.cpu;
    emit(json!({"phase": phase, "elapsed_ns": elapsed, "cpu_ns": cpu,
        "clients": state.clients, "fault_interval_ns": state.fault}));
}
