# Independent final review evidence preserved from tool outputs

Reviewer: `/root/sf01_boundary_review`, 2026-10-08 UTC. SF01 reviewed source `f9ec01d431d7650f33cd9fe729881b4ce8a5d6b3` plus dirty result/benchmark documentation.

These files were saved after the review at the parent request, by copying the already executed tool outputs preserved in the reviewer conversation. Tests were not rerun. They are transcript-derived evidence, not a claim of original shell redirection. `commands-and-exits.json` records actual commands, working directories, exit codes, tool chunk IDs, permissions, child command statuses, and capture limits.

The HTTP sandbox failure log omits repeated identical bind-error blocks and is explicitly an excerpt. Remaining execution logs concatenate the already returned output chunks. The historical fixture command originally flattened diagnostic newlines and the persisted log retains that form. Source-only reads are omitted where an excerpt is identified. Original full outputs remain in the conversation tool transcript.

The initial wrong Cargo package name is an invocation error. Initial socket failures are sandbox infra errors. Added network permission enabled the authorized owned-loopback tests; subsequent HTTP/native/Task successes are recorded separately. The final auth 9, HTTP 11, compiler 10, native 3-path group, Task 8, manual Clone 2, and capability unit results are retained.

The cost recalculation is read-only, not remeasurement. Its fixture hash mismatch was isolated to the runner hard-cap change during review; the actual fixture hashes matched. The results README/provenance were subsequently read back and correctly distinguish the old measured runner hash from the post-measurement runner hash, retain 62 observed requests, and disclose no original runner snapshot was retained/found. Latest four-OS CI and the whole workspace were not independently rerun by this reviewer.

This directory selects only SF01 evidence from the original reviewer preservation. Other independent work is retained with its separate PR; source logs have not been rewritten. The selected ledger and README are scope metadata, and their hashes are regenerated here.
