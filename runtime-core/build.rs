fn main() {
    // Package identity selects the implementation even with --no-default-features.
    // The original nagi-runtime never emits this cfg and keeps its complete API.
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(nagi_runtime_core)");
    println!("cargo::rustc-cfg=nagi_runtime_core");
}
