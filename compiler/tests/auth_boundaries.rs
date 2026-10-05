#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{ast::Program, check, emit, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-auth-boundary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn checked(&self, text: &str, high: bool) -> Result<Program, String> {
        let path = self.0.join(if high { "main.nagi" } else { "main.low" });
        fs::write(&path, text).unwrap();
        let mut sources = source::load(&path, high)?;
        check::check(&mut sources.program).map_err(|error| sources.diagnostic(&error))?;
        Ok(sources.program)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const DECLARATIONS: &str = "import std.auth as auth\nenum Read:\n    Permission\nenum Write:\n    Permission\n@rust(\"native::principal\")\nextern def principal(subject: i64) -> auth.Principal\n@rust(\"native::grant\")\nextern def grant(principal: view[auth.Principal], resource: i64) -> auth.Grant[Read]\n@rust(\"native::write_grant\")\nextern def write_grant(principal: view[auth.Principal], resource: i64) -> auth.Grant[Write]\n@rust(\"native::read\")\nextern def read(grant: auth.Grant[Read]) -> i64\n";

fn rejects(body: &str, reason: &str) {
    let fixture = Fixture::new();
    let text = format!("{DECLARATIONS}{body}");
    // Low is independently loaded, preserving only source metadata rather than
    // copying checker ownership or capability facts from High.
    let path = fixture.0.join("untyped.nagi");
    fs::write(&path, &text).unwrap();
    let loaded = source::load(&path, true).unwrap();
    let low = emit::low(&loaded.program);
    for (text, high) in [(&text, true), (&low, false)] {
        let error = fixture.checked(text, high).expect_err(text);
        assert!(error.contains(reason), "{text}\n{error}");
        assert!(
            error.contains(if high { "main.nagi:" } else { "main.low:" }),
            "{error}"
        );
    }
}

#[test]
fn missing_principal_grant_wrong_permission_and_reuse_are_rejected() {
    rejects("def main():\n    read()\n", "引数");
    rejects("def main():\n    grant(9)\n", "引数");
    rejects("def main():\n    grant(42, 9)\n", "型");
    rejects(
        "def main():\n    p = principal(42)\n    g = write_grant(view(p), 9)\n    read(g)\n",
        "型",
    );
    rejects(
        "def main():\n    p = principal(42)\n    g = grant(view(p), 9)\n    read(g)\n    read(g)\n",
        "move",
    );
    rejects(
        "class Principal:\n    subject: i64\ndef main():\n    grant(Principal(subject=42), 9)\n",
        "型",
    );
}

#[test]
fn indirect_arc_state_cannot_share_a_proof() {
    rejects("import std.http.server as http\ndef main():\n    app = http.app_default[auth.Principal](principal(42))\n", "shared");
    rejects("import std.actor as actor\ndef main():\n    group = actor.supervisor[Option[auth.Principal]](some(principal(42)), actor.default_options())\n", "shared");
    rejects("import std.http.server as http\ndef invalid(app: http.App[auth.Principal, Error]):\n    print(0)\n", "shared");
}

#[test]
fn a_function_signature_and_a_deep_dto_do_not_contain_a_proof_payload() {
    let fixture = Fixture::new();
    let mut text = format!("{DECLARATIONS}def protected(value: auth.Grant[Read]) -> i64:\n    return read(value)\ndef main():\n    callback = share(protected)\n");
    for index in 0..70 {
        text.push_str(&format!("class C{index}:\n    next: C{}\n", index + 1));
    }
    text.push_str("class C70:\n    value: i64\ndef publish(value: C0) -> shared[C0]:\n    return share(value)\n");
    let checked = fixture.checked(&text, true).unwrap();
    fixture.checked(&emit::low(&checked), false).unwrap();
}

#[test]
fn native_phantom_types_do_not_become_field_payloads() {
    let fixture = Fixture::new();
    let text = r#"import std.auth as auth
import std.http.server as http
enum Read:
    Permission
class Holder:
    principal_app: http.App[i64, auth.Principal]
    grant_app: http.App[i64, auth.Grant[Read]]
    callback_app: http.App[i64, fn[auth.Grant[Read], i64]]
@rust("native::verify")
extern def verify(value: Holder) -> unit
def principal_error(proof: auth.Principal) -> http.Response:
    return http.empty(http.Status.UNAUTHORIZED)
def grant_error(proof: auth.Grant[Read]) -> http.Response:
    return http.empty(http.Status.FORBIDDEN)
def callback_error(callback: fn[auth.Grant[Read], i64]) -> http.Response:
    return http.empty(http.Status.INTERNAL_SERVER_ERROR)
def main():
    value = Holder(principal_app=http.app[i64, auth.Principal](1, principal_error), grant_app=http.app[i64, auth.Grant[Read]](2, grant_error), callback_app=http.app[i64, fn[auth.Grant[Read], i64]](3, callback_error))
    verify(value)
"#;
    let high = fixture.checked(text, true).unwrap();
    let low = emit::low(&high);
    fixture.checked(&low, false).unwrap();
    // Exercise the registered native representation and its custom Debug,
    // rather than only asserting that the generated text mentions these types.
    fs::write(
        fixture.0.join("native.rs"),
        "pub fn verify(value: super::Holder) {\n    let _: nagi_runtime::http_server::App<i64, nagi_runtime::auth::Principal> = value.principal_app;\n    let _: nagi_runtime::http_server::App<i64, nagi_runtime::auth::Grant<super::Read>> = value.grant_app;\n    let _: nagi_runtime::http_server::App<i64, fn(nagi_runtime::auth::Grant<super::Read>) -> i64> = value.callback_app;\n}\n",
    )
    .unwrap();
    for (name, text) in [("main.nagi", text.to_owned()), ("saved.low", low)] {
        let path = fixture.0.join(name);
        fs::write(&path, text).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .arg("build")
            .arg(&path)
            .arg("--no-project")
            .arg("--rust")
            .arg(fixture.0.join("native.rs"))
            .arg("--out")
            .arg(fixture.0.join(format!("build-{name}")))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // The state/context is an actual retained Arc payload. Turn's state is
    // inline, so it must still obey the owned-field storage restriction.
    rejects("import std.http.server as http\nclass Invalid:\n    app: http.App[auth.Principal, Error]\n", "shared");
    rejects("import std.actor as actor\nclass Invalid:\n    group: actor.Supervisor[Option[auth.Grant[Read]]]\n", "shared");
    rejects("import std.actor as actor\nclass Invalid:\n    turn: actor.Turn[Option[auth.Principal], i64, Error]\n", "field");
}

#[test]
fn opaque_main_errors_fail_without_a_debug_bound_or_proof_payload() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("native.rs"), "pub fn principal(subject:i64)->nagi_runtime::auth::Principal { nagi_runtime::auth::Principal::from_verified_subject(subject) }\npub fn grant(principal:&nagi_runtime::auth::Principal, resource:i64)->nagi_runtime::auth::Grant<super::Read> { nagi_runtime::auth::Grant::from_authorized(principal,resource) }\npub fn write_grant(principal:&nagi_runtime::auth::Principal, resource:i64)->nagi_runtime::auth::Grant<super::Write> { nagi_runtime::auth::Grant::from_authorized(principal,resource) }\npub fn read(grant:nagi_runtime::auth::Grant<super::Read>)->i64 { let (subject,resource)=grant.into_authorized_parts(); subject+resource }\n").unwrap();
    for (index, (error_type, value)) in [
        ("auth.Principal", "principal(918273)"),
        ("Option[auth.Principal]", "some(principal(918273))"),
        ("auth.Grant[Read]", "grant(view(p), 876543)"),
    ]
    .iter()
    .enumerate()
    {
        let text = format!("{DECLARATIONS}def main() -> Result[unit, {error_type}]:\n    p = principal(918273)\n    return fail({value})\n");
        let high = fixture.checked(&text, true).unwrap();
        for (name, source) in [
            (format!("main-{index}.nagi"), text),
            (format!("saved-{index}.low"), emit::low(&high)),
        ] {
            let path = fixture.0.join(&name);
            fs::write(&path, source).unwrap();
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .arg("run")
                .arg(&path)
                .arg("--no-project")
                .arg("--rust")
                .arg(fixture.0.join("native.rs"))
                .arg("--out")
                .arg(fixture.0.join(format!("build-{name}")))
                .output()
                .unwrap();
            let error = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{name}: {error}");
            assert!(
                error.contains("NagiのmainがErrを返しました"),
                "{name}: {error}"
            );
            assert!(
                !error.contains("918273") && !error.contains("876543"),
                "{name}: {error}"
            );
            assert!(
                !error.contains("E0277") && !error.contains("Build failed"),
                "{name}: {error}"
            );
        }
    }
}

#[test]
fn opaque_proofs_cannot_be_constructed_decoded_copied_or_shared() {
    rejects("def main():\n    auth.Principal()\n", "resource");
    rejects("def main():\n    auth.Grant[Read]()\n", "resource");
    rejects(
        "def main():\n    value = json_decode[auth.Principal](\"{}\")\n",
        "JSON",
    );
    rejects(
        "def main():\n    value = json_decode[Option[auth.Grant[Read]]](\"null\")\n",
        "JSON",
    );
    rejects(
        "def main():\n    p = principal(1)\n    copy(view(p))\n",
        "Copy",
    );
    rejects(
        "def main():\n    p = principal(1)\n    share(p)\n",
        "shared",
    );
    rejects(
        "def main():\n    p = principal(1)\n    share(some(p))\n",
        "shared",
    );
    rejects(
        "def invalid(value: shared[Option[auth.Principal]]):\n    print(0)\n",
        "shared",
    );
    rejects(
        "class Box:\n    proof: Option[auth.Grant[Read]]\ndef main():\n    print(0)\n",
        "field",
    );
    rejects(
        "enum Box:\n    Proof(value: auth.Principal)\ndef main():\n    print(0)\n",
        "field",
    );
    rejects(
        "def invalid(value: auth.Grant[i64]):\n    print(0)\n",
        "marker",
    );
    rejects(
        "def invalid(value: auth.Grant[List[Read]]):\n    print(0)\n",
        "marker",
    );
}

#[test]
fn aliases_option_result_and_async_delegation_preserve_proof_identity() {
    let fixture = Fixture::new();
    let text = format!("{DECLARATIONS}from std.auth import Principal as Identity, Grant as Permit\nasync def delegate(value: Permit[Read]) -> i64:\n    await sleep(1)\n    return read(value)\nasync def main():\n    p: Identity = principal(42)\n    g: Option[Permit[Read]] = some(grant(view(p), 9))\n    match g:\n        case Some(proof):\n            print(await delegate(proof))\n        case None:\n            print(0)\n");
    let high = fixture.checked(&text, true).unwrap();
    let low = fixture.checked(&emit::low(&high), false).unwrap();
    assert_eq!(
        emit::rust(&checked_emission::seal(&high)).unwrap(),
        emit::rust(&checked_emission::seal(&low)).unwrap()
    );
    let rust = emit::rust(&checked_emission::seal(&high)).unwrap();
    assert!(rust.contains("::nagi_runtime::auth::Grant<"), "{rust}");
    assert!(!rust.contains("pub struct Principal"), "{rust}");
}

#[test]
fn handwritten_low_and_owned_local_containers_keep_nominal_marker_identity() {
    let fixture = Fixture::new();
    let low = "import std.auth as auth;\nenum Read { Permission; }\n@rust(\"native::principal\")\nextern fn principal(subject: i64) -> auth.Principal;\n@rust(\"native::grant\")\nextern fn grant(principal: view[auth.Principal], resource: i64) -> auth.Grant[Read];\nfn main() -> unit { let p: auth.Principal = principal(1); let grants: List[auth.Grant[Read]] = [grant(view(p), 9)]; print(len(grants)); }\n";
    let checked = fixture.checked(low, false).unwrap();
    assert!(emit::rust(&checked_emission::seal(&checked))
        .unwrap()
        .contains("::nagi_runtime::auth::Grant<"));
    for name in ["reader.nagi", "writer.nagi"] {
        fs::write(fixture.0.join(name), "enum Permission:\n    Allowed\n").unwrap();
    }
    let text = "import std.auth as auth\nimport \"reader.nagi\" as reader\nimport \"writer.nagi\" as writer\n@rust(\"native::issue\")\nextern def issue() -> auth.Grant[reader.Permission]\n@rust(\"native::consume\")\nextern def consume(grant: auth.Grant[writer.Permission]) -> unit\ndef main():\n    consume(issue())\n";
    let error = fixture.checked(text, true).unwrap_err();
    assert!(error.contains("型"), "{error}");
}

#[test]
fn proof_move_and_nominal_marker_build_and_run_against_real_runtime() {
    let fixture = Fixture::new();
    let text = format!("{DECLARATIONS}async def delegate(value: auth.Grant[Read]) -> i64:\n    await sleep(1)\n    return read(value)\nasync def main():\n    p = principal(42)\n    g = grant(view(p), 9)\n    print(await delegate(g))\n    grants = [grant(view(p), 9)]\n    print(len(grants))\n");
    let high = fixture.checked(&text, true).unwrap();
    let low = emit::low(&high);
    fs::write(fixture.0.join("native.rs"), "pub fn principal(subject:i64)->nagi_runtime::auth::Principal { nagi_runtime::auth::Principal::from_verified_subject(subject) }\npub fn grant(principal:&nagi_runtime::auth::Principal, resource:i64)->nagi_runtime::auth::Grant<super::Read> { nagi_runtime::auth::Grant::from_authorized(principal,resource) }\npub fn write_grant(principal:&nagi_runtime::auth::Principal, resource:i64)->nagi_runtime::auth::Grant<super::Write> { nagi_runtime::auth::Grant::from_authorized(principal,resource) }\npub fn read(grant:nagi_runtime::auth::Grant<super::Read>)->i64 { let (subject,resource)=grant.into_authorized_parts(); subject+resource }\n").unwrap();
    for (name, text) in [("main.nagi", text), ("saved.low", low)] {
        let path = fixture.0.join(name);
        fs::write(&path, text).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .arg("run")
            .arg(&path)
            .arg("--no-project")
            .arg("--rust")
            .arg(fixture.0.join("native.rs"))
            .arg("--out")
            .arg(fixture.0.join(format!("build-{name}")))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "51\n1");
    }
}
