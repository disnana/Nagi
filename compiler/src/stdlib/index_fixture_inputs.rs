// Inputs copied byte-for-byte from the immutable existing editor fixtures.
const FIXTURES: &[(&str, bool, &str)] = &[
    (
        "actor_nagi",
        true,
        r###"import std.actor as actors
import std.http.server as http
from std.actor import Actor as Handle, Turn as Step, Options as ActorConfig, Event as Update
from std.http.server import Options as HttpConfig
class Counter:
    value: i64
enum ReplyError:
    Negative
def choose(label: view[str], kind: actors.CallKind) -> bool:
    print(label)
    return kind == actors.CallKind.MAILBOX_FULL
def inspect(event: Update) -> i64:
    print(event.child_name)
    print(event.message)
    return event.generation
async def call_once(handle: Handle[i64, i64, ReplyError]) -> Result[Result[i64, ReplyError], actors.CallError]:
    return await actors.call(view(handle), 1, 100, 1000)
def finish(state: Counter) -> Step[Counter, i64, ReplyError]:
    return actors.turn[Counter, i64, ReplyError](state, ok(3))
def actor_limits() -> ActorConfig:
    return actors.default_options()
def http_limits() -> HttpConfig:
    return http.default_options()
def main():
    print(choose(view("😀"), actors.CallKind.MAILBOX_FULL))
"###,
    ),
    (
        "actor_low",
        false,
        r###"import std.actor as actors;
import std.http.server as http;
from std.actor import Actor as Handle, Turn as Step, Options as ActorConfig, Event as Update;
from std.http.server import Options as HttpConfig;
record Counter { value: i64; }
enum ReplyError { Negative; }
fn choose(label: view[str], kind: actors.CallKind) -> bool { print(label); return kind == actors.CallKind.MAILBOX_FULL; }
fn inspect(event: Update) -> i64 { print(event.child_name); print(event.message); return event.generation; }
async fn call_once(handle: Handle[i64, i64, ReplyError]) -> Result[Result[i64, ReplyError], actors.CallError] { return await actors.call(view(handle), 1, 100, 1000); }
fn finish(state: Counter) -> Step[Counter, i64, ReplyError] { return actors.turn[Counter, i64, ReplyError](state, ok(3)); }
fn actor_limits() -> ActorConfig { return actors.default_options(); }
fn http_limits() -> HttpConfig { return http.default_options(); }
fn main() { print(choose(view("😀"), actors.CallKind.MAILBOX_FULL)); }
"###,
    ),
    (
        "stdlib_nagi",
        true,
        r###"import std.http.server as http
from std.http.server import Request as Incoming, Status as Code
def label(prefix: view[str], status: Code) -> i64:
    print(prefix)
    return status.value
def handle(request: Incoming) -> Result[http.Response, Error]:
    json_content = try http.is_json_content_type(view(request))
    print(json_content)
    optional = try http.header_text(view(request), view("authorization"))
    match optional:
        case Some(token):
            print(token)
        case None:
            print("missing")
    if request.is_get:
        print(request.path)
    return ok(http.text(Code.OK, view("hello")))
def main():
    print(label(view("😀"), Code.UNAUTHORIZED))
"###,
    ),
    (
        "stdlib_low",
        false,
        r###"import std.http.server as http;
from std.http.server import Request as Incoming, Status as Code;
fn label(prefix: view[str], status: Code) -> i64 { print(prefix); return status.value; }
fn handle(request: Incoming) -> Result[http.Response, Error] {
    let json_content = try http.is_json_content_type(view(request));
    print(json_content);
    let optional = try http.header_text(view(request), view("authorization"));
    match optional { case Some(token) { print(token); } case None { print("missing"); } }
    if request.is_get { print(request.path); }
    return ok(http.text(Code.OK, view("hello")));
}
fn main() { print(label(view("😀"), Code.UNAUTHORIZED)); }
"###,
    ),
];
