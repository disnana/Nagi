use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi actor charging {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn checked(&self, text: &str) -> nagic::ast::Program {
        self.write("actor-charge.nagi", text);
        let mut loaded = source::load(&self.0.join("actor-charge.nagi"), true).unwrap();
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        loaded.program
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const DATA: &str = r#"import std.actor as actor
class Inline:
    value: i64
class Owned:
    label: str
    values: List[Inline]
class Node:
    children: List[Node]
class Caused:
    cause: Error
enum Packet:
    Empty
    Number(value: i64)
    Data(value: Owned)
"#;

#[test]
fn actor_charge_generation_survives_low_without_clones_reflection_or_serde_requirements() {
    let f = Fixture::new();
    let high = f.checked(DATA);
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let high_rust = emit::rust(&high).unwrap();
    assert_eq!(high_rust, emit::rust(&low).unwrap());
    assert!(
        high_rust.contains("ChargeOwned") && high_rust.contains("INLINE_ONLY"),
        "{high_rust}"
    );
    assert!(
        !high_rust.contains(".clone()") && !high_rust.contains("BoxFuture"),
        "{high_rust}"
    );
    let packet = high.modules.resolve_root_path("Packet").unwrap();
    let class = high.modules.resolve_root_path("Caused").unwrap();
    assert!(high_rust.contains(&format!("for {}", packet.symbol)));
    assert!(high_rust.contains(&format!("for {}", class.symbol)));
    // A private builtin Error remains chargeable without acquiring a JSON wire
    // format. A generated implementation must not rely on Serialize.
    let private = high_rust
        .find(&format!("pub struct {}", class.symbol))
        .unwrap();
    assert!(high_rust[private..].contains("ChargeOwned"));
    let http_only = f.checked(&DATA.replace(
        "import std.actor as actor",
        "import std.http.server as http",
    ));
    assert!(
        !emit::rust(&http_only).unwrap().contains("ChargeOwned"),
        "HTTP-only code acquired actor charging work"
    );
}

#[test]
fn generated_nominal_charges_execute_against_native_capacity_and_walk_limits() {
    let f = Fixture::new();
    let program = f.checked(&format!("{DATA}\n@rust(\"native::probe\")\nextern def probe() -> Result[unit, Error]\ndef main() -> Result[unit, Error]:\n    return probe()\n"));
    f.write("actor-charge.low", &emit::low(&program));
    f.write("native.rs", PROBE);
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    for name in ["actor-charge.nagi", "actor-charge.low"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&f.0)
            .args([
                "run",
                name,
                "--rust",
                "native.rs",
                "--out",
                "build",
                "--no-project",
            ])
            .env("NAGI_NATIVE_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let (launcher, application) = stdout.split_once('\n').unwrap();
        let executable = launcher
            .trim_end_matches('\r')
            .strip_prefix("native: ")
            .unwrap();
        assert!(Path::new(executable).is_file());
        assert_eq!(
            application.trim(),
            "generated charges preserve capacity and active variants",
            "{name}"
        );
        if name.ends_with(".nagi") {
            fs::remove_file(f.0.join(name)).unwrap();
        }
    }
}

const PROBE: &str = r#"
use nagi_runtime::actor::{charged_bytes, ChargeError, ChargeOwned};
use std::{mem::size_of, time::{Duration, Instant}};
pub fn probe() -> Result<(), nagi_runtime::Error> {
    let mut label = String::with_capacity(128); label.push_str("short");
    let mut values = Vec::with_capacity(32); values.push(crate::Inline { value: 7 });
    let value = crate::Owned { label, values };
    let heap = value.label.capacity() + value.values.capacity() * size_of::<crate::Inline>();
    let expected = 17 + size_of::<crate::Owned>() + heap;
    assert!(<crate::Inline as ChargeOwned>::INLINE_ONLY);
    assert!(!<crate::Owned as ChargeOwned>::INLINE_ONLY);
    let (actual, allocations) = nagi_runtime::metrics::measure(|| charged_bytes(&value, 17, expected, None));
    assert_eq!(allocations.allocations, 0, "generated structural charging allocated");
    assert_eq!(allocations.reallocations, 0, "generated structural charging reallocated");
    assert_eq!(allocations.allocated_bytes, 0, "generated structural charging retained temporary bytes");
    let actual = actual.unwrap();
    assert_eq!(actual, expected, "inline fields were double counted or capacity replaced by length");
    assert!(matches!(charged_bytes(&value, 17, expected - 1, None), Err(ChargeError::TooLarge)));
    let empty = crate::Packet::Empty;
    assert_eq!(charged_bytes(&empty, 0, usize::MAX, None).unwrap(), size_of::<crate::Packet>());
    let number = crate::Packet::Number { value: 42 };
    assert_eq!(charged_bytes(&number, 0, usize::MAX, None).unwrap(), size_of::<crate::Packet>());
    let packet = crate::Packet::Data { value };
    assert_eq!(charged_bytes(&packet, 0, usize::MAX, None).unwrap(), size_of::<crate::Packet>() + heap);
    let mut message = String::with_capacity(96); message.push_str("private cause");
    let caused = crate::Caused { cause: nagi_runtime::Error::invalid(message) };
    assert_eq!(charged_bytes(&caused, 0, usize::MAX, None).unwrap(), size_of::<crate::Caused>() + caused.cause.message.capacity());
    let zero_sized = Vec::<()>::with_capacity(100);
    assert_eq!(charged_bytes(&zero_sized, 0, usize::MAX, None).unwrap(), size_of::<Vec<()>>());
    let mut numeric = Vec::with_capacity(100_000);
    numeric.resize_with(100_000, || crate::Inline { value: 1 });
    assert_eq!(charged_bytes(&numeric, 0, usize::MAX, None).unwrap(), size_of::<Vec<crate::Inline>>() + numeric.capacity() * size_of::<crate::Inline>(), "INLINE_ONLY elements must not exhaust the walk budget");
    let mut deep = crate::Node { children: vec![] };
    for _ in 0..100 { deep = crate::Node { children: vec![deep] }; }
    assert!(matches!(charged_bytes(&deep, 0, usize::MAX, None), Err(ChargeError::Depth)));
    let mut broad = Vec::with_capacity(65_537);
    broad.resize_with(65_537, String::new);
    assert!(matches!(charged_bytes(&broad, 0, usize::MAX, None), Err(ChargeError::Work)));
    let expired = Instant::now().checked_sub(Duration::from_millis(1)).unwrap();
    assert!(matches!(charged_bytes(&packet, 0, usize::MAX, Some(expired)), Err(ChargeError::Deadline)));
    println!("generated charges preserve capacity and active variants");
    Ok(())
}
"#;
