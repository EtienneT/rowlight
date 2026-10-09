fn main() -> std::io::Result<()> {
    // Avoid Cargo's default whole-package scan: docs and dev tools are not inputs.
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=packaging/windows/icon.ico");
        // GPUI's Windows platform already embeds the execution/DPI manifest.
        // A second manifest with resource ID 1 fails to link with MSVC.
        winresource::WindowsResource::new()
            .set_icon("packaging/windows/icon.ico")
            .set("ProductName", "Rowlight")
            .set("FileDescription", "Rowlight")
            .compile()?;
    }
    Ok(())
}
