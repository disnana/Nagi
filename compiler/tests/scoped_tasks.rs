#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

fn checked(source: &str) -> Result<nagic::ast::Program, String> {
    let mut program = parser::parse(source, true)?;
    check::check(&mut program)?;
    Ok(program)
}

fn roundtrip(source: &str) -> String {
    let high = checked(source).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    emit::rust(&checked_emission::seal(&low)).unwrap()
}

#[test]
fn copied_views_and_values_computed_from_views_can_enter_tasks() {
    for expression in ["copy(part)", "copy(view(text))", "copy(view(copy(part)))"] {
        roundtrip(&format!(
            "async def work(value: str):\n    print(value)\nasync def main() -> Result[unit, Error]:\n    text = \"Nagi\"\n    part = view(text)\n    async with scope:\n        spawn work({expression})\n        spawn sleep(len(part))\n    return ok(print(text))\n"
        ));
    }
}

#[test]
fn futures_that_still_hold_views_are_rejected_after_copying_containers() {
    for (ty, expression) in [
        ("view[str]", "part"),
        ("view[str]", "view(text)"),
        ("List[view[str]]", "[part]"),
        ("List[view[str]]", "copy(view([part]))"),
        ("view[str]?", "some(part)"),
        ("shared[view[str]]", "share(part)"),
    ] {
        let source = format!(
            "async def work(value: {ty}):\n    print(1)\nasync def main() -> Result[unit, Error]:\n    text = \"Nagi\"\n    part = view(text)\n    async with scope:\n        spawn work({expression})\n    return ok(print(text))\n"
        );
        let error = checked(&source).unwrap_err();
        assert!(
            error.starts_with("line 7:") && error.contains("viewを別taskへ渡せません"),
            "{error}"
        );
        let low = format!("async fn work(value: {ty}) -> unit {{ print(1); }}\nasync fn main() -> Result[unit, Error] {{\n    let text: str = \"Nagi\";\n    let part: view[str] = view(text);\n    scope {{\n        spawn work({expression});\n    }}\n    return ok(print(text));\n}}\n");
        let mut program = parser::parse(&low, false).unwrap();
        let error = check::check(&mut program).unwrap_err();
        assert!(error.contains("viewを別taskへ渡せません"), "{error}");
    }
}

#[test]
fn moving_an_owned_argument_still_consumes_it_before_later_parent_work() {
    let source = "async def work(value: str):\n    print(value)\nasync def main() -> Result[unit, Error]:\n    text = \"Nagi\"\n    async with scope:\n        spawn work(text)\n        print(text)\n    return ok(print(0))\n";
    let error = checked(source).unwrap_err();
    assert!(
        error.starts_with("line 7:") && error.contains("move後"),
        "{error}"
    );
}

#[test]
fn scope_rejects_known_foreign_error_types_and_preserves_local_error_adapters() {
    for ty in [
        "str",
        "bytes",
        "i64",
        "unit",
        "Db",
        "UUID",
        "timestamp",
        "List[i64]",
        "shared[Error]",
        "owned[str]",
    ] {
        let source = format!("async def work() -> Result[unit, {ty}]:\n    async with scope:\n        spawn sleep(1)\n    return ok(print(0))\n");
        let error = checked(&source).unwrap_err();
        assert!(
            error.starts_with("line 2:") && error.contains("scopeの失敗はError"),
            "{ty}: {error}"
        );
    }
    for (declaration, ty) in [
        ("", "Error"),
        ("", "owned[Error]"),
        ("class Problem:\n    message: str\n", "Problem"),
        ("class i64:\n    message: str\n", "i64"),
    ] {
        let source = format!("{declaration}async def work() -> Result[unit, {ty}]:\n    async with scope:\n        spawn sleep(1)\n    return ok(print(0))\n");
        roundtrip(&source);
    }
}

#[test]
fn spawn_evaluates_owned_arguments_before_parent_continues_and_keeps_sources() {
    let source = r#"def prepare() -> i64:
    print("argument")
    return 0
async def work(value: str):
    print(value)
async def numbers(values: List[i64]):
    assert_true(len(view(values)) == 2)
async def shared_text(value: shared[str]):
    assert_true(True)
async def main() -> Result[unit, Error]:
    text = "Nagi"
    part = view(text)
    values = [1, 2]
    shared = share("Nagi")
    selected = work
    async with scope:
        spawn sleep(prepare())
        print("body")
        spawn selected(copy(part))
        spawn work(copy(view(text)))
        spawn numbers(copy(view(values)))
        spawn shared_text(clone_shared(shared))
        for number in range(2):
            async with scope:
                spawn work(copy(part))
                spawn sleep(number)
    assert_true(len(view(values)) == 2)
    later = clone_shared(shared)
    return ok(print(text))
"#;
    let mut code = roundtrip(source);
    code.push_str(RUNTIME_STUB);
    let output = compile_and_run(code);
    assert!(output.starts_with("argument\nbody\n"), "{output}");
    assert_eq!(output.matches("Nagi\n").count(), 5, "{output}");
}

const RUNTIME_STUB: &str = r#"
extern crate self as nagi_runtime;
#[derive(Debug)]
pub struct Error;
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("error") }
}
pub struct Scope { tasks: Vec<std::pin::Pin<Box<dyn std::future::Future<Output=Result<(), Error>> + Send>>> }
impl Scope {
    pub fn new() -> Self { Self { tasks: vec![] } }
    pub fn spawn(&mut self, task: impl std::future::Future<Output=Result<(), Error>> + Send + 'static) { self.tasks.push(Box::pin(task)); }
    pub async fn join(&mut self) -> Result<(), Error> { for task in self.tasks.drain(..) { task.await?; } Ok(()) }
    pub async fn cancel(&mut self) { self.tasks.clear(); }
}
pub async fn sleep(_: i64) {}
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct Wake;
    impl std::task::Wake for Wake { fn wake(self: std::sync::Arc<Self>) {} }
    let waker = std::task::Waker::from(std::sync::Arc::new(Wake));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) { std::task::Poll::Ready(value) => value, std::task::Poll::Pending => panic!("stub future did not finish") }
}
"#;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn compile_and_run(code: String) -> String {
    let fixture = Fixture(std::env::temp_dir().join(format!(
            "nagi-scoped-tasks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    fs::create_dir_all(&fixture.0).unwrap();
    let source = fixture.0.join("generated.rs");
    let binary = fixture
        .0
        .join(format!("program{}", std::env::consts::EXE_SUFFIX));
    fs::write(&source, code).unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "-D", "unused-imports"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}
