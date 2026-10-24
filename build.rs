fn main() {
    println!("cargo:rerun-if-env-changed=CODEX_MIGRATE_NOTICES_FILE");
    if std::env::var_os("CARGO_FEATURE_STANDALONE").is_some() {
        let source =
            std::path::PathBuf::from(std::env::var_os("CODEX_MIGRATE_NOTICES_FILE").expect(
                "standalone builds require scripts/build-standalone.ps1 to collect notices",
            ));
        println!("cargo:rerun-if-changed={}", source.display());
        let text =
            std::fs::read_to_string(&source).expect("read complete standalone license notices");
        assert!(
            !text.trim().is_empty(),
            "standalone license notices are empty"
        );
        let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(output.join("standalone-notices.txt"), text)
            .expect("embed standalone license notices");
    }

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/CodexMigrate.ico");
        resource.set("ProductName", "Codex Migrate");
        resource.set(
            "FileDescription",
            "Migrate, repair, and export local Codex sessions",
        );
        resource.set("LegalCopyright", "Copyright (c) 2026 contributors");
        resource
            .compile()
            .expect("failed to compile Windows application resources");
    }
}
