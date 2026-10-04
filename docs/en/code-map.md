# Map your code

`nagic map` inspects types, module dependencies, and function calls statically. It does not build Rust or start your application. It is available from Nagi 0.1.8.

## Export a map

Inspect the [small example](../../examples/code-map/) from the repository root.

```sh
nagic map types --project examples/code-map
nagic map modules --project examples/code-map --format d2
nagic map calls --project examples/code-map --format html --output calls.html
```

The default view is `types`; the default format is Mermaid. Without `--output`, text is written to stdout. HTML is a single file that lets you inspect nodes, source locations, and relationships in a browser.

| View | Relationships |
| --- | --- |
| `types` | Classes, enums, fields, and user types in parameters and returns. Distinguishes `shared`, `view`, `owned`, Result, and nullable wrappers |
| `modules` | Dependencies between loaded files and standard modules |
| `calls` | Statically resolved function calls and Rust FFI boundaries |

## Focus a diagram

```sh
nagic map types --project examples/code-map --focus Post --depth 1
nagic map calls --project examples/code-map --module service
```

`--focus` accepts a type or function name. When a name is ambiguous, use a qualified name or ID from the diagnostic. `--depth` counts relationship hops from the focus; 0 includes only that node, and the default is 1. Currently, `--depth` requires `--focus`.

`--module` accepts an import alias, standard module name, or the full path of a loaded file. Module selection limits the graph before focus and depth are applied.

## Render with D2

Mermaid and D2 text exports need no additional tools. D2 groups nodes by module and shows the direction of dependencies and calls.

SVG and PNG require [D2](https://d2lang.com/) on PATH. Use the matching output extension. ELK is the default layout. Dagre and TALA can also be selected; rendering depends on your installed D2 and its layout engines.

```sh
nagic map types --project examples/code-map --format svg --output types.svg
nagic map modules --project examples/code-map --format png --output modules.png --layout tala
```

## Analysis scope

Checked code becomes a shared Graph IR before rendering. `--format json` exports the schema version, nodes, semantic edges, groups, and source locations. Renderers do not change type checking or generated executable code.

Callback registration through function arguments and calls through local function values do not currently produce edges. Rust function internals, runtime HTTP flow, and runtime Actor configuration are not analyzed either. Unresolved relationships produce warnings. Warnings describe the whole input and remain when focus or module filters reduce the graph. `trace`, Graph IR-based `cost`, architecture maps, and a local web server remain future work. The existing `--cost-report` retains its current format.
