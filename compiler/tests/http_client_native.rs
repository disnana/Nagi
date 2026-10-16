//! Fresh checked High / saved Low without High / handwritten Low emit unedited
//! Rust and link the ordinary runtime. This first case proves native signatures
//! and failure projections, not network acceptance or a successful HTTPS call.
#[path = "support/native_triple.rs"]
mod native_triple;
use native_triple::Fixture;
#[path = "support/http_client_native_runtime.rs"]
mod runtime_fixture;

const HIGH: &str = r#"import std.http.client as client
import std.auth as auth
class State:
    n: i64
enum Permission:
    Allowed
async def protected_call(operation: view[client.ProtectedOperation[State, Permission, bytes]], grant: auth.Grant[Permission], input: bytes) -> Result[client.Response, client.Failure]:
    return await client.execute(operation, grant, input)
async def public_call(operation: view[client.PublicOperation[State, bytes]], input: bytes) -> Result[client.Response, client.Failure]:
    return await client.execute_public(operation, input)
def headers(response: view[client.Response], name: view[str]) -> Result[List[view[bytes]], Error]:
    return client.headers(response, name)
def local_headers(response: view[client.Response]) -> Result[List[view[bytes]], Error]:
    name = "X-DUP"
    values = try client.headers(response, view(name))
    name = "changed"
    return ok(values)
def body(response: view[client.Response]) -> view[bytes]:
    return client.body(response)
def take(response: client.Response) -> bytes:
    return client.take_body(response)
def response_status(response: view[client.Response]) -> i64:
    return client.status(response)
def invalid_input(failure: view[client.Failure]) -> bool:
    return client.kind(failure) == client.FailureKind.INVALID_INPUT and client.outcome(failure) == client.Outcome.REQUEST_NOT_STARTED
def native_message(failure: view[client.Failure]) -> view[str]:
    return client.message(failure)
def auth_failure(failure: view[client.Failure]) -> auth.FailureKind?:
    return client.auth_kind(failure)
def main():
    print(0)
"#;
const LOW: &str = r#"import std.http.client as client
import std.auth as auth
record State { n: i64; }
enum Permission { Allowed; }
async fn protected_call(operation: view[client.ProtectedOperation[State, Permission, bytes]], grant: auth.Grant[Permission], input: bytes) -> Result[client.Response, client.Failure] { return await client.execute(operation, grant, input); }
async fn public_call(operation: view[client.PublicOperation[State, bytes]], input: bytes) -> Result[client.Response, client.Failure] { return await client.execute_public(operation, input); }
fn headers(response: view[client.Response], name: view[str]) -> Result[List[view[bytes]], Error] { return client.headers(response, name); }
fn local_headers(response: view[client.Response]) -> Result[List[view[bytes]], Error] { let name = "X-DUP"; let values = try client.headers(response, view(name)); name = "changed"; return ok(values); }
fn body(response: view[client.Response]) -> view[bytes] { return client.body(response); }
fn take(response: client.Response) -> bytes { return client.take_body(response); }
fn response_status(response: view[client.Response]) -> i64 { return client.status(response); }
fn invalid_input(failure: view[client.Failure]) -> bool { return client.kind(failure) == client.FailureKind.INVALID_INPUT and client.outcome(failure) == client.Outcome.REQUEST_NOT_STARTED; }
fn native_message(failure: view[client.Failure]) -> view[str] { return client.message(failure); }
fn auth_failure(failure: view[client.Failure]) -> auth.FailureKind? { return client.auth_kind(failure); }
fn main() { print(0); }
"#;

#[test]
fn public_signatures_and_failure_projections_build_and_run_in_three_forms() {
    Fixture::new().run_three_with_assertions("http-client-native-contract", HIGH, LOW, "", |program| {
        let symbol = |name| program.modules.definitions.iter().find(|definition| definition.id.name == name).expect("checked definition").symbol.as_str();
        format!(
            "#[test] fn native_projection_oracle() {{ let failure = nagi_runtime::http_client::Failure::invalid_input(); assert!({}(&failure)); assert_eq!({}(&failure), nagi_runtime::http_client::message(&failure)); assert_eq!({}(&failure), None); }}",
            symbol("invalid_input"), symbol("native_message"), symbol("auth_failure")
        )
    });
}

#[test]
fn generated_operations_execute_real_tls_and_grant_in_three_test_network_paths() {
    runtime_fixture::run_three(HIGH, LOW);
}
