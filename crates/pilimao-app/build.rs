fn main() {
    println!("cargo:rerun-if-changed=../../assets/pilimao.ico");

    // Embed application icon and Windows Explorer version metadata when building on Windows.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/pilimao.ico");
        res.set("ProductName", "Pilimao Music Player");
        res.set(
            "FileDescription",
            "Pilimao - Windows 10/11 Native Rust Music Player (Classic Winamp 2.x Homage)",
        );
        res.set(
            "LegalCopyright",
            "Copyright (c) 2026 sonwant-git (MIT License)",
        );
        res.set("OriginalFilename", "pilimao.exe");
        if let Err(err) = res.compile() {
            println!("cargo:warning=failed to embed Windows icon/metadata: {err}");
        }
    }
}
