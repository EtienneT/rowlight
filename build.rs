fn main() -> std::io::Result<()> {
    // Avoid Cargo's default whole-package scan: docs and dev tools are not inputs.
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=packaging/windows/icon.ico");
        println!("cargo:rerun-if-changed=packaging/windows/app.manifest");
        winresource::WindowsResource::new()
            .set_icon("packaging/windows/icon.ico")
            .set("ProductName", "Parquet Viewer")
            .set("FileDescription", "Parquet Viewer")
            .set_manifest(include_str!("packaging/windows/app.manifest"))
            .compile()?;
    }
    Ok(())
}
