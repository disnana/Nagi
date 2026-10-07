# Task results, move, and business Err

This example uses Task result handles published in Nagi 0.1.11. Confirm that the installed compiler is version 0.1.11 or later. Run `nagic check` and `nagic run` from this directory. Run the handwritten Low with `nagic run main.low`.

The example transfers `original` and its receipt obligation with `move`, then awaits once to receive 42. The next Task returns a business Err. Separate matches handle the outer TaskFailure and the inner Result, so the business Err lets its sibling finish. The unit sibling is explicitly discarded. Discard abandons receipt and does not stop the child; the scope still waits for actual termination.

`42`, `business sentinel`, and `sibling finished` appear before `after scope` and `task-results: OK`. The relative order of the sibling and business Err messages is unspecified. Printing a TaskFailure leaves the scope failed, preventing the final success messages.

For saved Low, run `nagic lower --out build/lowered`, then `nagic run build/lowered/generated.low`. Verification builds and runs saved Low from a copy with the original High removed, separately from handwritten Low.

[Task guide](../../../docs/en/task-handles.md) · [日本語](README.md)
