use std::env;
use std::fs;
use std::path::PathBuf;

const POSTGRESQL_VERSION: &str = "=17.7.0";

fn main() {
    // cfg!(target_os) in a build script reflects the HOST; use the cargo env
    // var so cross-compiles link the right frameworks.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
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

    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DESKTOP_EMBEDDED_DB");

    if env::var_os("CARGO_FEATURE_DESKTOP_EMBEDDED_DB").is_none() {
        return;
    }

    let output_path = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"))
        .join("embedded-postgresql.jar");
    if output_path.exists() {
        return;
    }

    let version = postgresql_archive::VersionReq::parse(POSTGRESQL_VERSION)
        .expect("embedded PostgreSQL version is valid");
    let (resolved_version, archive) = postgresql_archive::blocking::get_archive(
        postgresql_archive::configuration::zonky::URL,
        &version,
    )
    .expect("download the PostgreSQL runtime used by the desktop installer");

    fs::write(&output_path, archive).expect("write embedded PostgreSQL archive to OUT_DIR");
    println!("cargo:warning=Bundled PostgreSQL {resolved_version} for the desktop sidecar");
}
