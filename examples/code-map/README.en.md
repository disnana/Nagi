# Code map example

[日本語](README.md)

`Post` holds a shared reference to `User`, and `service.nagi` creates a post. `main.nagi` calls both service functions and prints `Nagi`.

Run from the repository root.

```sh
nagic run --project examples/code-map
nagic map types --project examples/code-map --format mermaid
nagic map modules --project examples/code-map --format d2
nagic map calls --project examples/code-map --format html --output calls.html
```

| View | Rendered D2 example | Mermaid | D2 |
| --- | --- | --- | --- |
| Types, parameters, returns | [types.svg](visuals/types.svg) | [types.mmd](visuals/types.mmd) | [types.d2](visuals/types.d2) |
| Imports and module use | [modules.svg](visuals/modules.svg) | [modules.mmd](visuals/modules.mmd) | [modules.d2](visuals/modules.d2) |
| Function calls | [calls.svg](visuals/calls.svg) | [calls.mmd](visuals/calls.mmd) | [calls.d2](visuals/calls.d2) |

The SVG examples use D2 0.9.0 with TALA. Renderers and layout engines determine placement; the Graph IR stores type and call relationships. See [formats, filters, and analysis scope](../../docs/en/code-map.md).
