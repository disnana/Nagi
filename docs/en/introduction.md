# Goals and implementation scope

Nagi aims to reduce the cost of rebuilding the same data as different general-purpose objects at HTTP, JSON, database, and task boundaries. High is the language used to write applications; Low lets you inspect and adjust generated code.

High uses indentation for readability. It does not use dynamic Python objects or provide Python compatibility. Low uses braces, explicit types, and declarations. Generated and handwritten Low go through the same parser and checker.

The working path in 0.1 is High → Low text → Low AST → type checking → Rust → native executable. Working examples cover CRUD APIs with HTTP, JSON, and SQLite, CPU loops, tasks, actor communication, and worker panic recovery. Each page identifies features that remain design proposals.

The main ideas being tested are readable Low, function replacement that preserves types, visible costs for views and owned values, and backend paths that connect directly to typed models. The current implementation does not establish a new memory management system or fault isolation comparable to BEAM.

Start with the [Docs contents](README.md), [setup and first run](getting-started.md), and [language guide](language-guide.md). Use the [syntax reference](syntax.md) to look up notation and the [measurements](measurements.md) for benchmark results.
