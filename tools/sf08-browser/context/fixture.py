#!/usr/bin/env python3
"""Owned Linux test-only TLS transport. Authentication stays in nagi-runtime."""
import argparse
import asyncio
import contextlib
import json
import os
from pathlib import Path
import signal
import ssl
import stat
import subprocess
import tempfile
import shutil

HEADER_LIMIT = 8192
BODY_LIMIT = 4096
RESPONSE_LIMIT = 32768
CONNECTION_LIMIT = 16
IO_SECONDS = 5
STOP_SECONDS = 14
CLOSE_FACTS = {"stopped": True, "http_joined": True, "prelogin_closed": True, "auth_closed": True}
STATS_KEYS = {"route_calls", "verifier_calls", "claim_committed", "ready_rows", "original_prelogin_unexpired"}


def atomic_json(path, value):
    data = json.dumps(value, separators=(",", ":")).encode()
    if len(data) > 32768:
        raise ValueError("bounded JSON exceeded")
    fd, temporary = tempfile.mkstemp(prefix=".atomic-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(temporary)


def certificates(directory, role):
    if type(role) is not str or role not in ('positive', 'wrong-ca'):
        raise ValueError("invalid certificate role")
    ca_subject = (
        "/CN=Nagi owned ephemeral SF08 positive CA"
        if role == 'positive'
        else "/CN=Nagi owned ephemeral SF08 wrong-ca CA"
    )
    # umask protects runtime-created private key files; none are repository artifacts.
    previous = os.umask(0o077)
    try:
        def openssl(*args):
            subprocess.run(["openssl", *args], cwd=directory, check=True,
                           stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, timeout=10)
        openssl("req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                "-nodes", "-keyout", "ca.key", "-out", "ca.pem", "-days", "2",
                "-subj", ca_subject, "-addext", "basicConstraints=critical,CA:TRUE",
                "-addext", "keyUsage=critical,keyCertSign,cRLSign")
        openssl("req", "-new", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                "-nodes", "-keyout", "leaf.key", "-out", "leaf.csr", "-subj", "/CN=localhost")
        (directory / "leaf.ext").write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost,DNS:app.example.invalid,IP:127.0.0.1\n")
        openssl("x509", "-req", "-in", "leaf.csr", "-CA", "ca.pem", "-CAkey", "ca.key",
                "-CAcreateserial", "-out", "leaf.pem", "-days", "1", "-extfile", "leaf.ext")
    finally:
        os.umask(previous)
    server = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server.minimum_version = ssl.TLSVersion.TLSv1_2
    server.set_alpn_protocols(["http/1.1"])
    server.load_cert_chain(directory / "leaf.pem", directory / "leaf.key")
    return server


def valid_close_facts(value):
    return isinstance(value, dict) and set(value) == set(CLOSE_FACTS) and all(item is True for item in value.values())


def validate_stats(stats, fixture):
    if not isinstance(stats, dict) or set(stats) != STATS_KEYS:
        raise ValueError("invalid bounded runtime facts")
    for key in ("route_calls", "verifier_calls"):
        if type(stats[key]) is not int or not 0 <= stats[key] <= 10000:
            raise ValueError("counter outside bound")
    if fixture == "login":
        if type(stats["claim_committed"]) is not bool or type(stats["original_prelogin_unexpired"]) is not bool:
            raise ValueError("invalid login facts")
        if type(stats["ready_rows"]) is not int or not 0 <= stats["ready_rows"] <= 10000:
            raise ValueError("invalid row fact")
    elif any(stats[key] is not None for key in ("claim_committed", "ready_rows", "original_prelogin_unexpired")):
        raise ValueError("invalid CORS facts")
    return stats


class Journal:
    def __init__(self, path):
        self.path = path
        self.reset()

    def reset(self):
        self.events = []
        atomic_json(self.path, {"schema_version": 2, "events": []})

    def record(self, fixture, method, path, status, origin_class, allow_origin, stats):
        validate_stats(stats, fixture)
        valid = ((fixture == "login" and (method, path) in (("GET", "/bootstrap"), ("POST", "/login"))
                  and origin_class == "same_origin" and allow_origin is False)
                 or (fixture == "cors" and (method, path) in (("GET", "/items/7"), ("POST", "/write"), ("OPTIONS", "/write"))
                     and origin_class in ("allowed", "denied") and allow_origin == (origin_class == "allowed")))
        if not valid or type(status) is not int or not 100 <= status <= 599 or type(allow_origin) is not bool or len(self.events) >= 64:
            raise ValueError("observation outside fixed contract")
        event = {"seq": len(self.events) + 1, "fixture": fixture, "method": method, "path": path,
                 "status": status, "origin_class": origin_class, "allow_origin_present": allow_origin, **stats}
        events = [*self.events, event]
        atomic_json(self.path, {"schema_version": 2, "events": events})
        self.events = events


async def bounded_line(reader, limit=2048, *, timeout=None):
    value = await asyncio.wait_for(reader.readuntil(b"\n"), IO_SECONDS if timeout is None else timeout)
    if len(value) > limit:
        raise ValueError("IPC limit exceeded")
    return value


async def bounded_response(reader):
    # One deadline covers the entire read-through-EOF, including slow fragments.
    async def read_all():
        result = bytearray()
        while True:
            part = await reader.read(RESPONSE_LIMIT + 1 - len(result))
            if not part:
                return bytes(result)
            result.extend(part)
            if len(result) > RESPONSE_LIMIT:
                raise ValueError("runtime response bound exceeded")
    return await asyncio.wait_for(read_all(), IO_SECONDS)


async def read_request(reader):
    head = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), IO_SECONDS)
    if len(head) > HEADER_LIMIT:
        raise ValueError("header limit exceeded")
    lines = head[:-4].split(b"\r\n")
    method, path, version = lines[0].decode("ascii").split(" ")
    if method not in ("GET", "POST", "OPTIONS") or version != "HTTP/1.1" or path not in ("/", "/favicon.ico", "/bootstrap", "/login", "/items/7", "/write"):
        raise ValueError("fixed test route required")
    headers = []
    for line in lines[1:]:
        key, value = line.split(b":", 1)
        if not key or key.strip() != key or any(c < 33 or c > 126 for c in key):
            raise ValueError("invalid header")
        headers.append((key.lower(), value.strip(), line))
    lengths = [v for k, v, _ in headers if k == b"content-length"]
    if len(lengths) > 1 or any(k == b"transfer-encoding" for k, _, _ in headers):
        raise ValueError("fixed body framing required")
    length = int(lengths[0]) if lengths and lengths[0].isdigit() else 0
    if lengths and not lengths[0].isdigit() or length > BODY_LIMIT:
        raise ValueError("body limit exceeded")
    body = await asyncio.wait_for(reader.readexactly(length), IO_SECONDS)
    # Transport-only close; preserve Host, Origin, Cookie, CSRF, authorization and response bytes.
    forwarded = [line for key, _, line in headers if key not in (b"connection", b"proxy-connection")]
    request = lines[0] + b"\r\n" + b"\r\n".join(forwarded) + b"\r\nConnection: close\r\n\r\n" + body
    origins = [value.decode("ascii") for key, value, _ in headers if key == b"origin"]
    if len(origins) > 1:
        raise ValueError("duplicate fixture origin")
    hosts = [value.decode("ascii") for key, value, _ in headers if key == b"host"]
    if len(hosts) != 1:
        raise ValueError("one fixture Host required")
    return method, path, origins[0] if origins else None, hosts[0], request


class Runtime:
    def __init__(self, executable, directory, app, ui):
        self.executable, self.directory, self.app, self.ui = executable, directory, app, ui
        self.process = None
        self.ports = {}
        self.ready = False
        self.command_lock = asyncio.Lock()

    async def start(self):
        self.process = await asyncio.create_subprocess_exec(str(self.executable), str(self.directory), self.app, self.ui,
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL,
            limit=2048)
        value = json.loads(await bounded_line(self.process.stdout))
        if set(value) != {"ready", "login_port", "cors_port"} or value["ready"] is not True:
            raise ValueError("runtime readiness refused")
        for kind in ("login", "cors"):
            port = value[kind + "_port"]
            if type(port) is not int or not 1024 <= port <= 65535:
                raise ValueError("runtime loopback port refused")
            self.ports[kind] = port
            reader, writer = await asyncio.wait_for(asyncio.open_connection("127.0.0.1", port, limit=HEADER_LIMIT), IO_SECONDS)
            try:
                authority = self.app.removeprefix("https://").encode()
                writer.write(b"GET /__sf08_readiness HTTP/1.1\r\nHost: " + authority + b"\r\nConnection: close\r\n\r\n")
                await writer.drain()
                reply = await bounded_response(reader)
                if not reply.startswith(b"HTTP/1.1 404 "):
                    raise ValueError("runtime HTTP readiness refused")
            finally:
                writer.close()
                await asyncio.wait_for(writer.wait_closed(), 2)
        self.ready = True

    async def command(self, command):
        if command not in ("snapshot login", "snapshot cors", "stop"):
            raise ValueError("fixed runtime command required")
        # The outer stop deadline covers lock acquisition and all these phases.
        # Its ACK must not inherit the shorter ordinary snapshot I/O timeout.
        budget = STOP_SECONDS if command == "stop" else IO_SECONDS
        async with self.command_lock:
            self.process.stdin.write(command.encode() + b"\n")
            await asyncio.wait_for(self.process.stdin.drain(), budget)
            return json.loads(await bounded_line(self.process.stdout, timeout=budget))

    async def snapshot(self, kind):
        return validate_stats(await self.command("snapshot " + kind), kind)

    async def stop(self):
        # One original 14s budget includes lock, drain, ACK and actual child wait.
        deadline = asyncio.get_running_loop().time() + STOP_SECONDS
        acknowledgement = await asyncio.wait_for(self.command("stop"), STOP_SECONDS)
        if not valid_close_facts(acknowledgement):
            raise ValueError("runtime close not acknowledged")
        remaining = max(0, deadline - asyncio.get_running_loop().time())
        if await asyncio.wait_for(self.process.wait(), min(2, remaining)) != 0:
            raise ValueError("runtime close not acknowledged")
        return acknowledgement

    async def abort(self):
        if self.process is None:
            return
        for signum in (signal.SIGTERM, signal.SIGKILL):
            if self.process.returncode is None:
                with contextlib.suppress(ProcessLookupError):
                    self.process.send_signal(signum)
                try:
                    await asyncio.wait_for(self.process.wait(), 2)
                except asyncio.TimeoutError:
                    continue
        if self.process.returncode is None:
            raise RuntimeError("owned runtime exit unconfirmed")


class Supervisor:
    def __init__(self, executable, work, runtime_factory=Runtime):
        self.executable, self.work, self.runtime_factory = executable, work, runtime_factory
        self.owned = Path(tempfile.mkdtemp(prefix="owned-", dir=work))
        self.ssl_context = certificates(self.owned, 'positive')
        self.runtime = None
        self.servers, self.tasks, self.control_tasks = [], set(), set()
        self.request_lock, self.control_lock = asyncio.Lock(), asyncio.Lock()
        self.stopping = asyncio.Event()
        self.accepting = False
        self.failed = False
        self.resets = 0
        self.stop_result = None
        self._stop_task = None
        self.runtime_ready = False
        self.journal = Journal(work / "observations.json")

    async def start(self):
        try:
            await self._start()
        except BaseException:
            self.failed = True
            self.stopping.set()
            raise

    async def _start(self):
        for kind, host in (("app", "localhost"), ("ui", "app.example.invalid"), ("denied", "127.0.0.1")):
            server = await asyncio.start_server(lambda r, w, k=kind: self.accept(k, r, w), "127.0.0.1", 0,
                                                limit=HEADER_LIMIT, backlog=CONNECTION_LIMIT, start_serving=False)
            self.servers.append(server)
            port = server.sockets[0].getsockname()[1]
            setattr(self, kind, f"https://{host}:{port}")
        await self.new_runtime()
        if self.stopping.is_set():
            raise ValueError("startup cancelled")
        for server in self.servers:
            await server.start_serving()
        self.control = await asyncio.start_unix_server(self.accept_control, path=self.work / "control.sock", limit=128)
        os.chmod(self.work / "control.sock", 0o600)
        self.accepting = True
        # Real TLS probes verify our runtime-generated chain and SANs without changing host trust.
        client = ssl.create_default_context(cafile=self.owned / "ca.pem")
        for kind, server in zip(("app", "ui", "denied"), self.servers):
            host = {"app": "localhost", "ui": "app.example.invalid", "denied": "127.0.0.1"}[kind]
            reader, writer = await asyncio.wait_for(asyncio.open_connection("127.0.0.1", server.sockets[0].getsockname()[1],
                ssl=client, server_hostname=host), IO_SECONDS)
            authority = getattr(self, kind).removeprefix("https://")
            writer.write(f"GET / HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n".encode())
            await writer.drain()
            response = await bounded_response(reader)
            if not response.startswith(b"HTTP/1.1 200 "):
                raise ValueError("TLS readiness refused")
            writer.close()
            await asyncio.wait_for(writer.wait_closed(), 2)
        if self.stopping.is_set():
            raise ValueError("startup cancelled")
        atomic_json(self.work / "ready.json", {"schema_version": 1, "app": self.app, "ui": self.ui,
            "denied": self.denied, "ca": str(self.owned / "ca.pem"), "control": str(self.work / "control.sock")})

    async def new_runtime(self):
        if self.stopping.is_set():
            raise ValueError("new runtime after stop refused")
        self.session = Path(tempfile.mkdtemp(prefix="session-", dir=self.owned))
        self.runtime = self.runtime_factory(self.executable, self.session, self.app, self.ui)
        self.runtime_ready = False
        await self.runtime.start()
        self.runtime_ready = True

    def task_done(self, task, owners):
        owners.discard(task)
        if not task.cancelled() and task.exception() is not None:
            self.failed = True
            self.stopping.set()

    def accept_control(self, reader, writer):
        if self.stopping.is_set() or len(self.control_tasks) >= 2:
            writer.transport.abort()
            return
        task = asyncio.create_task(self.control_request(reader, writer))
        self.control_tasks.add(task)
        task.add_done_callback(lambda done: self.task_done(done, self.control_tasks))

    def accept(self, kind, reader, writer):
        # Register the TCP owner before any TLS handshake; start_server(ssl=...) would hide that lifetime.
        writer.transport.pause_reading()
        if not self.accepting or len(self.tasks) >= CONNECTION_LIMIT:
            writer.transport.abort()
            return
        task = asyncio.create_task(self.handle(kind, reader, writer))
        self.tasks.add(task)
        task.add_done_callback(lambda done: self.task_done(done, self.tasks))

    async def handle(self, kind, reader, writer):
        try:
            await writer.start_tls(self.ssl_context, ssl_handshake_timeout=3)
            method, path, origin, host, request = await read_request(reader)
            if host != getattr(self, kind).removeprefix("https://"):
                raise ValueError("fixture Host mismatch")
            if path in ("/", "/favicon.ico") or kind != "app":
                body = b"<!doctype html><title>Owned SF08 fixture</title>"
                response = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: " + str(len(body)).encode() + b"\r\nConnection: close\r\n\r\n" + body
            else:
                fixture = "login" if path in ("/bootstrap", "/login") else "cors"
                async with self.request_lock:
                    backend, outbound = await asyncio.wait_for(asyncio.open_connection("127.0.0.1", self.runtime.ports[fixture], limit=HEADER_LIMIT), IO_SECONDS)
                    try:
                        outbound.write(request)
                        await outbound.drain()
                        response = await bounded_response(backend)
                    finally:
                        outbound.close()
                        await asyncio.wait_for(outbound.wait_closed(), 2)
                    head = response.split(b"\r\n\r\n", 1)[0]
                    status = int(head.split(b"\r\n", 1)[0].split(b" ")[1])
                    grant = any(line.lower().startswith(b"access-control-allow-origin:") for line in head.split(b"\r\n")[1:])
                    if fixture == "login":
                        if origin not in (None, self.app): raise ValueError("unexpected login observation class")
                        origin_class = "same_origin"
                    else:
                        if origin not in (self.ui, self.denied): raise ValueError("unexpected CORS observation class")
                        origin_class = "allowed" if origin == self.ui else "denied"
                    self.journal.record(fixture, method, path, status, origin_class, grant, await self.runtime.snapshot(fixture))
            writer.write(response)
            await asyncio.wait_for(writer.drain(), IO_SECONDS)
        except (ssl.SSLError, ConnectionError, asyncio.IncompleteReadError, asyncio.TimeoutError):
            # Failed TLS/network activity is never counted as a successful browser observation.
            pass
        except Exception:
            self.failed = True
            self.stopping.set()
        finally:
            writer.close()
            with contextlib.suppress(ConnectionError, ssl.SSLError, asyncio.TimeoutError):
                await asyncio.wait_for(writer.wait_closed(), 2)

    async def join_tasks(self, owners):
        tasks = list(owners)
        if tasks:
            _, pending = await asyncio.wait(tasks, timeout=8)
            for task in pending:
                task.cancel()
            # Cancellation is followed by actual task completion, never a detached owner.
            await asyncio.gather(*tasks, return_exceptions=True)
            for task in tasks:
                owners.discard(task)

    async def drain(self):
        self.accepting = False
        await self.join_tasks(self.tasks)

    async def close_runtime(self):
        if self.runtime is None:
            return False
        try:
            if not self.runtime_ready:
                raise ValueError("runtime was not ready")
            acknowledgement = await self.runtime.stop()
            if not valid_close_facts(acknowledgement):
                raise ValueError("close acknowledgement shape refused")
        except BaseException:
            await self.runtime.abort()
            raise
        # Keep the DB if runtime close is unconfirmed; remove only this session after
        # all HTTP joins, both close acknowledgements and process exit zero.
        shutil.rmtree(self.session)
        self.runtime = None
        self.runtime_ready = False
        return True

    async def reset(self):
        # Caller holds control_lock, also used by stop; startup cannot race stop.
        if self.resets >= 16 or self.stopping.is_set():
            raise ValueError("reset refused")
        await self.drain()
        async with self.request_lock:
            await self.close_runtime()
            self.journal.reset()
            await self.new_runtime()
        self.resets += 1
        self.accepting = not self.stopping.is_set()

    async def control_request(self, reader, writer):
        try:
            async with self.control_lock:
                command = json.loads(await bounded_line(reader, 64))
                if command == {"op": "reset"}:
                    await self.reset()
                    result = {"ready": True}
                elif command == {"op": "stop"}:
                    self.stopping.set()
                    result = {"stopping": True}
                else:
                    raise ValueError("unknown fixed control operation")
                writer.write(json.dumps(result).encode() + b"\n")
                await asyncio.wait_for(writer.drain(), IO_SECONDS)
        except Exception:
            self.failed = True
            self.stopping.set()
        finally:
            writer.close()
            with contextlib.suppress(ConnectionError, ssl.SSLError, asyncio.TimeoutError):
                await asyncio.wait_for(writer.wait_closed(), 2)

    async def stop(self):
        # Supervisor is confined to one event loop. Publish the one cleanup
        # owner without yielding, so simultaneous callers share its result/error.
        if self._stop_task is None:
            self.stopping.set()
            self.accepting = False
            self._stop_task = asyncio.create_task(self._stop_owned())
        cancelled = False
        # A non-cancellation cleanup error propagates from the shared task even
        # after caller cancellation; never replace it with a success/False.
        while True:
            try:
                result = await asyncio.shield(self._stop_task)
            except asyncio.CancelledError:
                if self._stop_task.cancelled():
                    raise
                # Defer caller cancellation until the owner actually completes;
                # shield alone would leave cleanup running after the caller exits.
                # Repeated caller cancellation must also leave that owner intact.
                cancelled = True
            else:
                if cancelled:
                    raise asyncio.CancelledError
                return result

    async def _stop_owned(self):
        for server in self.servers:
            server.close()
        if hasattr(self, "control"):
            self.control.close()
        closed = False
        failure = None
        try:
            # Finish an in-progress reset under the same lock before stopping its
            # runtime. No control task is cancelled during subprocess startup.
            async with self.control_lock:
                await self.drain()
                closed = await self.close_runtime()
        except BaseException as error:
            self.failed = True
            failure = error
        try:
            # A refused runtime receipt must not bypass frontend/control joins.
            await self.join_tasks(self.control_tasks)
            for server in self.servers:
                await asyncio.wait_for(server.wait_closed(), 2)
            if hasattr(self, "control"):
                await asyncio.wait_for(self.control.wait_closed(), 2)
        except BaseException as error:
            self.failed = True
            if failure is None:
                failure = error
        finally:
            self.stop_result = closed and not self.failed
            atomic_json(self.work / "closed.json", {"schema_version": 1,
                "frontends_joined": not self.tasks and not self.control_tasks,
                "http_joined": closed, "prelogin_closed": closed, "auth_closed": closed,
                "runtime_exit_zero": closed, "fixture_failed": self.failed, "resets": self.resets})
        if failure is not None:
            raise failure
        return self.stop_result


async def main(args):
    work = args.work_dir.resolve(strict=True)
    mode = work.stat()
    if not stat.S_ISDIR(mode.st_mode) or mode.st_uid != os.getuid() or stat.S_IMODE(mode.st_mode) != 0o700:
        raise ValueError("exclusive owned 0700 workspace required")
    fixture = Supervisor(args.runtime.resolve(strict=True), work)
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGTERM, signal.SIGINT): loop.add_signal_handler(sig, fixture.stopping.set)
    try:
        await fixture.start()
        await fixture.stopping.wait()
    except BaseException:
        fixture.failed = True
        raise
    finally:
        okay = await fixture.stop()
    return 0 if okay else 2


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    try:
        raise SystemExit(asyncio.run(main(parser.parse_args())))
    except Exception:
        # No exception text: paths, HTTP material and dependency messages are not fixture artifacts.
        print("SF08 fixture failed; no successful lifecycle receipt is established.", file=__import__("sys").stderr)
        raise SystemExit(2)
