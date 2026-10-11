//! CORS stage oracles and unmodified generated Rust in three independent forms.
#[path = "support/native_triple.rs"]
mod native_triple;
use nagic::{check, emit, source};
use std::fs;

const NATIVE: &str = r#"
#[test]
fn actual_owned_cors_registration_and_wire_metadata() {
    assert_eq!(user_value(),9);
    let runtime=::tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let app=build("/owned/{id}").unwrap();
        let bad=::nagi_runtime::http_server::cors_policy(vec!["*".to_owned()],vec![],::nagi_runtime::http_server::CorsCredentials::ALLOW,0,1,64,1,16);
        assert!(bad.is_err());
        let listener=::tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address=listener.local_addr().unwrap();
        let options=::nagi_runtime::http_server::authority(::nagi_runtime::http_server::default_options(),"https://localhost",vec!["localhost".to_owned()],1,128).unwrap();
        let (stop,shutdown)=::tokio::sync::oneshot::channel();
        let mut task=::tokio::spawn(::nagi_runtime::http_server::serve_listener(listener,app,options,async {let _=shutdown.await;}));
        for (method,extra,expected) in [("OPTIONS","Access-Control-Request-Method: GET\r\nAccess-Control-Request-Headers: content-type\r\n",204),("GET","",200)] {
            let text=::tokio::task::spawn_blocking(move || {
                use ::std::io::{Read,Write};
                let mut stream=::std::net::TcpStream::connect_timeout(&address,::std::time::Duration::from_secs(5)).unwrap();
                stream.set_read_timeout(Some(::std::time::Duration::from_secs(5))).unwrap();
                stream.set_write_timeout(Some(::std::time::Duration::from_secs(5))).unwrap();
                let request=format!("{method} /owned/7 HTTP/1.1\r\nHost: localhost\r\nOrigin: https://app.example.invalid\r\nConnection: close\r\nContent-Length: 0\r\n{extra}\r\n");
                stream.write_all(request.as_bytes()).unwrap();
                let mut bytes=Vec::new();stream.take(4097).read_to_end(&mut bytes).unwrap();assert!(bytes.len()<=4096);
                String::from_utf8(bytes).unwrap()
            }).await.unwrap();
            let (head,body)=text.split_once("\r\n\r\n").unwrap();
            let status=head.lines().next().unwrap().split_whitespace().nth(1).unwrap().parse::<u16>().unwrap();
            assert_eq!(status,expected);
            let headers=head.split("\r\n").skip(1).map(|line| {let (name,value)=line.split_once(':').unwrap();(name.to_ascii_lowercase(),value.trim())}).collect::<Vec<_>>();
            let values=|name:&str|headers.iter().filter(|(key,_)|key==name).map(|(_,value)|*value).collect::<Vec<_>>();
            assert_eq!(values("access-control-allow-origin"),vec!["https://app.example.invalid"]);
            assert_eq!(values("access-control-allow-credentials"),vec!["true"]);
            if expected==204 {assert!(body.is_empty());assert_eq!(values("access-control-max-age"),vec!["0"]);}
            else {assert_eq!(body,"owned");}
        }
        stop.send(()).unwrap();
        ::tokio::time::timeout(::std::time::Duration::from_secs(5),&mut task).await.unwrap().unwrap().unwrap();
    });
}
"#;
fn native(case: &str, high: &str, low: &str) {
    native_triple::Fixture::new().run_three_with_assertions(case, high, low, "", |program| {
        let mut assertions = NATIVE.to_owned();
        for name in ["build", "user_value"] {
            let symbol = program
                .modules
                .resolve_root_path(name)
                .unwrap()
                .symbol
                .clone();
            assertions = assertions.replace(&format!("{name}("), &format!("{symbol}("));
        }
        assertions
    });
}
#[test]
fn cors_dynamic_registration_and_owned_socket_are_native_in_three_forms() {
    native(
        "sf03-cors-config",
        include_str!("fixtures/security-sf03-cors/config.nagi"),
        include_str!("fixtures/security-sf03-cors/config-manual.low"),
    );
}
#[test]
fn aliases_and_user_shadow_type_keep_canonical_identity_in_three_forms() {
    let high = include_str!("fixtures/security-sf03-cors/config.nagi")
        .replace("as http", "as web")
        .replace("http.", "web.");
    let low = include_str!("fixtures/security-sf03-cors/config-manual.low")
        .replace("as http", "as web")
        .replace("http.", "web.");
    native("sf03-cors-alias", &high, &low);
}
fn reject_three(
    case: &str,
    high: &str,
    high_line: usize,
    manual: &str,
    manual_line: usize,
    reason: &str,
) {
    let fixture = native_triple::Fixture::new();
    fixture.write("bad.nagi", high);
    let mut parsed = source::load(&fixture.0.join("bad.nagi"), true)
        .expect("negative must parse/name resolve before checker");
    let low = emit::low_with_lines(&parsed.program);
    let low_line = (1..=low.text.lines().count())
        .find(|&line| {
            low.line_origin(line).is_some_and(|global| {
                let origin = parsed.provenance().origin(global);
                origin.line == Some(high_line)
                    && origin
                        .path
                        .as_ref()
                        .is_some_and(|path| path.ends_with("bad.nagi"))
            })
        })
        .expect("negative origin must survive Low lowering");
    let error = check::check(&mut parsed.program).expect_err("must fail at checker");
    assert!(error.contains(reason), "{case}: High {error}");
    assert!(parsed
        .diagnostic(&error)
        .contains(&format!("bad.nagi:{high_line}\n")));
    fixture.write("saved.low", &low.text);
    fixture.write("manual.low", manual);
    fs::remove_file(fixture.0.join("bad.nagi")).unwrap();
    for (name, line) in [("saved.low", low_line), ("manual.low", manual_line)] {
        let mut loaded = source::load(&fixture.0.join(name), false)
            .expect("independent Low must parse/name resolve");
        let error = check::check(&mut loaded.program).expect_err("Low must fail by checker");
        assert!(error.contains(reason), "{case}: {name} {error}");
        assert!(loaded
            .diagnostic(&error)
            .contains(&format!("{name}:{line}\n")));
    }
}
#[test]
fn wrong_credentials_opacity_and_nonclone_reject_at_checker_original_lines() {
    reject_three("credential-type","import std.http.server as http\ndef wrong() -> Result[http.CorsPolicy, Error]:\n    return http.cors_policy([\"https://app.example.invalid\"], [\"content-type\"], true, 60, 4, 256, 4, 128)\ndef main():\n    print(0)\n",3,
        "import std.http.server as http;\nfn wrong() -> Result[http.CorsPolicy,Error] {\n return http.cors_policy([\"https://app.example.invalid\"],[\"content-type\"],true,60,4,256,4,128);\n}\nfn main() -> unit { print(0); }\n",3,"型");
    reject_three("opaque","import std.http.server as http\ndef wrong() -> http.CorsPolicy:\n    return http.CorsPolicy()\ndef main():\n    print(0)\n",3,
        "import std.http.server as http;\nfn wrong() -> http.CorsPolicy {\n return http.CorsPolicy();\n}\nfn main() -> unit { print(0); }\n",3,"opaque");
    reject_three("copy","import std.http.server as http\ndef wrong(value: http.CorsPolicy):\n    other = copy(view(value))\ndef main():\n    print(0)\n",3,
        "import std.http.server as http;\nfn wrong(value: http.CorsPolicy) -> unit {\n let other = copy(view(value));\n}\nfn main() -> unit { print(0); }\n",3,"非Clone");
}
#[test]
fn wrong_policy_nominal_shadow_and_reuse_reject_at_checker_original_lines() {
    reject_three("policy-type","import std.http.server as http\ndef wrong(app: http.App[i64, Error], policy: http.Policy[i64, unit]) -> Result[http.App[i64, Error], Error]:\n    return http.cors_route[i64, Error](app, http.Method.GET, \"/owned\", policy)\ndef main():\n    print(0)\n",3,
        "import std.http.server as http;\nfn wrong(app: http.App[i64,Error],policy: http.Policy[i64,unit]) -> Result[http.App[i64,Error],Error] {\n return http.cors_route[i64,Error](app,http.Method.GET,\"/owned\",policy);\n}\nfn main() -> unit { print(0); }\n",3,"型");
    reject_three("reuse","import std.http.server as http\ndef wrong(app: http.App[i64, Error], cors: http.CorsPolicy) -> http.CorsPolicy:\n    result = http.cors_route[i64, Error](app, http.Method.GET, \"/owned\", cors)\n    return cors\ndef main():\n    print(0)\n",4,
        "import std.http.server as http;\nfn wrong(app: http.App[i64,Error],cors: http.CorsPolicy) -> http.CorsPolicy {\n let result = http.cors_route[i64,Error](app,http.Method.GET,\"/owned\",cors);\n return cors;\n}\nfn main() -> unit { print(0); }\n",4,"move");
    reject_three("nominal","import std.http.server as http\nclass CorsPolicy:\n    value: i64\ndef wrong(app: http.App[i64, Error], cors: CorsPolicy) -> Result[http.App[i64, Error], Error]:\n    return http.cors_route[i64, Error](app, http.Method.GET, \"/owned\", cors)\ndef main():\n    print(0)\n",5,
        "import std.http.server as http;\nrecord CorsPolicy { value: i64; }\nfn wrong(app: http.App[i64,Error],cors: CorsPolicy) -> Result[http.App[i64,Error],Error] {\n return http.cors_route[i64,Error](app,http.Method.GET,\"/owned\",cors);\n}\nfn main() -> unit { print(0); }\n",4,"型");
}
#[test]
fn user_named_cors_functions_do_not_gain_standard_call_paths() {
    let f = native_triple::Fixture::new();
    f.write("user.nagi","def cors_policy(value: i64) -> i64:\n    return value\ndef main():\n    print(cors_policy(7))\n");
    let high = f.checked("user.nagi").unwrap();
    f.write("user.low", &emit::low(&high));
    fs::remove_file(f.0.join("user.nagi")).unwrap();
    let low = f.checked("user.low").unwrap();
    for program in [&high, &low] {
        let rust = emit::rust(&native_triple::checked_emission::seal(program)).unwrap();
        assert!(!rust.contains("::nagi_runtime::http_server::cors_policy"));
    }
    // Replacement integration uses the existing final checker and sealed emitter.
    f.write("config.nagi", "import std.http.server as http\ndef configure() -> Result[http.CorsPolicy, Error]:\n    return http.cors_policy([\"https://app.example.invalid\"], [\"content-type\"], http.CorsCredentials.OMIT, 0, 2, 128, 2, 64)\ndef main():\n    print(0)\n");
    let mut primary = f.checked("config.nagi").unwrap();
    let original_id = primary
        .modules
        .resolve_root_path("configure")
        .unwrap()
        .id
        .clone();
    f.write("native.low", "import std.http.server as http;\n@replace generated::configure\nfn replacement() -> Result[http.CorsPolicy,Error] { return http.cors_policy([\"https://app.example.invalid\"],[\"content-type\"],http.CorsCredentials.ALLOW,0,2,128,2,64); }\n");
    let native = source::load(&f.0.join("native.low"), false)
        .unwrap()
        .program;
    check::integrate(&mut primary, native).unwrap();
    assert_eq!(
        primary.modules.resolve_root_path("configure").unwrap().id,
        original_id
    );
    f.write("integrated.low", &emit::low(&primary));
    fs::remove_file(f.0.join("config.nagi")).unwrap();
    let saved = f.checked("integrated.low").unwrap();
    for program in [&primary, &saved] {
        let rust = emit::rust(&native_triple::checked_emission::seal(program)).unwrap();
        assert!(rust.contains("::nagi_runtime::http_server::cors_policy"));
        assert!(rust.contains("::nagi_runtime::http_server::CorsCredentials::ALLOW"));
    }
}
