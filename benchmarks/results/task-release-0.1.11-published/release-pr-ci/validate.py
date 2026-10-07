from pathlib import Path
import hashlib, json, re

folder = Path(__file__).parent
report = {"head": "c2b227533b89268028ecb2d67e39492b7aa4e4a5", "platforms": {}}
for name in ("linux-package", "macos-arm64", "macos-x86_64", "windows-x86_64"):
    file = folder / (name + ".log")
    text = re.sub(r"\x1b\[[0-9;]*m", "", file.read_text())
    text = re.sub(r"^\d{4}-\d\d-\d\dT[^ ]+Z ", "", text, flags=re.M)
    suites = []
    headers = list(re.finditer(r"^\s*(?:Running|Doc-tests) [^\n]+", text, re.M))
    for i, header in enumerate(headers):
        block = text[header.end():headers[i+1].start() if i+1 < len(headers) else len(text)]
        own = re.search(r"^running (\d+) tests?\s*$", block, re.M)
        if own is None:
            continue
        # Cargo stderr may announce the next binary before this stdout summary.
        # Its next `running N tests` is the boundary, not the stderr header.
        tail = text[header.end() + own.end():]
        next_run = re.search(r"^running \d+ tests?\s*$", tail, re.M)
        if next_run is not None:
            tail = tail[:next_run.start()]
        summary = re.search(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", tail)
        if summary is not None:
            suites.append((header.group().strip(), int(own[1]), tuple(map(int, summary.groups()))))
    native = [s for s in suites if re.search(r"tests[/\\]task_handles\.rs", s[0]) and s[1] == 8 and s[2] == (8,0,0)]
    assert len(native) == 1, (name, native)
    assert "test task_conditional_consumption::always_evaluated_receipts_and_lazy_values_run_in_three_sources ... ok" in text
    assert "test task_service_s2::typed_monitor_preserves_service_errors_shutdown_and_parent_drop ... ok" in text
    assert "Task contracts: 148/148 matched" in text
    assert "test result: ok. 17 passed; 0 failed; 0 ignored;" in text
    assert len([s for s in suites if re.search(r"tests[/\\]task_handles\.rs", s[0]) and s[1] == 3 and s[2] == (3,0,0)]) == 1, name
    assert any(s[0].startswith("Doc-tests nagi_runtime") and s[1] == 9 and s[2] == (9,0,0) for s in suites), (name, suites)
    forms = [f"Passed: {sample} ({form})" for sample in ("task-results", "supervised-service") for form in ("high", "saved-low", "handwritten-low")]
    assert all(marker in text for marker in forms), (name, forms)
    assert "Verified Task handles: extracted compiler/runtime" in text
    release_platform = "linux-x86_64" if name == "linux-package" else name
    assert f"Verified 0.1.11 {release_platform}:" in text, name
    report["platforms"][name] = {"log_sha256": hashlib.sha256(file.read_bytes()).hexdigest(), "task_contracts": "148/148", "native_suite": native[0], "new_service_oracle": True, "conditional_consumption_oracle": True, "runtime_task_tests": 17, "public_api_tests": 3, "runtime_doc_tests": 9, "example_forms": forms, "extracted_archive_version": "0.1.11", "extracted_archive_task_gate": True}
(folder / "validated.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
print(json.dumps({"validated_platforms": list(report["platforms"]), "result": "success"}))
