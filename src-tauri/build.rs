fn main() {
    // kAXTrustedCheckOptionPrompt / AXIsProcessTrustedWithOptions live in
    // HIServices under the ApplicationServices umbrella; link explicitly so
    // the data symbol resolves regardless of what tauri links transitively.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // AX* (HIServices) + CFString/CFDictionary helpers
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        // CGWindowListCopyWindowInfo
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        // NSWorkspace / NSRunningApplication (objc2::class! lookups)
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
    tauri_build::build()
}
