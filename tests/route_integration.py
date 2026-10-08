"""Build a small Nagi server and check path/query extraction over real HTTP."""
import argparse
import json
import os
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def verify(compiler: Path, target: Path) -> None:
    compiler, target = compiler.resolve(), target.resolve()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with tempfile.TemporaryDirectory(prefix="nagi routes 凪 ") as temporary:
        folder = Path(temporary)
        source = folder / "routes.nagi"
        source.write_text('''import std.http.server as http

class State:
    ready: bool

def query_id(request: view[http.Request]) -> Result[i64, Error]:
    match request.query:
        case Some(query):
            if len(query) < 4 or try slice(query, 0, 3) != "id=":
                return error("query must contain id")
            return parse_i64(try slice(query, 3, len(query)))
        case None:
            return error("query must contain id")

def path_id(request: view[http.Request], prefix: view[str]) -> Result[i64, Error]:
    if len(request.path) <= len(prefix):
        return error("path value is missing")
    return parse_i64(try slice(request.path, len(prefix), len(request.path)))

async def lookup(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return http.json[i64](http.Status.OK, try query_id(view(request)))

async def value(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return http.json[i64](http.Status.OK, try path_id(view(request), view("/values/")))

async def legacy(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return http.json[i64](http.Status.OK, try path_id(view(request), view("/legacy/")))

async def literal(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return http.json[i64](http.Status.OK, try query_id(view(request)))

async def health(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return ok(http.text(http.Status.OK, "ok"))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(ready=true))
    app = try http.route(app, http.Method.GET, "/health", http.public_policy[State](), health)
    app = try http.route(app, http.Method.GET, "/lookup", http.public_policy[State](), lookup)
    app = try http.route(app, http.Method.GET, "/values/{id}", http.public_policy[State](), value)
    app = try http.route(app, http.Method.GET, "/legacy/{key}", http.public_policy[State](), legacy)
    app = try http.route(app, http.Method.GET, "/literal/%7Bid%7D", http.public_policy[State](), literal)
    port = try parse_i64(env("NAGI_TEST_PORT", "0"))
    return await http.serve(app, port, http.default_options())
''', encoding="utf-8")
        environment = {**os.environ, "NAGI_NATIVE_TARGET_DIR": str(target)}
        environment.pop("NAGI_ROOT", None)
        build = subprocess.run([str(compiler), "build", str(source), "--no-project", "--out", str(folder / "build")], cwd=folder,
                               env=environment, capture_output=True, text=True, encoding="utf-8")
        if build.returncode:
            raise AssertionError(build.stderr)
        with socket.socket() as available:
            available.bind(("127.0.0.1", 0))
            environment["NAGI_TEST_PORT"] = str(available.getsockname()[1])
        base = "http://127.0.0.1:" + environment["NAGI_TEST_PORT"]
        native = [line.removeprefix("native: ").strip()
                  for line in (build.stdout + "\n" + build.stderr).splitlines()
                  if line.startswith("native: ")]
        assert len(native) == 1, build.stdout + build.stderr
        executable = Path(native[0])
        assert executable.is_file(), (build.stdout, build.stderr, executable)
        with (folder / "server.log").open("w+") as log:
            server = subprocess.Popen([str(executable)], cwd=folder, env=environment,
                                      stdout=log, stderr=log)
            try:
                for _ in range(100):
                    if server.poll() is not None:
                        log.seek(0)
                        raise AssertionError("server exited: " + log.read())
                    try:
                        with opener.open(base + "/health", timeout=1) as response:
                            assert response.status == 200
                        break
                    except urllib.error.URLError:
                        time.sleep(0.05)
                else:
                    log.seek(0)
                    raise AssertionError("server did not start: " + log.read())
                cases = [("/lookup?id=42", 200, 42), ("/lookup?id=bad", 400, None),
                         ("/lookup", 400, None), ("/values/7?id=99", 200, 7),
                         ("/values/bad", 400, None), ("/legacy/8", 200, 8),
                         ("/literal/%7Bid%7D?id=9", 200, 9)]
                for path, status, value in cases:
                    try:
                        response = opener.open(base + path, timeout=5)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        body = response.read().decode()
                        assert response.status == status, (path, response.status, body)
                        if value is not None:
                            assert json.loads(body) == value, (path, body)
                print(f"HTTP route parameters: {len(cases)} passed")
            finally:
                if server.poll() is None:
                    server.terminate()
                server.wait(timeout=10)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path,
                        default=ROOT / "target/release" / ("nagic.exe" if os.name == "nt" else "nagic"))
    parser.add_argument("--target", type=Path, default=ROOT / "native-target")
    arguments = parser.parse_args()
    verify(arguments.compiler, arguments.target)
