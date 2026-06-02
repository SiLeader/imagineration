use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

const DIST_DIR: &str = "dist";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let source_dir = manifest_dir.join("web");
    let dist_dir = manifest_dir.join(DIST_DIR);
    let index = dist_dir.join("index.html");

    println!("cargo:rerun-if-changed={}", source_dir.display());

    if should_build_frontend(&source_dir, &index).expect("check frontend asset freshness") {
        let pnpm = which::which("pnpm").expect("pnpm");
        run_pnpm(
            &pnpm,
            &source_dir,
            &["install"],
            "install frontend dependencies",
        );
        run_pnpm(
            &pnpm,
            &source_dir,
            &["run", "build"],
            "build frontend assets",
        );
    }

    let mut files = Vec::new();
    collect_files(&dist_dir, &dist_dir, &mut files).expect("collect frontend assets");
    files.sort();

    let generated = render_manifest(&files);
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("asset_manifest.rs"), generated).expect("write asset manifest");
}

fn should_build_frontend(source_dir: &Path, index: &Path) -> io::Result<bool> {
    let index_modified = index
        .metadata()
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    has_newer_source(source_dir, index_modified)
}

fn has_newer_source(dir: &Path, index_modified: SystemTime) -> io::Result<bool> {
    let mut has_newer = false;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if should_skip_source_path(&path) {
            continue;
        }
        if entry.file_type()?.is_dir() {
            if has_newer_source(&path, index_modified)? {
                has_newer = true;
            }
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let modified = entry
            .metadata()?
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if modified > index_modified {
            has_newer = true;
        }
    }
    Ok(has_newer)
}

fn should_skip_source_path(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("node_modules" | ".svelte-kit")
    )
}

fn run_pnpm(pnpm: &Path, source_dir: &Path, args: &[&str], description: &str) {
    let status = Command::new(pnpm)
        .args(args)
        .current_dir(source_dir)
        .status()
        .unwrap_or_else(|error| panic!("{description}: {error}"));
    assert!(status.success(), "{description}: {status}");
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }

        println!("cargo:rerun-if-changed={}", path.display());
        let relative = path.strip_prefix(root).expect("asset path below root");
        files.push(slash_path(relative));
    }
    Ok(())
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn render_manifest(files: &[String]) -> String {
    let mut source = String::from(
        "pub(crate) struct EmbeddedAsset {\n    pub(crate) path: &'static str,\n    pub(crate) mime: &'static str,\n    pub(crate) bytes: &'static [u8],\n}\n\npub(crate) static ASSETS: &[EmbeddedAsset] = &[\n",
    );

    for path in files {
        source.push_str("    EmbeddedAsset {\n");
        source.push_str(&format!("        path: {},\n", rust_string(path)));
        source.push_str(&format!(
            "        mime: {},\n",
            rust_string(mime_type(path))
        ));
        source.push_str(&format!(
            "        bytes: include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{DIST_DIR}/\", {})),\n",
            rust_string(path)
        ));
        source.push_str("    },\n");
    }

    source.push_str("];\n");
    source
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

fn mime_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}
