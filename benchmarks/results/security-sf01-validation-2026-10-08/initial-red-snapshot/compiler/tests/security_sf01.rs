use nagic::{check, emit, source};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir();
        let path = (0u64..).find_map(|n| {
            let p = base.join(format!("nagi-sf01-{}-{n}", std::process::id()));
            match fs::create_dir(&p) {
                Ok(()) => Some(p),
                Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists => None,
                Err(e) => panic!("{e}"),
            }
        }).unwrap();
        Self(path)
    }
    fn reject(&self, text:&str, expected:&str, line:usize) {
        let high=self.0.join("source.nagi"); fs::write(&high,text).unwrap();
        // Loading must succeed before the purpose-specific checker rejection.
        let loaded=source::load(&high,true).unwrap();
        let low=emit::low(&loaded.program);
        for (name,text,high) in [("source.nagi",text.to_owned(),true),("saved.low",low,false)] {
            let path=self.0.join(name); fs::write(&path,&text).unwrap();
            let mut loaded=source::load(&path,high).unwrap();
            let error=check::check(&mut loaded.program).expect_err("legacy security entry accepted");
            assert!(error.contains(expected),"{error}");
            let d=loaded.diagnostic(&error);
            assert!(d.contains(name),"{d}");
            if high {assert!(d.contains(&format!(":{line}\n")),"{d}");}
            assert!(!d.contains("internal compiler error"),"{d}");
        }
    }
}
impl Drop for Fixture { fn drop(&mut self){fs::remove_dir_all(&self.0).unwrap();} }
#[test]
fn legacy_decorator_is_a_checker_migration_diagnostic() {
 Fixture::new().reject("@get(\"/answer\")\nasync def health() -> Result[i64, Error]:\n    return ok(7)\n", "SF01 migration",2);
}
#[test]
fn legacy_global_serve_is_a_checker_migration_diagnostic() {
 Fixture::new().reject("async def main() -> Result[unit, Error]:\n    db = try await db_open(\":memory:\")\n    try await serve(db, 0)\n    return ok(print(0))\n", "SF01 migration",3);
}
#[test]
fn legacy_principal_is_a_checker_migration_diagnostic() {
 Fixture::new().reject("import std.auth as auth\n@rust(\"native::old\")\nextern def old() -> auth.Principal\n", "SF01 migration",3);
}
#[test]
fn policy_free_route_is_a_checker_migration_diagnostic() {
 Fixture::new().reject("import std.http.server as http\nclass State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State]) -> Result[http.Response, Error]:\n    return ok(http.empty(http.Status.OK))\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=7))\n    return http.route(app, http.Method.GET, \"/\", handler)\n", "SF01 migration",8);
}
