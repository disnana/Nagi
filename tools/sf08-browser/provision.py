"""Candidate: mutate CA stores only in the dedicated disposable CI container."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile

BASE = Path("/run/nagi-sf08")
IMAGE = "sha256:bdfda2a8fdf48707c5e50a49bd3e06d8d0c759153dc8ee057d8a81c3450bb3e9"
PROTECTED = ("/root", "/home", "/etc/ssl", "/etc/pki", "/usr/local/share/ca-certificates", "/ms-playwright")


class Refused(RuntimeError):
    pass


def refuse():
    raise Refused("SF08 container trust prerequisite refused") from None


def under(path, prefix):
    return path == prefix or path.startswith(prefix + "/")


def validate_scope(environ, uid, docker_marker, mounts, base_mode, base_uid, docker_socket):
    # A misuse guard for the explicit docker-run contract, not root attestation.
    if (environ.get("GITHUB_ACTIONS") != "true" or environ.get("NAGI_SF08_EPHEMERAL") != IMAGE
            or uid != 0 or not docker_marker or docker_socket or base_uid != 0 or base_mode != 0o700):
        refuse()
    if any(key in environ for key in ("NODE_TLS_REJECT_UNAUTHORIZED", "NODE_EXTRA_CA_CERTS",
            "SSL_CERT_FILE", "SSL_CERT_DIR", "PLAYWRIGHT_FIREFOX_POLICIES_JSON")):
        refuse()
    if mounts.get("/") != "overlay" or mounts.get(str(BASE)) != "tmpfs":
        refuse()
    if any(under(path, str(BASE)) and path != str(BASE) for path in mounts):
        refuse()  # No host fixture/CA/HOME bind mount nested inside the tmpfs.
    if any(any(under(path, protected) for protected in PROTECTED) for path in mounts if path != "/"):
        refuse()


def check_scope():
    try:
        if os.uname().sysname != "Linux":
            refuse()
        marker = os.lstat("/.dockerenv")
        base = os.lstat(BASE)
        with open("/proc/self/mountinfo", "rb") as stream:
            data = stream.read(262145)
        if len(data) > 262144:
            refuse()
        mounts = {}
        for line in data.decode("utf-8").splitlines():
            left, right = line.split(" - ", 1)
            path = left.split()[4]
            # Reject ambiguous escaped mount paths instead of interpreting them.
            if "\\" in path:
                refuse()
            mounts[path] = right.split()[0]
        validate_scope(os.environ, os.geteuid(), stat.S_ISREG(marker.st_mode) and marker.st_uid == 0,
                       mounts, stat.S_IMODE(base.st_mode), base.st_uid,
                       Path("/var/run/docker.sock").exists())
        if not stat.S_ISDIR(base.st_mode):
            refuse()
    except (OSError, ValueError, IndexError):
        refuse()


def command_plan(work):
    home = work / "home"
    nss = home / ".local/share/pki/nssdb"
    return [
        ["openssl", "x509", "-in", str(work / "ca.pem"), "-noout"],
        ["openssl", "verify", "-CAfile", str(work / "ca.pem"), str(work / "ca.pem")],
        ["certutil", "-N", "-d", "sql:" + str(nss), "--empty-password"],
        ["certutil", "-A", "-d", "sql:" + str(nss), "-n", "Nagi SF08 ephemeral", "-t", "C,,", "-i", str(work / "ca.pem")],
        ["update-ca-certificates"],
    ]


def exclusive(path, data, mode=0o600):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, mode)
    with os.fdopen(fd, "wb") as stream:
        stream.write(data)


def provision(ca):
    check_scope()  # Must precede every mkdir/write/command; no test bypass flag.
    try:
        raw = os.fspath(ca)
        # BASE is a directory boundary, never a CA file to open or tool-check.
        if raw == str(BASE):
            refuse()
        for program in ("openssl", "certutil", "update-ca-certificates"):
            if shutil.which(program) is None:
                refuse()  # Never apt/install/download here.
        if (not os.path.isabs(raw) or os.path.normpath(raw) != raw
                or not under(raw, str(BASE))):
            refuse()
        # Resolve every descendant through held directory fds, not lstat/open.
        current = os.open(BASE, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            parts = Path(raw).relative_to(BASE).parts
            for part in parts[:-1]:
                child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=current)
                os.close(current)
                current = child
                info = os.fstat(current)
                if info.st_uid != 0 or stat.S_IMODE(info.st_mode) != 0o700:
                    refuse()
            fd = os.open(parts[-1], os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=current)
            try:
                info = os.fstat(fd)
                if (not stat.S_ISREG(info.st_mode) or info.st_uid != 0
                        or stat.S_IMODE(info.st_mode) != 0o600 or not 1 <= info.st_size <= 16384):
                    refuse()
                data = bytearray()
                while len(data) <= 16384:
                    chunk = os.read(fd, 16385 - len(data))
                    if not chunk:
                        break
                    data.extend(chunk)
                after = os.fstat(fd)
                if (len(data) != info.st_size or len(data) > 16384 or info.st_size != after.st_size
                        or info.st_mtime_ns != after.st_mtime_ns or info.st_ctime_ns != after.st_ctime_ns):
                    refuse()
            finally:
                os.close(fd)
        finally:
            os.close(current)
        work = Path(tempfile.mkdtemp(prefix="trust-", dir=BASE))
        for relative in ("home", "home/.local", "home/.local/share", "home/.local/share/pki", "home/.local/share/pki/nssdb"):
            (work / relative).mkdir(mode=0o700)
        exclusive(work / "ca.pem", data)
        exclusive(work / "firefox-policy.json", json.dumps({"policies": {"Certificates": {
            "Install": [str(work / "ca.pem")]}}}).encode())
        env = dict(os.environ, HOME=str(work / "home"), XDG_DATA_HOME=str(work / "home/.local/share"),
                   XDG_CONFIG_HOME=str(work / "home/.config"), XDG_CACHE_HOME=str(work / "home/.cache"))
        plan = command_plan(work)
        for command in plan[:4]:
            subprocess.run(command, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, check=True, timeout=20)
        # This system mutation is inside the guarded fresh container only.
        exclusive(Path("/usr/local/share/ca-certificates/nagi-sf08.crt"), data, 0o644)
        subprocess.run(plan[4], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL, check=True, timeout=20)
        receipt = {"schema_version": 1, "home": env["HOME"], "ca": str(work / "ca.pem"),
                   "ca_sha256": hashlib.sha256(data).hexdigest(), "firefox_policy": str(work / "firefox-policy.json"),
                   "declared_image": IMAGE, "stores_provisioned": True, "browser_trust_verified": False}
        exclusive(work / "provisioned.json", json.dumps(receipt, separators=(",", ":")).encode())
        return work
    except (OSError, ValueError, subprocess.SubprocessError):
        refuse()  # Retain owned data/container on failure; no recursive removal.


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--ca", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        previous = os.umask(0o077)
        try:
            if args.check:
                check_scope()
            elif args.ca is None:
                refuse()
            else:
                provision(args.ca)
        finally:
            os.umask(previous)
    except Refused:
        print("SF08 trust provisioning refused")
        raise SystemExit(1)
