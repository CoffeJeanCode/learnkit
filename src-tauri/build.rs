fn main() {
    // Windows-GNU (MSVCRT) lacks `memset_explicit`, required by libsodium
    // (stronghold engine). Provide a minimal shim for that target only.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
    {
        cc::Build::new()
            .file("shim/memset_explicit.c")
            .cargo_metadata(false)
            .compile("memset_shim");
        // Link by absolute path: `rustc-link-arg` entries are placed after
        // the rlibs, so the archive resolves `memset_explicit` references
        // from libsodium regardless of static link order.
        let out = std::env::var("OUT_DIR").expect("OUT_DIR set by cargo");
        println!("cargo:rustc-link-arg={out}/libmemset_shim.a");
    }
    tauri_build::build();

    // Windows-GNU imports `WebView2Loader.dll` dynamically (MSVC links a static
    // loader instead). `tauri-build` copies it next to the exe only if it can
    // find an already-built `webview2-com-sys` — on a clean build the app's
    // build script runs BEFORE that dependency compiles, so the copy silently
    // does nothing and the exe dies at startup with "no se encontró
    // WebView2Loader.dll". Stage it ourselves, from the crate's sources.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
    {
        stage_webview2_loader();
    }

    // tauri embeds the Windows resource (app manifest with Common-Controls v6)
    // for binary targets only. Without the manifest in test binaries the
    // loader binds comctl32 v5 and v6-only imports fail at startup
    // (STATUS_ENTRYPOINT_NOT_FOUND), so on GNU link the same resource into every
    // artifact too (`compile_for_everything` = a plain `rustc-link-arg`, which
    // also reaches `#[test]` binaries; the `-tests` flavour only targets
    // `[[test]]` integration tests, and cargo rejects it when there are none).
    //
    // GNU only. That also reaches the bin, which tauri already covered: MinGW's
    // ld merges the identical duplicate, but MSVC's cvtres does not
    // ("CVT1100: duplicate resource type:VERSION" + LNK1123), which broke the
    // release build on windows-latest. So MSVC relies on tauri's own embedding.
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
            let out = std::env::var("OUT_DIR").expect("OUT_DIR set by cargo");
            let rc = std::path::PathBuf::from(out).join("resource.rc");
            if rc.exists() {
                let _ = embed_resource::compile_for_everything(&rc, embed_resource::NONE);
            }
        }
    }
}

/// Copies `WebView2Loader.dll` (from the `webview2-com-sys` crate sources in the
/// cargo registry) to every place Windows will look for it:
/// - the profile dir (`target/<profile>/`): where `cargo run` / `tauri dev` start the exe;
/// - `target/<profile>/deps/`: where `cargo test` binaries live;
/// - `resources/`: the staging spot `tauri.gnu.conf.json` bundles into installers.
/// A miss is a build warning, never an error: the app compiles either way.
fn stage_webview2_loader() {
    use std::path::{Path, PathBuf};

    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x64",
        Ok("x86") => "x86",
        Ok("aarch64") => "arm64",
        other => {
            println!("cargo:warning=WebView2Loader.dll: unsupported architecture {other:?}, not staging it");
            return;
        }
    };

    let cargo_home = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| {
        std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(|h| PathBuf::from(h).join(".cargo"))
    });
    let Some(cargo_home) = cargo_home else {
        println!("cargo:warning=WebView2Loader.dll: cannot locate CARGO_HOME, not staging it");
        return;
    };

    // registry/src/<index>/webview2-com-sys-<version>/<arch>/WebView2Loader.dll
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(indexes) = std::fs::read_dir(cargo_home.join("registry").join("src")) {
        for index in indexes.flatten() {
            if let Ok(crates) = std::fs::read_dir(index.path()) {
                for krate in crates.flatten() {
                    let name = krate.file_name();
                    if name.to_string_lossy().starts_with("webview2-com-sys-") {
                        let dll = krate.path().join(arch).join("WebView2Loader.dll");
                        if dll.exists() {
                            candidates.push(dll);
                        }
                    }
                }
            }
        }
    }
    // Newest crate version last (same-length version strings sort correctly; ties are harmless).
    candidates.sort();
    let Some(source) = candidates.pop() else {
        println!("cargo:warning=WebView2Loader.dll not found under {}; the exe will need it next to it", cargo_home.display());
        return;
    };

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR set by cargo"));
    // OUT_DIR = <target>/<profile>/build/<pkg>-<hash>/out
    let mut dests: Vec<PathBuf> = vec![manifest_dir.join("resources")];
    if let Some(profile_dir) = out.ancestors().nth(3) {
        dests.push(profile_dir.to_path_buf());
        dests.push(profile_dir.join("deps"));
    }

    for dir in dests {
        let target: &Path = &dir;
        if std::fs::create_dir_all(target).is_err() {
            continue;
        }
        let dest = target.join("WebView2Loader.dll");
        let same = std::fs::metadata(&dest).ok().zip(std::fs::metadata(&source).ok()).is_some_and(|(a, b)| a.len() == b.len());
        if !same {
            if let Err(e) = std::fs::copy(&source, &dest) {
                println!("cargo:warning=could not copy WebView2Loader.dll to {}: {e}", dest.display());
            }
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
