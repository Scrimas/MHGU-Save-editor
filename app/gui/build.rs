use std::fmt::Write;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(&p, root, out);
        } else {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push((rel, p));
        }
    }
}

fn main() {
    // Fluent on both platforms: it follows the system light/dark setting.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/app.slint", config).unwrap();

    // Embed the generated game assets (app/assets/gen, made by tools/build_assets.py).
    let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/gen");
    println!("cargo:rerun-if-changed={}", pack.display());
    let mut files = vec![];
    walk(&pack, &pack, &mut files);
    let mut src = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (rel, p) in &files {
        println!("cargo:rerun-if-changed={}", p.display());
        writeln!(src, "    ({rel:?}, include_bytes!({:?})),", p.canonicalize().unwrap()).unwrap();
    }
    src.push_str("];\n");
    if files.is_empty() {
        println!("cargo:warning=no game assets in {}: building with placeholders", pack.display());
    }
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("assets.rs"), src).unwrap();

    // Windows: icon and DPI-aware manifest.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_manifest(include_str!("windows/app.manifest"));
        let ico = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon.ico");
        if ico.exists() {
            res.set_icon(ico.to_str().unwrap());
        }
        res.compile().unwrap();
    }
}
