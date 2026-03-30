use std::fs;
use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // Copy sidecar binaries to target dir for dev mode.
    // Source files use Tauri's naming convention: {name}-{target_triple}[.exe]
    // At runtime, Tauri's sidecar() resolves to just {name}[.exe], so we
    // strip the target triple suffix when copying.
    let target_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap())
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();

    let target_triple = std::env::var("TARGET").unwrap_or_default();
    let src = PathBuf::from("binaries");
    if src.is_dir() {
        let dest = target_dir.join("binaries");
        fs::create_dir_all(&dest).ok();
        if let Ok(entries) = fs::read_dir(&src) {
            for entry in entries.flatten() {
                let from = entry.path();
                if from.is_file() {
                    // Strip target triple: mihomo-x86_64-pc-windows-msvc.exe → mihomo.exe
                    let name = entry.file_name().to_string_lossy().to_string();
                    let stripped = name.replace(&format!("-{target_triple}"), "");
                    let to = dest.join(&stripped);
                    if needs_copy(&from, &to) {
                        fs::copy(&from, &to).ok();
                    }
                }
            }
        }
    }
}

fn needs_copy(src: &PathBuf, dst: &PathBuf) -> bool {
    match (fs::metadata(src), fs::metadata(dst)) {
        (Ok(s), Ok(d)) => s.len() != d.len() || s.modified().ok() != d.modified().ok(),
        _ => true,
    }
}
