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

    // tauri embeds the Windows resource (app manifest with Common-Controls v6)
    // for binary targets only. Without the manifest in test binaries the
    // loader binds comctl32 v5 and v6-only imports fail at startup
    // (STATUS_ENTRYPOINT_NOT_FOUND), so link the same resource into every
    // target as well (merging identical resources is harmless).
    #[cfg(windows)]
    {
        let out = std::env::var("OUT_DIR").expect("OUT_DIR set by cargo");
        let rc = std::path::PathBuf::from(out).join("resource.rc");
        if rc.exists() {
            let _ = embed_resource::compile_for_everything(&rc, embed_resource::NONE);
        }
    }
}
