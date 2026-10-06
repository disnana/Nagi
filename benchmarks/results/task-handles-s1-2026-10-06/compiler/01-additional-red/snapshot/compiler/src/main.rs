fn main() {
    if let Err(e) = nagic::emit::cli(std::env::args().skip(1).collect()) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
