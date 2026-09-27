fn main() {
    slint_build::compile("ui/main.slint").expect("failed to compile Slint UI");

    // Windows-only: embed the application icon + version-info resource into the executable
    // (Stage 13 production shell). No-op on other targets; embed-resource itself is only a
    // build-dependency on cfg(windows) (see Cargo.toml), so this line only compiles there too.
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=assets/branding/app.rc");
        println!("cargo:rerun-if-changed=assets/branding/icon.ico");
        embed_resource::compile("assets/branding/app.rc", embed_resource::NONE)
            .manifest_required()
            .expect("failed to embed Windows application icon/version resource");
    }
}
