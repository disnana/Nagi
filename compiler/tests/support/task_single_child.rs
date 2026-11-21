use super::Fixture;
use std::{fs, process::Command};

const HIGH: &str = r#"from std.task import kind, message, TaskFailureKind

def bad_index() -> i64:
    return 1

async def child() -> Result[i64, i64]:
    print("child")
    mode = env("NAGI_TASK_TEST_MODE", "")
    if mode == "panic":
        values = [17]
        return ok(values[bad_index()])
    if mode == "business":
        return fail(22)
    return ok(17)

async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn child()
        received = await task
        match received:
            case Ok(inner):
                match inner:
                    case Ok(value):
                        print("value")
                        print(value)
                    case Err(problem):
                        print("business")
                        print(problem)
            case Err(failure):
                assert_true(kind(failure) == TaskFailureKind.Panicked)
                print(message(failure))
    return ok(print("after scope"))
"#;

const LOW: &str = r#"from std.task import kind, message, TaskFailureKind;

fn bad_index() -> i64 { return 1; }

async fn child() -> Result[i64, i64] {
    print("child");
    let mode = env("NAGI_TASK_TEST_MODE", "");
    if mode == "panic" {
        let values = [17];
        return ok(values[bad_index()]);
    }
    if mode == "business" { return fail(22); }
    return ok(17);
}

async fn main() -> Result[unit, Error] {
    scope {
        let task = spawn child();
        let received = await task;
        match received {
            case Ok(inner) {
                match inner {
                    case Ok(value) { print("value"); print(value); }
                    case Err(problem) { print("business"); print(problem); }
                }
            }
            case Err(failure) {
                assert_true(kind(failure) == TaskFailureKind.Panicked);
                print(message(failure));
            }
        }
    }
    return ok(print("after scope"));
}
"#;

fn cli(fixture: &Fixture) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nagic"));
    command
        .current_dir(&fixture.0)
        .env_remove("NAGI_RUN_RETENTION");
    command
}

fn go_rejected(fixture: &Fixture, source: &str, diagnostic: &str, line: usize) {
    let output = cli(fixture)
        .args([
            "build",
            source,
            "--no-project",
            "--backend",
            "go",
            "--out",
            "rejected",
        ])
        .env("NAGI_GO", fixture.0.join("missing-go"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{source} unexpectedly built");
    assert!(stderr.contains(diagnostic), "{source}: {stderr}");
    assert!(
        stderr
            .lines()
            .any(|entry| entry.ends_with(&format!("{source}:{line}"))),
        "{stderr}"
    );
    assert!(!stderr.contains("Go toolchain was not found"), "{stderr}");
    assert!(!fixture.0.join("rejected/.nagi/apps").exists());
}

#[test]
fn single_task_cli_outcomes_and_existing_go_gate_in_three_forms() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", HIGH);
    fixture.write("handwritten.low", LOW);
    let lowered = cli(&fixture)
        .args(["lower", "main.nagi", "--no-project", "--out", "lowered"])
        .output()
        .unwrap();
    assert!(lowered.status.success(), "{lowered:?}");
    let saved = fs::read_to_string(fixture.0.join("lowered/generated.low")).unwrap();
    fixture.write("saved.low", &saved);

    for (source, text) in [
        ("main.nagi", HIGH),
        ("saved.low", &saved),
        ("handwritten.low", LOW),
    ] {
        if source == "saved.low" {
            fs::remove_file(fixture.0.join("main.nagi")).unwrap();
        }
        let scope = text
            .lines()
            .position(|line| {
                line.trim_start().starts_with("scope {")
                    || line.trim_start().starts_with("async with scope:")
            })
            .unwrap()
            + 1;
        go_rejected(
            &fixture,
            source,
            "async task operations are not supported by the initial Go backend",
            scope,
        );
        let built = cli(&fixture)
            .args([
                "build",
                source,
                "--no-project",
                "--profile",
                "dev",
                "--out",
                source.trim_end_matches(".low").trim_end_matches(".nagi"),
            ])
            .output()
            .unwrap();
        assert!(built.status.success(), "{source}: {built:?}");
        let stderr = String::from_utf8_lossy(&built.stderr);
        let native = stderr
            .lines()
            .find_map(|line| line.strip_prefix("native: "))
            .expect(&stderr);
        for (mode, status, stdout) in [
            ("ok", 0, "child\nvalue\n17\nafter scope\n"),
            ("business", 0, "child\nbusiness\n22\nafter scope\n"),
            ("panic", 1, "child\nchild panicked\n"),
        ] {
            let run = Command::new(native)
                .current_dir(&fixture.0)
                .env("NAGI_TASK_TEST_MODE", mode)
                .output()
                .unwrap();
            assert_eq!(run.status.code(), Some(status), "{source}/{mode}: {run:?}");
            assert_eq!(run.stdout, stdout.as_bytes(), "{source}/{mode}: {run:?}");
            if mode == "panic" {
                assert!(
                    String::from_utf8_lossy(&run.stderr).contains("child panicked"),
                    "{run:?}"
                );
            }
        }
    }

    for (name, high, low, anchor, replacement, diagnostic) in [
        (
            "double-await",
            include_str!("../../../tests/task-handles/double-await.nagi"),
            include_str!("../../../tests/task-handles/double-await.low"),
            " = await task;",
            " = await task;\n        let second = await task; # primary",
            "move後",
        ),
        (
            "outside-scope",
            include_str!("../../../tests/task-handles/outside-scope.nagi"),
            include_str!("../../../tests/task-handles/outside-scope.low"),
            "return ok(print(\"after scope\"));",
            "let escaped = task; # primary\n    return ok(print(\"after scope\"));",
            "未定義",
        ),
    ] {
        // Derive the saved-Low negative from valid CLI output so the ordinary
        // Low parser and checker have to reject the new use.
        let edited = saved.replacen(anchor, replacement, 1);
        assert_ne!(edited, saved, "{name}: missing Low fixture anchor");
        for (kind, text) in [
            ("high.nagi", high),
            ("manual.low", low),
            ("saved.low", &edited),
        ] {
            let source = format!("{name}-{kind}");
            let line = text
                .lines()
                .position(|line| line.contains("# primary"))
                .unwrap()
                + 1;
            fixture.write(&source, text);
            go_rejected(&fixture, &source, diagnostic, line);
        }
    }
}
