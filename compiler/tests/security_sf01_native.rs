//! High, independently loaded saved Low, and handwritten Low use the same live
//! policy dispatcher and request-bound proof implementation.
use nagic::{check, emit, source};
use std::{fs, path::PathBuf, process::Command};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        for n in 0u64.. {
            let p =
                std::env::temp_dir().join(format!("nagi-sf01-native-{}-{n}", std::process::id()));
            match fs::create_dir(&p) {
                Ok(()) => return Self(p),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => panic!("{e}"),
            }
        }
        unreachable!()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
const HIGH: &str = r#"import std.auth as auth
import std.http.server as http
enum Read:
    Allowed
class State:
    value: i64
@rust("native::verify")
extern async def verify(request: http.Request, state: shared[State]) -> Result[auth.VerifiedIdentity, auth.Failure]
@rust("native::authorize")
extern async def authorize(scope: auth.AuthScope, request: http.Request, state: shared[State]) -> Result[auth.Grant[Read], auth.Failure]
@rust("native::read")
extern def read(grant: auth.Grant[Read]) -> Result[i64, auth.Failure]
@rust("native::exercise")
extern async def exercise(app: http.App[State, Error]) -> Result[unit, Error]
async def handler(request: http.Request, state: shared[State], grant: auth.Grant[Read]) -> Result[http.Response, Error]:
    match read(grant):
        case Ok(value):
            return http.json[i64](http.Status.OK, value)
        case Err(problem):
            return internal_error(copy(auth.message(view(problem))))
def setup() -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](State(value=42))
    policy = http.authorized_policy[State, Read](verify, authorize)
    return http.route(app, http.Method.GET, "/answer", policy, handler)
def deny_factory() -> auth.Failure:
    return auth.denied()
async def main() -> Result[unit, Error]:
    factories = [deny_factory]
    duplicates = copy(view(factories))
    factory = duplicates[0]
    problem = factory()
    assert_true(auth.kind(view(problem)) == auth.FailureKind.DENIED)
    app = try setup()
    try await exercise(app)
    return ok(print("SF01 native ready"))
"#;
const LOW: &str = r#"import std.auth as auth;
import std.http.server as http;
enum Read { Allowed; }
record State { value: i64; }
@rust("native::verify")
extern async fn verify(request: http.Request, state: shared[State]) -> Result[auth.VerifiedIdentity, auth.Failure];
@rust("native::authorize")
extern async fn authorize(scope: auth.AuthScope, request: http.Request, state: shared[State]) -> Result[auth.Grant[Read], auth.Failure];
@rust("native::read")
extern fn read(grant: auth.Grant[Read]) -> Result[i64, auth.Failure];
@rust("native::exercise")
extern async fn exercise(app: http.App[State, Error]) -> Result[unit, Error];
async fn handler(request: http.Request, state: shared[State], grant: auth.Grant[Read]) -> Result[http.Response, Error] {
    match read(grant) {
        case Ok(value) { return http.json[i64](http.Status.OK, value); }
        case Err(problem) { return internal_error(copy(auth.message(view(problem)))); }
    }
}
fn setup() -> Result[http.App[State, Error], Error] {
    let app = http.app_default[State](State(value=42));
    let policy = http.authorized_policy[State, Read](verify, authorize);
    return http.route(app, http.Method.GET, "/answer", policy, handler);
}
fn deny_factory() -> auth.Failure { return auth.denied(); }
async fn main() -> Result[unit, Error] {
    let factories = [deny_factory];
    let duplicates = copy(view(factories));
    let factory = duplicates[0];
    let problem = factory();
    assert_true(auth.kind(view(problem)) == auth.FailureKind.DENIED);
    let app = try setup();
    try await exercise(app);
    return ok(print("SF01 native ready"));
}
"#;
const NATIVE: &str = r#"use std::{sync::Arc,time::{Duration,Instant}};
use nagi_runtime::{auth::{AuthScope,VerifiedIdentity,Grant,Failure},http_server as http,Error};
pub async fn verify(request:http::Request,state:Arc<super::State>)->Result<VerifiedIdentity,Failure>{
 assert!(request.body().is_empty(), "verifier sees head only");
 let token=http::header_text(&request,"authorization").map_err(|_|Failure::invalid_request())?;
 if token!=Some("Bearer fixture"){return Err(Failure::invalid_credential());}
 VerifiedIdentity::from_verified(state.value,Instant::now()+Duration::from_secs(2))
}
pub async fn authorize(scope:AuthScope,request:http::Request,_:Arc<super::State>)->Result<Grant<super::Read>,Failure>{
 assert!(request.body().is_empty());
 Grant::from_authorized(scope,9)
}
pub fn read(grant:Grant<super::Read>)->Result<i64,Failure>{grant.submit((),|subject,target,()|subject+target)}
pub async fn exercise(app:http::App<super::State,Error>)->Result<(),Error>{
 use tokio::{net::{TcpListener,TcpStream},io::{AsyncReadExt,AsyncWriteExt},sync::oneshot};
 let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let address=listener.local_addr().unwrap();
 let (stop,done)=oneshot::channel();let task=tokio::spawn(http::serve_listener(listener,app,http::default_options(),async{let _=done.await;}));
 for (credentials,status,body) in [("Authorization: Bearer fixture\r\n",200,"51"),("",401,"invalid credential"),("Authorization: Bearer rejected\r\n",401,"invalid credential")] {
  let mut stream=TcpStream::connect(address).await.unwrap();let request=format!("GET /answer HTTP/1.1\r\nHost: localhost\r\n{credentials}Connection: close\r\n\r\n");stream.write_all(request.as_bytes()).await.unwrap();
  let mut output=String::new();tokio::time::timeout(Duration::from_secs(3),stream.read_to_string(&mut output)).await.unwrap().unwrap();
  assert!(output.starts_with(&format!("HTTP/1.1 {status}")),"{output}");assert!(output.ends_with(body),"{output}");assert!(output.to_lowercase().contains("x-content-type-options: nosniff"),"{output}");
 }
 stop.send(()).unwrap();task.await.unwrap()?;Ok(())
}
"#;
#[test]
fn policies_and_nominal_request_proofs_execute_in_all_three_paths() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("nagi.toml"), "entry = \"main.nagi\"\n[rust.dependencies]\ntokio = { version = \"1.48\", features = [\"net\", \"io-util\", \"sync\", \"time\", \"rt\"] }\n").unwrap();
    fs::write(fixture.0.join("native.rs"), NATIVE).unwrap();
    let high = fixture.0.join("source.nagi");
    fs::write(&high, HIGH).unwrap();
    let mut loaded = source::load(&high, true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let saved = emit::low(&loaded.program);
    for (name, text, is_high) in [
        ("main.nagi", HIGH.to_owned(), true),
        ("saved.low", saved, false),
        ("manual.low", LOW.to_owned(), false),
    ] {
        let path = fixture.0.join(name);
        fs::write(&path, text).unwrap();
        let mut loaded = source::load(&path, is_high).unwrap_or_else(|e| panic!("{name}: {e}"));
        check::check(&mut loaded.program)
            .unwrap_or_else(|e| panic!("{name}: {}", loaded.diagnostic(&e)));
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .arg("run")
            .arg(&path)
            .arg("--project")
            .arg(&fixture.0)
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
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "SF01 native ready"
        );
    }
}
