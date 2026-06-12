fn main() {
    // cfg!(target_os) in a build script reflects the HOST; use the cargo env
    // var so cross-compiles link the right frameworks.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // AX* (HIServices) lives under the ApplicationServices umbrella.
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
        // NSWorkspace / NSRunningApplication — without this the ObjC classes
        // are not registered in the process and objc2::class! panics.
        println!("cargo:rustc-link-lib=framework=AppKit");
        // CGWindowListCopyWindowInfo
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        // CFString / CFArray / CFDictionary helpers
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}
