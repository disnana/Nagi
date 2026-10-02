//! Locate the runtime shipped with the compiler, independently of the project.
use std::{
    fs,
    path::{Path, PathBuf},
};

fn valid(root: &Path) -> bool {
    root.join("runtime/Cargo.toml").is_file()
}

pub(crate) fn root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    if let Some(path) = std::env::var_os("NAGI_ROOT") {
        let root = cwd.join(path);
        if !valid(&root) {
            return Err(format!(
                "NAGI_ROOTにruntime/Cargo.tomlがありません: {}\nNAGI_ROOTにはnagicのあるフォルダーではなく、runtime/を含むNagiの展開フォルダーを指定してください。通常の配布版ではNAGI_ROOTを解除すると自動で見つけます。",
                root.display()
            ));
        }
        return fs::canonicalize(root).map_err(|e| e.to_string());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let root = exe.parent().and_then(|p| p.ancestors().find(|p| {
        valid(p) && (p.join("release.json").is_file() || p.join("compiler/Cargo.toml").is_file())
    })).or_else(|| cwd.ancestors().find(|p| valid(p)))
        .or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).parent().filter(|p| valid(p)))
        .ok_or_else(|| "Nagiのruntime/が見つかりません。配布物全体を展開し、nagicとruntime/の位置を保ってください。コンパイラだけを移した場合はNAGI_ROOTにNagiの展開フォルダーを指定してください。".to_string())?;
    fs::canonicalize(root).map_err(|e| e.to_string())
}
