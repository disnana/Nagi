//! Prepare immutable generated/manual packages with identical runtime contracts.
//! Run: cargo run --example sqlite-public-cost -- /tmp/nagi-sqlite-cost
//! Build each package offline/release with the workspace lock, then run its binary.
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let destination = PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    if destination.exists() {
        return Err("output directory must not exist".into());
    }
    fs::create_dir_all(&destination)?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let inputs = root.join("benchmarks/sqlite-public");
    let mut high = nagic::source::load(&inputs.join("operations.nagi"), true)?;
    nagic::check::check(&mut high.program)?;
    let low = nagic::emit::low(&high.program);
    fs::write(destination.join("saved.low"), &low)?;
    let saved = nagic::source::load(&destination.join("saved.low"), false)?;
    let provenance = saved.provenance();
    let checked = nagic::check::finalize(saved.program, Default::default(), provenance)?;
    let generated = nagic::emit::rust(&checked)?;
    for (name, library) in [
        ("generated", generated),
        ("manual", fs::read_to_string(inputs.join("manual.rs"))?),
    ] {
        let package = destination.join(name);
        fs::create_dir(&package)?;
        fs::write(package.join("lib.rs"), library)?;
        fs::copy(inputs.join("main.rs"), package.join("main.rs"))?;
        fs::copy(root.join("Cargo.lock"), package.join("Cargo.lock"))?;
        let runtime = toml::Value::String(root.join("runtime").to_string_lossy().into_owned());
        fs::write(package.join("Cargo.toml"), format!(
            "[package]\nname = \"nagi-sqlite-cost-{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[lib]\nname = \"sqlite_cost\"\npath = \"lib.rs\"\n[[bin]]\nname = \"sqlite-cost-{name}\"\npath = \"main.rs\"\n[dependencies]\nnagi-runtime = {{ path = {runtime} }}\ntokio = {{ version = \"1.48\", features = [\"rt-multi-thread\", \"time\", \"sync\"] }}\nserde_json = \"1.0\"\n[profile.release]\ndebug = 0\n"
        ))?;
    }
    println!("{}", destination.display());
    Ok(())
}
