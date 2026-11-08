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

/// `src` zlib-compressed (best level) into `dest`; returns `dest`.
fn compressed(src: &Path, dest: &Path) -> PathBuf {
    let data = std::fs::read(src).unwrap();
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::write(dest, miniz_oxide::deflate::compress_to_vec_zlib(&data, 10)).unwrap();
    dest.to_path_buf()
}

fn main() {
    // Fluent on both platforms: it follows the system light/dark setting. The @tr strings'
    // translations are bundled from lang/ (tools/i18n.py), without the component name as
    // context: src/i18n.rs reads the same catalogs.
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".into())
        .with_bundled_translations("lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();

    // Embed the generated game assets (app/assets/gen, made by tools/build_assets.py).
    let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/gen");
    println!("cargo:rerun-if-changed={}", pack.display());
    let mut files = vec![];
    walk(&pack, &pack, &mut files);
    // (name, zlib-compressed, bytes): the JSON name tables compress about 5x; the PNGs
    // are compressed already and go in as they are.
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut src = String::from("pub static FILES: &[(&str, bool, &[u8])] = &[\n");
    for (rel, p) in &files {
        println!("cargo:rerun-if-changed={}", p.display());
        let (z, path) =
            if rel.ends_with(".json") { (true, compressed(p, &out.join("gen").join(format!("{rel}.z")))) } else { (false, p.canonicalize().unwrap()) };
        writeln!(src, "    ({rel:?}, {z}, include_bytes!({path:?})),").unwrap();
    }
    src.push_str("];\n");
    if files.is_empty() {
        println!("cargo:warning=no game assets in {}: building with placeholders", pack.display());
    }
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("assets.rs"), src).unwrap();

    // The .po catalogs src/i18n.rs parses (Slint bundles its own copy from lang/ above).
    for e in std::fs::read_dir("lang").unwrap().flatten() {
        let po = e.path().join("LC_MESSAGES/mhgu-editor.po");
        if po.is_file() {
            println!("cargo:rerun-if-changed={}", po.display());
            compressed(&po, &out.join("po").join(format!("{}.po.z", e.file_name().to_string_lossy())));
        }
    }

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
