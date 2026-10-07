# Workspace final result summary and raw-log locator

- Source semantic code: 0bebcd0 / e3e0ea3 final.
- workspace-tests-final.log exited 0.
- Parent's raw-log summary: 95 result blocks, 970 passed, 0 failed, 1 ignored.
- The pass count is a simple textual aggregate and includes child-process result duplication. It is not a count of unique test cases and is not itself a quality claim.
- Raw-log SHA-256: dfaf7dc623b541bb4fbf7dcf886bfc6b4a86e548707498fcfdb6a8c8814dfd8d. The complete raw log is preserved as [mtime-zero gzip](workspace-tests-final.log.gz); compressed SHA-256: 1fc7ac633129c3d78f1e8e7a3f2d1e424ef3fdb7fd6b9846f505b8996ebd3905.
- Final fmt and workspace clippy passed. The [clippy log](workspace-clippy-final.log) is copied separately; the fmt log was empty (SHA-256 is the empty-file digest).
- Subsequent local Linux source validation passed release build, 10 example projects / 19 runs, no-SQL-feature tests, seeded fuzz smoke (1,000 mutations, 16 bounded native runs, 77 checked Low emit mutations, zero panics), SQLite High/saved-Low native sample, and local linux-x86_64 archive extraction/acceptance. These results are in the adjacent selected logs; they are separate from this workspace test log.
- PR #99 initial e3e0ea3 checks are running and latest-head 4-OS CI has not been read back. Generated-versus-manual cost measurement is complete; see ../cost-summary.json.
