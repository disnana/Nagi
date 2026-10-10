#[path = "support/native_triple.rs"]
mod native_triple;
use native_triple::Fixture;
const HIGH: &str = r#"import std.auth as auth
import std.http.server as http
import std.db.sqlite as sqlite
enum Edit:
    Allowed
class State:
    pool: sqlite.Pool
    subject: i64
    target: i64
class ValueRow:
    value: i64
class QueryHolder:
    query: sqlite.Query
@rust("native::verify")
extern async def verify(request: http.Request, state: shared[State]) -> Result[auth.VerifiedIdentity, auth.Failure]
@rust("native::authorize")
extern async def authorize(scope: auth.AuthScope, request: http.Request, state: shared[State]) -> Result[auth.Grant[Edit], auth.Failure]
@rust("native::protected_update")
extern async def protected_update(pool: view[sqlite.Pool], grant: auth.Grant[Edit], value: i64) -> Result[i64, Error]
async def handler(request: http.Request, state: shared[State], grant: auth.Grant[Edit]) -> Result[http.Response, Error]:
    changed = try await protected_update(view(state.pool), grant, 101)
    return http.json[i64](http.Status.OK, changed)
def setup(pool: sqlite.Pool, subject: i64, target: i64) -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](State(pool=pool, subject=subject, target=target))
    policy = http.authorized_policy[State, Edit](verify, authorize)
    return http.route(app, http.Method.POST, "/update", policy, handler)
def selected_query(id: i64) -> sqlite.Query:
    if id == 9:
        return sqlite.literal("SELECT value FROM protected WHERE id=?")
    return sqlite.literal("SELECT value FROM protected WHERE id=? ORDER BY id")
def make_holder(query: sqlite.Query) -> shared[QueryHolder]:
    return share(QueryHolder(query=query))
async def read_value(tx: view[sqlite.Tx], id: i64) -> Result[ValueRow?, sqlite.Failure]:
    query = selected_query(id)
    return await sqlite.query[ValueRow](tx, query, sqlite.bind_i64(sqlite.parameters(), id))
"#;
const LOW: &str = r#"import std.auth as auth;
import std.http.server as http;
import std.db.sqlite as sqlite;
enum Edit { Allowed; }
record State { pool: sqlite.Pool; subject: i64; target: i64; }
record ValueRow { value: i64; }
record QueryHolder { query: sqlite.Query; }
@rust("native::verify")
extern async fn verify(request: http.Request, state: shared[State]) -> Result[auth.VerifiedIdentity, auth.Failure];
@rust("native::authorize")
extern async fn authorize(scope: auth.AuthScope, request: http.Request, state: shared[State]) -> Result[auth.Grant[Edit], auth.Failure];
@rust("native::protected_update")
extern async fn protected_update(pool: view[sqlite.Pool], grant: auth.Grant[Edit], value: i64) -> Result[i64, Error];
async fn handler(request: http.Request, state: shared[State], grant: auth.Grant[Edit]) -> Result[http.Response, Error] {
    let changed = try await protected_update(view(state.pool), grant, 101);
    return http.json[i64](http.Status.OK, changed);
}
fn setup(pool: sqlite.Pool, subject: i64, target: i64) -> Result[http.App[State, Error], Error] {
    let app = http.app_default[State](State(pool=pool, subject=subject, target=target));
    let policy = http.authorized_policy[State, Edit](verify, authorize);
    return http.route(app, http.Method.POST, "/update", policy, handler);
}
fn selected_query(id: i64) -> sqlite.Query {
    if id == 9 { return sqlite.literal("SELECT value FROM protected WHERE id=?"); }
    return sqlite.literal("SELECT value FROM protected WHERE id=? ORDER BY id");
}
fn make_holder(query: sqlite.Query) -> shared[QueryHolder] { return share(QueryHolder(query=query)); }
async fn read_value(tx: view[sqlite.Tx], id: i64) -> Result[ValueRow?, sqlite.Failure] {
    let query = selected_query(id);
    return await sqlite.query[ValueRow](tx, query, sqlite.bind_i64(sqlite.parameters(), id));
}
"#;
const NATIVE: &str = r#"
mod native {
    use nagi_runtime::{auth::{AuthScope,VerifiedIdentity,Grant,Failure},http_server as http,sqlite,Error};
    use std::{sync::Arc,time::{Duration,Instant}};
    pub async fn verify(request:http::Request,state:Arc<super::State>)->Result<VerifiedIdentity,Failure>{
        assert!(request.body().is_empty());
        if http::header_text(&request,"authorization").map_err(|_|Failure::invalid_request())? != Some("Bearer fixture") { return Err(Failure::invalid_credential()); }
        VerifiedIdentity::from_verified(state.subject,Instant::now()+Duration::from_secs(5))
    }
    pub async fn authorize(scope:AuthScope,request:http::Request,state:Arc<super::State>)->Result<Grant<super::Edit>,Failure>{
        assert!(request.body().is_empty()); Grant::from_authorized(scope,state.target)
    }
    // Trusted, reviewed adapter: fixed statement and both supplied real proof
    // values go into the predicate; generic SQL does not prove tenant isolation.
    pub async fn protected_update(pool:&sqlite::Pool,grant:Grant<super::Edit>,value:i64)->Result<i64,Error>{
        let tx=sqlite::begin(pool,sqlite::BeginMode::Immediate).await.map_err(|e|Error::internal(e.to_string()))?;
        let reservation=tx.reserve_exec().await.map_err(|e|Error::internal(e.to_string()))?;
        let reply=grant.submit(reservation,|subject,target,res|res.enqueue(
            sqlite::literal("UPDATE protected SET value=? WHERE owner=? AND id=?"),
            sqlite::bind_i64(sqlite::bind_i64(sqlite::bind_i64(sqlite::parameters(),value),subject),target)))
            .map_err(|e|Error::internal(e.to_string()))?;
        let result=reply.await;
        match result {
            Ok(count)=>{sqlite::commit(tx).await.map_err(|e|Error::internal(e.to_string()))?;Ok(count)}
            Err(e)=>{sqlite::rollback(tx).await.map_err(|e|Error::internal(e.to_string()))?;Err(Error::internal(e.to_string()))}
        }
    }
}
"#;
const ASSERTIONS: &str = r#"
#[test]
fn real_sqlite_and_request_bound_predicate() {
    use nagi_runtime::{sqlite,http_server as http};
    use tokio::{net::{TcpListener,TcpStream},io::{AsyncReadExt,AsyncWriteExt},sync::oneshot};
    use std::time::Duration;
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let query = selected_query(9);
        let holder = make_holder(query);
        assert_eq!(format!("{:?}", holder.query), "Query { .. }");
        let debug = format!("{:?}", holder);
        assert!(debug.contains("Query { .. }"));
        assert!(!debug.contains("SELECT") && !debug.contains("protected"));
        // Query is copied, stored, shared, returned and selected by Nagi;
        // the direct literal restriction does not constrain these value uses.
        let second_holder = make_holder(query);
        assert_eq!(format!("{:?}", second_holder.query), "Query { .. }");
        let pool=sqlite::open(":memory:",sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        // Administrative fixture bootstrap with reviewed fixed SQL, outside a
        // request; no dynamic SQL factory or request-facing management export.
        let tx=sqlite::begin(&pool,sqlite::BeginMode::Immediate).await.unwrap();
        sqlite::exec(&tx,sqlite::literal("CREATE TABLE protected(id INTEGER PRIMARY KEY,owner INTEGER,value INTEGER)"),sqlite::parameters()).await.unwrap();
        sqlite::exec(&tx,sqlite::literal("INSERT INTO protected VALUES(9,7,100),(10,8,200)"),sqlite::parameters()).await.unwrap();
        sqlite::commit(tx).await.unwrap();
        for (subject,target,body) in [(7,9,"1"),(8,9,"0"),(7,10,"0")] {
            let app=setup(sqlite::clone_pool(&pool),subject,target).unwrap();
            let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let address=listener.local_addr().unwrap();
            let (stop,done)=oneshot::channel();
            let task=tokio::spawn(http::serve_listener(listener,app,http::default_options(),async{let _=done.await;}));
            for (credentials,status,expected) in [("Authorization: Bearer fixture\r\n",200,body),("",401,"invalid credential")] {
                let mut stream=TcpStream::connect(address).await.unwrap();
                stream.write_all(format!("POST /update HTTP/1.1\r\nHost: localhost\r\n{credentials}Content-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
                let mut output=String::new();tokio::time::timeout(Duration::from_secs(3),stream.read_to_string(&mut output)).await.unwrap().unwrap();
                assert!(output.starts_with(&format!("HTTP/1.1 {status}")),"{output}");assert!(output.ends_with(expected),"{output}");
            }
            stop.send(()).unwrap();task.await.unwrap().unwrap();
            let tx=sqlite::begin(&pool,sqlite::BeginMode::Deferred).await.unwrap();
            assert_eq!(read_value(&tx,9).await.unwrap().unwrap().value,101);
            assert_eq!(read_value(&tx,10).await.unwrap().unwrap().value,200);
            sqlite::rollback(tx).await.unwrap();
        }
        sqlite::close(&pool,2000).await.unwrap();
    });
}
"#;
#[test]
fn query_parameters_and_protected_predicate_execute_high_saved_and_handwritten_low() {
    Fixture::new().run_three("sf05-native", HIGH, LOW, NATIVE, ASSERTIONS);
}
