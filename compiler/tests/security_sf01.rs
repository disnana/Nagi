use nagic::{check, emit, source};
#[cfg(nagi_sf01_diagnostic)]
use std::cell::Cell;
use std::{fs, path::PathBuf};

const SLOT_POLICY: u8 = 1;
const SLOT_NATIVE_REPLACEMENT: u8 = 2;
const SLOT_NONCLONE: u8 = 3;
const SLOT_NONSHARED: u8 = 4;

#[derive(Clone, Copy)]
#[repr(u8)]
enum Phase {
    TestStart = 1,
    CaseStart = 2,
    HighWrite = 3,
    HighLoad = 4,
    LowEmit = 5,
    LowAnchor = 6,
    CandidateWrite = 7,
    SourceLoad = 8,
    Check = 9,
    ChildStart = 10,
    ChildSuccess = 11,
    ChildExit = 12,
    ChildSignal = 13,
    CleanupSuccess = 14,
    CleanupFailure = 15,
    CaseEnd = 16,
    TestEnd = 17,
}
#[cfg(nagi_sf01_diagnostic)]
struct Observation {
    slot: u8,
    case: Cell<u8>,
}
#[cfg(not(nagi_sf01_diagnostic))]
struct Observation;

impl Observation {
    fn off() -> Self {
        #[cfg(nagi_sf01_diagnostic)]
        {
            Self {
                slot: 0,
                case: Cell::new(0),
            }
        }
        #[cfg(not(nagi_sf01_diagnostic))]
        {
            Self
        }
    }

    fn new(slot: u8, case: u8) -> Self {
        #[cfg(nagi_sf01_diagnostic)]
        {
            Self {
                slot,
                case: Cell::new(case),
            }
        }
        #[cfg(not(nagi_sf01_diagnostic))]
        {
            let _ = (slot, case);
            Self
        }
    }

    fn set_case(&self, case: u8) {
        #[cfg(nagi_sf01_diagnostic)]
        self.case.set(case);
        #[cfg(not(nagi_sf01_diagnostic))]
        let _ = case;
    }

    fn mark(&self, phase: Phase) {
        #[cfg(nagi_sf01_diagnostic)]
        sf01_diag::record(self.slot, self.case.get(), phase as u8);
        #[cfg(not(nagi_sf01_diagnostic))]
        let _ = phase;
    }
}

#[cfg(nagi_sf01_diagnostic)]
mod sf01_diag {
    use std::io::{self, Write};
    use std::sync::Mutex;

    const MAX_EVENTS: usize = 512;
    const MAX_BYTES: usize = 32 * 1024;
    const OVERFLOW: &[u8] = b"SF01D|overflow\n";

    #[derive(Clone, Copy)]
    struct Limits {
        events: usize,
        bytes: usize,
    }

    impl Limits {
        const DEFAULT: Self = Self {
            events: MAX_EVENTS,
            bytes: MAX_BYTES,
        };
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum RecordResult {
        Written,
        Overflow,
        UnknownSlot,
        UnknownPhase,
        IoFailure,
    }

    struct Budget {
        limits: Limits,
        events: usize,
        bytes: usize,
        overflowed: bool,
    }

    impl Budget {
        const fn new(limits: Limits) -> Self {
            Self {
                limits,
                events: 0,
                bytes: 0,
                overflowed: false,
            }
        }

        fn push<W: Write>(
            &mut self,
            output: &mut W,
            slot: u8,
            case: u8,
            phase: u8,
        ) -> RecordResult {
            if !(1..=4).contains(&slot) {
                return RecordResult::UnknownSlot;
            }
            if !(1..=17).contains(&phase) {
                return RecordResult::UnknownPhase;
            }
            if self.overflowed {
                return RecordResult::Overflow;
            }

            let mut line = [0u8; 32];
            let mut cursor = io::Cursor::new(&mut line[..]);
            if writeln!(&mut cursor, "SF01D|s={slot}|c={case}|p={phase}").is_err() {
                return RecordResult::IoFailure;
            }
            let len = cursor.position() as usize;
            let normal_events = self.limits.events.saturating_sub(1);
            let normal_bytes = self.limits.bytes.saturating_sub(OVERFLOW.len());
            if self.events < normal_events && self.bytes.saturating_add(len) <= normal_bytes {
                self.events += 1;
                self.bytes += len;
                return if output.write_all(&line[..len]).is_ok() {
                    RecordResult::Written
                } else {
                    RecordResult::IoFailure
                };
            }

            self.overflowed = true;
            if self.events < self.limits.events
                && self.bytes.saturating_add(OVERFLOW.len()) <= self.limits.bytes
            {
                self.events += 1;
                self.bytes += OVERFLOW.len();
                if output.write_all(OVERFLOW).is_err() {
                    return RecordResult::IoFailure;
                }
            }
            RecordResult::Overflow
        }
    }

    static OUTPUT: Mutex<Budget> = Mutex::new(Budget::new(Limits::DEFAULT));

    pub(super) fn record(slot: u8, case: u8, phase: u8) {
        let mut budget = OUTPUT
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut stderr = io::stderr();
        let _ = budget.push(&mut stderr, slot, case, phase);
    }

    #[cfg(nagi_sf01_diag_oracle)]
    pub(super) fn oracle() {
        let expected = b"SF01D|s=1|c=0|p=1\nSF01D|overflow\n";
        let mut events = Vec::new();
        let mut event_budget = Budget::new(Limits {
            events: 2,
            bytes: MAX_BYTES,
        });
        assert_eq!(
            event_budget.push(&mut events, 1, 0, 1),
            RecordResult::Written
        );
        assert_eq!(
            event_budget.push(&mut events, 1, 0, 1),
            RecordResult::Overflow
        );
        assert_eq!(events.as_slice(), &expected[..]);
        assert_eq!(event_budget.events, 2);
        assert!(event_budget.bytes <= MAX_BYTES);
        let before_unknown = events.clone();
        assert_eq!(
            event_budget.push(&mut events, 255, 0, 1),
            RecordResult::UnknownSlot
        );
        assert_eq!(events, before_unknown);
        assert_eq!(
            event_budget.push(&mut events, 1, 0, 255),
            RecordResult::UnknownPhase
        );
        assert_eq!(events, before_unknown);

        let mut sample = Vec::new();
        let mut sample_budget = Budget::new(Limits::DEFAULT);
        assert_eq!(
            sample_budget.push(&mut sample, 1, 0, 1),
            RecordResult::Written
        );
        let byte_limit = sample.len() + OVERFLOW.len();
        let mut byte_output = Vec::new();
        let mut byte_budget = Budget::new(Limits {
            events: 8,
            bytes: byte_limit,
        });
        assert_eq!(
            byte_budget.push(&mut byte_output, 1, 0, 1),
            RecordResult::Written
        );
        assert_eq!(
            byte_budget.push(&mut byte_output, 1, 0, 1),
            RecordResult::Overflow
        );
        assert!(byte_output.ends_with(OVERFLOW));
        assert!(byte_output.len() <= byte_limit);

        // The formatter accepts only closed numeric IDs; it has no source,
        // path, error, environment, argument, or register input.
        assert!(events.is_ascii());
    }
}

struct Fixture(PathBuf, Observation);
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir();
        let path = (0u64..)
            .find_map(|n| {
                let p = base.join(format!("nagi-sf01-{}-{n}", std::process::id()));
                match fs::create_dir(&p) {
                    Ok(()) => Some(p),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(e) => panic!("{e}"),
                }
            })
            .unwrap();
        Self(path, Observation::off())
    }
    fn observed(slot: u8, case: u8) -> Self {
        let mut fixture = Self::new();
        fixture.1 = Observation::new(slot, case);
        fixture
    }
    fn mark(&self, phase: Phase) {
        self.1.mark(phase);
    }
    fn set_case(&self, case: u8) {
        self.1.set_case(case);
    }
    fn reject(&self, text: &str, expected: &str, line: usize, low_anchor: &str) {
        self.mark(Phase::CaseStart);
        let high = self.0.join("source.nagi");
        self.mark(Phase::HighWrite);
        fs::write(&high, text).unwrap();
        // Loading must succeed before the purpose-specific checker rejection.
        self.mark(Phase::HighLoad);
        let loaded = source::load(&high, true).unwrap();
        self.mark(Phase::LowEmit);
        let low = emit::low(&loaded.program);
        self.mark(Phase::LowAnchor);
        let low_line = low
            .lines()
            .position(|s| s.contains(low_anchor))
            .unwrap_or_else(|| panic!("expected Low origin {low_anchor}: {low}"))
            + 1;
        for (name, text, high) in [
            ("source.nagi", text.to_owned(), true),
            ("saved.low", low, false),
        ] {
            let path = self.0.join(name);
            self.mark(Phase::CandidateWrite);
            fs::write(&path, &text).unwrap();
            self.mark(Phase::SourceLoad);
            let mut loaded = source::load(&path, high).unwrap();
            self.mark(Phase::Check);
            let error =
                check::check(&mut loaded.program).expect_err("legacy security entry accepted");
            assert!(error.contains(expected), "{error}");
            let d = loaded.diagnostic(&error);
            assert!(d.contains(name), "{d}");
            let expected_line = if high { line } else { low_line };
            assert!(d.contains(&format!("{name}:{expected_line}\n")), "{d}");
            assert!(!d.contains("internal compiler error"), "{d}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let result = fs::remove_dir_all(&self.0);
        self.1.mark(if result.is_ok() {
            Phase::CleanupSuccess
        } else {
            Phase::CleanupFailure
        });
        result.unwrap();
    }
}
#[cfg(all(nagi_sf01_diagnostic, nagi_sf01_diag_oracle))]
#[test]
fn sf01_diagnostic_observer_oracle() {
    sf01_diag::oracle();
}
#[test]
fn legacy_decorator_is_a_checker_migration_diagnostic() {
    Fixture::new().reject(
        "@get(\"/answer\")\nasync def health() -> Result[i64, Error]:\n    return ok(7)\n",
        "SF01 migration",
        2,
        "async fn ",
    );
}
#[test]
fn legacy_global_serve_is_a_checker_migration_diagnostic() {
    Fixture::new().reject("async def main() -> Result[unit, Error]:\n    db = 0\n    try await serve(db, 0)\n    return ok(print(0))\n", "SF01 migration",3,"await serve");
}
#[test]
fn legacy_principal_is_a_checker_migration_diagnostic() {
    Fixture::new().reject(
        "import std.auth as auth\n@rust(\"native::old\")\nextern def old() -> auth.Principal\n",
        "SF01 migration",
        3,
        "extern fn ",
    );
}
#[test]
fn policy_free_route_is_a_checker_migration_diagnostic() {
    Fixture::new().reject("import std.http.server as http\nclass State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State]) -> Result[http.Response, Error]:\n    return http.empty(http.response_context(view(request)), http.Status.OK)\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=7))\n    return http.route(app, http.Method.GET, \"/\", handler)\n", "SF01 migration",8,"_f_726f757465(");
}

#[test]
fn handwritten_low_cannot_use_retired_or_policy_free_entrypoints() {
    for (text,line) in [
  ("@get(\"/answer\")\nasync fn handler() -> Result[i64, Error] { return ok(7); }\n",2),
  ("import std.auth as auth;\n@rust(\"native::old\")\nextern fn old() -> auth.Principal;\n",3),
  ("async fn main() -> Result[unit, Error] {\n let db = 0;\n try await serve(db,0);\n return ok(print(0));\n}\n",3),
  ("import std.http.server as http;\nasync fn handler(r: http.Request, s: shared[i64]) -> Result[http.Response, Error] { return http.empty(http.response_context(view(r)), http.Status.OK); }\nfn setup() -> Result[http.App[i64, Error], Error] {\n let app = http.app_default[i64](7);\n return http.route(app,http.Method.GET,\"/\",handler);\n}\n",5),
 ] {
  let fixture=Fixture::new();let path=fixture.0.join("manual.low");fs::write(&path,text).unwrap();let mut loaded=source::load(&path,false).unwrap();let e=check::check(&mut loaded.program).expect_err(text);assert!(e.contains("SF01 migration"),"{e}");let d=loaded.diagnostic(&e);assert!(d.contains(&format!("manual.low:{line}\n")),"{d}");
 }
}
#[test]
fn policy_output_state_and_callback_contracts_are_checked() {
    let fixture = Fixture::observed(SLOT_POLICY, 0);
    fixture.mark(Phase::TestStart);
    let baseline = r#"import std.auth as auth
import std.http.server as http
class State:
    value: i64
@rust("native::verify")
extern async def verify(r: http.Request, s: shared[State]) -> Result[auth.VerifiedIdentity, auth.Failure]
async def handler(r: http.Request, s: shared[State], a: unit) -> Result[http.Response, Error]:
    return http.empty(http.response_context(view(r)), http.Status.OK)
def setup() -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](State(value=1))
    return http.route(app, http.Method.GET, "/", http.public_policy[State](), handler)
"#;
    for (case, (text, reason)) in [
        (
            baseline.replace(
                "http.public_policy[State]()",
                "http.authenticated_policy[State](verify)",
            ),
            "型",
        ),
        (
            baseline.replace("http.public_policy[State]()", "http.public_policy[i64]()"),
            "型",
        ),
        (
            baseline
                .replace(
                    "Result[auth.VerifiedIdentity, auth.Failure]",
                    "Result[auth.VerifiedIdentity, Error]",
                )
                .replace(
                    "http.public_policy[State]()",
                    "http.authenticated_policy[State](verify)",
                ),
            "型",
        ),
        (
            baseline.replace(
                "http.public_policy[State]()",
                "http.authenticated_policy[State](handler)",
            ),
            "型",
        ),
        (baseline.replace("a: unit", "a: auth.AuthScope"), "型"),
    ]
    .into_iter()
    .enumerate()
    {
        fixture.set_case(case as u8);
        fixture.mark(Phase::CaseStart);
        let high = fixture.0.join("bad.nagi");
        fixture.mark(Phase::HighWrite);
        fs::write(&high, &text).unwrap();
        fixture.mark(Phase::HighLoad);
        let loaded = source::load(&high, true).unwrap();
        fixture.mark(Phase::LowEmit);
        let saved = emit::low(&loaded.program);
        for (name, text, high) in [("bad.nagi", text, true), ("bad.low", saved, false)] {
            let path = fixture.0.join(name);
            fixture.mark(Phase::CandidateWrite);
            fs::write(&path, text).unwrap();
            fixture.mark(Phase::SourceLoad);
            let mut loaded = source::load(&path, high).unwrap();
            fixture.mark(Phase::Check);
            let e =
                check::check(&mut loaded.program).expect_err("invalid policy contract accepted");
            assert!(e.contains(reason), "{e}");
            assert!(!e.contains("internal compiler error"), "{e}");
        }
    }
    fixture.mark(Phase::TestEnd);
}
#[test]
fn native_replacements_and_aliases_cannot_downgrade_security_contracts() {
    use std::process::Command;
    let fixture = Fixture::observed(SLOT_NATIVE_REPLACEMENT, u8::MAX);
    fixture.mark(Phase::TestStart);
    let high = r#"import std.http.server as http
import std.auth as auth
from std.auth import AuthScope as Identity
async def handler(r: http.Request, s: shared[i64], a: unit) -> Result[http.Response, Error]:
    return http.empty(http.response_context(view(r)), http.Status.OK)
def setup() -> Result[http.App[i64, Error], Error]:
    app = http.app_default[i64](7)
    return http.route(app,http.Method.GET,"/",http.public_policy[i64](),handler)
async def use_scope(value: Identity):
    print(auth.subject(view(value)))
async def guarded(value: Identity) -> Result[unit, Error]:
    await use_scope(value)
    return ok(print(0))
"#;
    let high_path = fixture.0.join("main.nagi");
    fixture.mark(Phase::HighWrite);
    fs::write(&high_path, high).unwrap();
    fixture.mark(Phase::HighLoad);
    let mut sources = source::load(&high_path, true).unwrap();
    fixture.mark(Phase::Check);
    check::check(&mut sources.program).unwrap();
    fixture.mark(Phase::LowEmit);
    let saved = emit::low(&sources.program);
    fixture.mark(Phase::CandidateWrite);
    fs::write(fixture.0.join("saved.low"), saved).unwrap();
    for (replacement_case, (replacement,reason,line)) in [
  ("@replace generated::setup\nfn changed() -> Result[http.App[i64, Error], Error] {\n let app = http.app_default[i64](7);\n return http.route(app,http.Method.GET,\"/\",handler);\n}\n","SF01 migration",4),
  ("@replace generated::handler\nasync fn changed(r: http.Request, s: shared[i64], a: auth.AuthScope) -> Result[http.Response, Error] { return http.empty(http.response_context(view(r)), http.Status.OK); }\n","一致しません",2),
  ("@replace generated::guarded\nasync fn changed(value: Identity) -> Result[unit, Error] {\n scope { spawn use_scope(value); }\n return ok(print(0));\n}\n","SameTask",3),
  ("@replace generated::setup\nfn changed() -> Result[http.App[i64, Error], Error] {\n let values = [some(http.public_policy[i64]())];\n let copied = copy(view(values));\n let app = http.app_default[i64](7);\n return http.route(app,http.Method.GET,\"/\",http.public_policy[i64](),handler);\n}\n","非Clone security resource",4),
  ("@replace generated::setup\nfn changed() -> Result[http.App[i64, Error], Error] {\n let invalid = share(some(http.public_policy[i64]()));\n let app = http.app_default[i64](7);\n return http.route(app,http.Method.GET,\"/\",http.public_policy[i64](),handler);\n}\n","非共有 security resource",3),
 ].into_iter().enumerate() {
  fixture.set_case((replacement_case * 4) as u8);
  fixture.mark(Phase::CandidateWrite);
  fs::write(fixture.0.join("replace.low"),replacement).unwrap();
  for (source_index, source) in ["main.nagi","saved.low"].into_iter().enumerate() {
   for (command_index, command) in ["check","build"].into_iter().enumerate() {
    fixture.set_case((replacement_case * 4 + source_index * 2 + command_index) as u8);
    fixture.mark(Phase::CaseStart);
    fixture.mark(Phase::ChildStart);
    let output=Command::new(env!("CARGO_BIN_EXE_nagic")).current_dir(&fixture.0).args([command,source,"--no-project","--native","replace.low"]).env("PATH","").output().unwrap();
    if output.status.success() { fixture.mark(Phase::ChildSuccess); }
    else if output.status.code().is_none() { fixture.mark(Phase::ChildSignal); }
    else { fixture.mark(Phase::ChildExit); }
    let error=String::from_utf8_lossy(&output.stderr);
   assert!(!output.status.success(),"{replacement}");assert!(error.contains(reason),"{source}/{command}: {error}");assert!(error.contains(&format!("replace.low:{line}\n")),"{error}");assert!(!error.contains("Cargoが見つかりません") && !error.contains("internal compiler error"),"{error}");
    fixture.mark(Phase::CaseEnd);
   }
  }
 }
    fixture.set_case(u8::MAX);
    fixture.mark(Phase::TestEnd);
}

#[test]
fn security_nonclone_payloads_are_rejected_at_copy_in_high_and_saved_low() {
    let observer = Observation::new(SLOT_NONCLONE, u8::MAX);
    observer.mark(Phase::TestStart);
    for (case, (source, line)) in [
        ("import std.http.server as http\ndef main():\n    values = [some(http.public_policy[i64]())]\n    duplicate = copy(view(values))\n    print(len(duplicate))\n", 4),
        ("import std.auth as auth\ndef main():\n    values = [some(auth.denied())]\n    duplicate = copy(view(values))\n    print(len(duplicate))\n", 4),
        ("import std.auth as auth\n@rust(\"native::identity\")\nextern def identity() -> auth.VerifiedIdentity\ndef main():\n    values = [some(identity())]\n    duplicate = copy(view(values))\n    print(len(duplicate))\n", 6),
        ("import std.http.server as http\nclass Holder:\n    policy: http.Policy[i64, unit]\ndef main():\n    values = [Holder(policy=http.public_policy[i64]())]\n    duplicate = copy(view(values))\n    print(len(duplicate))\n", 6),
    ].into_iter().enumerate() {
        Fixture::observed(SLOT_NONCLONE, case as u8)
            .reject(source, "非Clone security resource", line, "copy(");
    }
    observer.mark(Phase::TestEnd);
}

#[test]
fn security_nonshared_payloads_are_rejected_through_wrappers_and_fields() {
    let observer = Observation::new(SLOT_NONSHARED, u8::MAX);
    observer.mark(Phase::TestStart);
    for (case, (source, line, anchor)) in [
        ("import std.http.server as http\ndef accept(value: shared[http.Policy[i64,unit]]):\n    print(0)\n", 2, "fn "),
        ("import std.http.server as http\ndef main():\n    shared_policy = share(some(http.public_policy[i64]()))\n", 3, "share("),
        ("import std.http.server as http\nclass Holder:\n    policy: http.Policy[i64,unit]\ndef main():\n    shared_policy = share(Holder(policy=http.public_policy[i64]()))\n", 5, "share("),
        ("import std.auth as auth\ndef main():\n    shared_failure = share(some(auth.denied()))\n", 3, "share("),
        ("import std.auth as auth\n@rust(\"native::identity\")\nextern def identity() -> auth.VerifiedIdentity\ndef main():\n    shared_identity = share(some(identity()))\n", 5, "share("),
        ("import std.http.server as http\nclass Holder:\n    policy: http.Policy[i64,unit]\ndef accept(value: shared[Holder]):\n    print(0)\n", 4, "fn "),
        ("import std.http.server as http\nclass Holder:\n    policy: http.Policy[i64,unit]\ndef main():\n    app = http.app_default[Holder](Holder(policy=http.public_policy[i64]()))\n", 5, "app = "),
    ].into_iter().enumerate() {
        Fixture::observed(SLOT_NONSHARED, case as u8)
            .reject(source, "非共有 security resource", line, anchor);
    }
    observer.mark(Phase::TestEnd);
}

#[test]
fn handwritten_low_rejects_security_clone_and_share_before_cargo() {
    use std::process::Command;
    for (text, reason, line) in [
        ("import std.http.server as http;\nfn main() {\n let values = [some(http.public_policy[i64]())];\n let duplicate = copy(view(values));\n print(len(duplicate));\n}\n", "非Clone security resource", 4),
        ("import std.auth as auth;\nfn main() {\n let values = [some(auth.denied())];\n let duplicate = copy(view(values));\n print(len(duplicate));\n}\n", "非Clone security resource", 4),
        ("import std.http.server as http;\nfn accept(value: shared[http.Policy[i64,unit]]) { print(0); }\n", "非共有 security resource", 2),
        ("import std.http.server as http;\nrecord Holder { policy: http.Policy[i64,unit]; }\nfn main() {\n let invalid = share(Holder(policy=http.public_policy[i64]()));\n}\n", "非共有 security resource", 4),
    ] {
        let fixture = Fixture::new();
        let path = fixture.0.join("manual.low");
        fs::write(&path, text).unwrap();
        source::load(&path, false).expect("Low must parse and resolve");
        for command in ["check", "build"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&fixture.0).args([command, "manual.low", "--no-project"]).env("PATH", "").output().unwrap();
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{text}");
            assert!(error.contains(reason), "{error}");
            assert!(error.contains(&format!("manual.low:{line}\n")), "{error}");
            assert!(!error.contains("Cargoが見つかりません") && !error.contains("internal compiler error"), "{error}");
        }
    }
}
