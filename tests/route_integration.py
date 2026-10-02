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
        source.write_text('''@get("/lookup")
async def lookup(id: i64) -> Result[i64, Error]:
    return ok(id)

@get("/values/{id}")
async def value(id: i64) -> Result[i64, Error]:
    return ok(id)

@get("/legacy/{key}")
async def legacy(id: i64) -> Result[i64, Error]:
    return ok(id)

@get("/literal/{{id}}")
async def literal(id: i64) -> Result[i64, Error]:
    return ok(id)

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    port = try parse_i64(env("NAGI_TEST_PORT", "0"))
    return await serve(db, port)
''', encoding="utf-8")
        environment = {**os.environ, "NAGI_NATIVE_TARGET_DIR": str(target)}
        environment.pop("NAGI_ROOT", None)
        build = subprocess.run([str(compiler), "build", str(source)], cwd=folder,
                               env=environment, capture_output=True, text=True, encoding="utf-8")
        if build.returncode:
            raise AssertionError(build.stderr)
        with socket.socket() as available:
            available.bind(("127.0.0.1", 0))
            environment["NAGI_TEST_PORT"] = str(available.getsockname()[1])
        base = "http://127.0.0.1:" + environment["NAGI_TEST_PORT"]
        executable = target / "release" / ("nagi-routes.exe" if os.name == "nt" else "nagi-routes")
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
                    raise AssertionError("server did not start")
                cases = [("/lookup?id=42", 200, 42), ("/lookup?id=bad", 400, None),
                         ("/lookup", 400, None), ("/values/7?id=99", 200, 7),
                         ("/values/bad", 400, None), ("/legacy/8", 200, 8),
                         ("/literal/{id}?id=9", 200, 9)]
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
