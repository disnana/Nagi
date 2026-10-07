fn main() {
    let args = std::env::args().skip(1).collect();
    // Expression syntax is bounded, but a flat left-associative expression can
    // still produce a deep AST without consuming parser nesting.  Keep the CLI
    // independent of the platform main-thread stack while retaining a fixed,
    // finite stack budget for all compiler passes.
    let result = match std::thread::Builder::new()
        .name("nagic".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || nagic::emit::cli(args))
    {
        Ok(worker) => match worker.join() {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        },
        Err(error) => Err(format!("compiler worker thread: {error}")),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
