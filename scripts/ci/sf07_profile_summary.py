#!/usr/bin/env python3
"""Validate and re-emit the single closed SF07 small-profile test record."""
from __future__ import annotations

import json
import re
import sys
from typing import Any

MAX_CAPTURE_BYTES = 64 * 1024
PREFIX = "SF07_PROFILE_JSON="
TEST_NAME = "response_profile_reports_closed_measurements"
RUNNING = "running 1 test"
TEST_OK = f"test {TEST_NAME} ... ok"
SUMMARY = re.compile(
    r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in (?:0|[1-9][0-9]*)(?:\.[0-9]+)?s"
)
CARGO_FINISHED = re.compile(
    r" {0,8}Finished `test` profile \[unoptimized \+ debuginfo\] target\(s\) in (?:0|[1-9][0-9]*)(?:\.[0-9]+)?s"
)
CARGO_RUNNING = re.compile(
    r" {0,8}Running tests[/\\]sf07_small_profile\.rs "
    r"\((?:[A-Za-z]:)?[^()\r\n]*[/\\]sf07_small_profile-[0-9a-f]{16,64}(?:\.exe)?\)"
)
ALLOCATION_SCOPE = (
    "calling-thread allocation events during the 32-request window; includes loopback client, response parsing "
    "and current-thread Tokio tasks; deallocations may include pre-window allocations; excludes other threads "
    "and process RSS"
)


def _pairs_without_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result


def _reject_constant(_: str) -> None:
    raise ValueError("non-finite JSON number")


def _integer(value: Any, *, positive: bool = False, maximum: int = (1 << 64) - 1) -> bool:
    return type(value) is int and (value > 0 if positive else value >= 0) and value <= maximum


def _expect_keys(value: Any, expected: set[str]) -> dict[str, Any]:
    if type(value) is not dict or set(value) != expected:
        raise ValueError("invalid object shape")
    return value


def _validate_record(record: Any) -> dict[str, Any]:
    root = _expect_keys(
        record,
        {"schema", "platform", "architecture", "fixture", "type_sizes_bytes", "observed"},
    )
    if root["schema"] != "nagi.sf07.small_profile.v1":
        raise ValueError("unsupported profile schema")
    if type(root["platform"]) is not str or root["platform"] not in {"linux", "windows", "macos"}:
        raise ValueError("unsupported platform")
    if type(root["architecture"]) is not str or root["architecture"] not in {"x86_64", "aarch64"}:
        raise ValueError("unsupported architecture")

    fixture = _expect_keys(
        root["fixture"],
        {
            "transport", "route_count", "connection_slots", "request_slots", "client_concurrency",
            "authority_entry_limit", "authority_byte_limit", "other_server_options", "response_limits", "rate_limits", "route_policies", "header_heavy_value_bytes",
        },
    )
    fixed_fixture = {
        "transport": "loopback_http1",
        "route_count": 4,
        "connection_slots": 8,
        "request_slots": 8,
        "client_concurrency": 1,
        "authority_entry_limit": 1,
        "authority_byte_limit": 128,
        "other_server_options": "default_options; not introspected",
        "header_heavy_value_bytes": 96,
    }
    if any(fixture[key] != value or type(fixture[key]) is not type(value) for key, value in fixed_fixture.items()):
        raise ValueError("profile configuration mismatch")
    responses = _expect_keys(
        fixture["response_limits"],
        {
            "body_bytes", "user_header_bytes", "user_header_count", "managed_header_bytes",
            "managed_header_count", "serialization_writes", "live_responses", "retained_bytes",
            "fallback_responses", "fallback_retained_bytes", "fallback_managed_header_bytes",
            "fallback_managed_header_count",
        },
    )
    expected_responses = {
        "body_bytes": 4096,
        "user_header_bytes": 4096,
        "user_header_count": 16,
        "managed_header_bytes": 4096,
        "managed_header_count": 32,
        "serialization_writes": 4096,
        "live_responses": 8,
        "retained_bytes": 2 * 1024 * 1024,
        "fallback_responses": 8,
        "fallback_retained_bytes": 2 * 1024 * 1024,
        "fallback_managed_header_bytes": 4096,
        "fallback_managed_header_count": 32,
    }
    if any(not _integer(value) for value in responses.values()) or responses != expected_responses:
        raise ValueError("response profile mismatch")
    rates = _expect_keys(
        fixture["rate_limits"],
        {"rules", "domains", "subject_slots", "retained_bytes", "request_work"},
    )
    if any(not _integer(value) for value in rates.values()) or rates != {
        "rules": 8,
        "domains": 8,
        "subject_slots": 1,
        "retained_bytes": 1024 * 1024,
        "request_work": 65_536,
    }:
        raise ValueError("rate profile mismatch")
    policies = fixture["route_policies"]
    expected_policies = [
        {"path": "/ordinary", "burst": 1000, "refill_ms": 1000},
        {"path": "/headers", "burst": 1000, "refill_ms": 1000},
        {"path": "/limited", "burst": 1, "refill_ms": 86_400_000},
        {"path": "/healthy", "burst": 1000, "refill_ms": 1000},
    ]
    if type(policies) is not list or len(policies) != len(expected_policies):
        raise ValueError("route policy mismatch")
    for policy, expected in zip(policies, expected_policies, strict=True):
        policy = _expect_keys(policy, {"path", "burst", "refill_ms"})
        if (
            type(policy["path"]) is not str
            or not _integer(policy["burst"], positive=True)
            or not _integer(policy["refill_ms"], positive=True)
            or policy != expected
        ):
            raise ValueError("route policy mismatch")
    sizes = _expect_keys(
        root["type_sizes_bytes"],
        {"response_context", "response", "response_limits", "rate_limits", "rate_policy"},
    )
    if any(not _integer(size, positive=True, maximum=1_048_576) for size in sizes.values()):
        raise ValueError("invalid public type size")

    observed = _expect_keys(
        root["observed"],
        {
            "warmup_requests", "ordinary_requests", "ordinary_latency_ns", "ordinary_median_ns",
            "ordinary_p95_ns", "header_heavy_status", "header_heavy_custom_header_count",
            "header_heavy_custom_header_value_bytes", "header_heavy_body_bytes", "header_heavy_latency_ns",
            "limited_accepted_latency_ns",
            "rate_limited_latency_ns", "rate_limited_status", "rate_limited_retry_after_seconds",
            "healthy_after_rate_fallback_status", "healthy_after_rate_fallback_latency_ns",
            "fallback_retained_high_water",
            "retained_high_water_note", "shutdown_joined", "allocation_scope",
            "calling_thread_allocations",
        },
    )

    # The closed schema requires integer JSON numbers, not bools or floats.
    # Python otherwise treats True == 1 and 16.0 == 16 during dict/value checks.
    integer_observations = {
        "warmup_requests": 4,
        "ordinary_requests": 32,
        "header_heavy_status": 200,
        "header_heavy_custom_header_count": 12,
        "header_heavy_custom_header_value_bytes": 12 * 96,
        "header_heavy_body_bytes": 7,
        "rate_limited_status": 429,
        "healthy_after_rate_fallback_status": 200,
    }
    if any(
        not _integer(observed[key], positive=True) or observed[key] != value
        for key, value in integer_observations.items()
    ):
        raise ValueError("profile observation mismatch")
    samples = observed["ordinary_latency_ns"]
    if type(samples) is not list or len(samples) != 32 or any(
        not _integer(sample, positive=True, maximum=1 << 63) for sample in samples
    ):
        raise ValueError("invalid latency samples")
    ordered = sorted(samples)
    median = (ordered[15] + ordered[16]) // 2
    p95 = ordered[(len(ordered) * 95 + 99) // 100 - 1]
    if (
        not _integer(observed["ordinary_median_ns"], positive=True, maximum=1 << 63)
        or not _integer(observed["ordinary_p95_ns"], positive=True, maximum=1 << 63)
        or observed["ordinary_median_ns"] != median
        or observed["ordinary_p95_ns"] != p95
    ):
        raise ValueError("latency summary mismatch")
    fixed_observations = {
        "header_heavy_status": 200,
        "header_heavy_custom_header_count": 12,
        "header_heavy_custom_header_value_bytes": 12 * 96,
        "header_heavy_body_bytes": 7,
        "rate_limited_status": 429,
        "healthy_after_rate_fallback_status": 200,
        "fallback_retained_high_water": None,
        "retained_high_water_note": "not exposed by the public API",
        "shutdown_joined": True,
        "allocation_scope": ALLOCATION_SCOPE,
    }
    if any(
        observed[key] != value or (value is not None and type(observed[key]) is not type(value))
        for key, value in fixed_observations.items()
    ):
        raise ValueError("profile observation mismatch")
    if not _integer(observed["rate_limited_retry_after_seconds"], positive=True, maximum=86_400):
        raise ValueError("invalid retry-after sample")
    for key in (
        "header_heavy_latency_ns",
        "limited_accepted_latency_ns",
        "rate_limited_latency_ns",
        "healthy_after_rate_fallback_latency_ns",
    ):
        if not _integer(observed[key], positive=True, maximum=1 << 63):
            raise ValueError("invalid stage latency")
    allocations = _expect_keys(
        observed["calling_thread_allocations"],
        {"allocations", "reallocations", "allocated_bytes", "deallocations"},
    )
    if any(not _integer(value) for value in allocations.values()):
        raise ValueError("invalid allocator counters")
    return root


def parse_capture(data: bytes) -> dict[str, Any]:
    if len(data) > MAX_CAPTURE_BYTES:
        raise ValueError("capture exceeds byte bound")
    try:
        text = data.decode("utf-8", "strict")
    except UnicodeDecodeError as exc:
        raise ValueError("capture is not UTF-8") from exc
    text = text.replace("\r\n", "\n")
    if any(separator in text for separator in ("\r", "\v", "\f", "\x85", "\u2028", "\u2029")):
        raise ValueError("capture contains a noncanonical line separator")
    lines = text.split("\n")
    if not lines or lines[-1] != "":
        raise ValueError("capture is not newline terminated")
    lines.pop()
    if len(lines) != 17 or not CARGO_FINISHED.fullmatch(lines[0]) or not CARGO_RUNNING.fullmatch(lines[1]):
        raise ValueError("cargo invocation output is incomplete or unexpected")
    if (
        lines[2] != ""
        or lines[3] != RUNNING
        or lines[4] != TEST_OK
        or lines[5] != ""
        or lines[6] != "successes:"
        or lines[7] != ""
        or lines[8] != f"---- {TEST_NAME} stdout ----"
        or not lines[9].startswith(PREFIX)
        or lines[10:12] != ["", ""]
        or lines[12] != "successes:"
        or lines[13] != f"    {TEST_NAME}"
        or lines[14] != ""
        or not SUMMARY.fullmatch(lines[15])
        or lines[16] != ""
    ):
        raise ValueError("test inventory or output block is incomplete or unexpected")
    marker = lines[9]
    if marker.count(PREFIX) != 1:
        raise ValueError("profile record is malformed")
    markers = [marker[len(PREFIX):]]
    try:
        record = json.loads(
            markers[0],
            object_pairs_hook=_pairs_without_duplicates,
            parse_constant=_reject_constant,
        )
    except (json.JSONDecodeError, ValueError, RecursionError) as exc:
        raise ValueError("profile JSON is invalid") from exc
    return _validate_record(record)


def main() -> int:
    args = sys.argv[1:]
    if len(args) != 2 or args[0] != "--cargo-exit-code" or not re.fullmatch(r"0|[1-9][0-9]{0,2}", args[1]):
        print("sf07_profile=unavailable", file=sys.stderr)
        return 2
    if args[1] != "0":
        print("sf07_profile=unavailable", file=sys.stderr)
        return 2
    data = sys.stdin.buffer.read(MAX_CAPTURE_BYTES + 1)
    try:
        record = parse_capture(data)
    except (ValueError, TypeError, RecursionError):
        print("sf07_profile=unavailable", file=sys.stderr)
        return 2
    print("SF07_PROFILE_ACCEPTED=" + json.dumps(record, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
