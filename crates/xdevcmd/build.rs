//! Windows only: put the icon, and the name, version and description from Cargo.toml,
//! into xdevcmd.exe, so Explorer, the taskbar and the file's Properties show them.
//!
//! It needs the Windows resource compiler: `rc.exe` from the Windows SDK for an MSVC
//! build, `windres` from MinGW for a GNU one. Without it the build still works, with a
//! warning and no icon, so `cargo check --target x86_64-pc-windows-gnu` from Linux does
//! not need MinGW. Release and CI set `XDEVCMD_REQUIRE_ICON`, which makes it an error.

use std::path::PathBuf;

fn main() {
    let icon = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/xdevcmd.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-env-changed=XDEVCMD_REQUIRE_ICON");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&icon.to_string_lossy());
    res.set("ProductName", "xdevcmd");
    res.set("FileDescription", env!("CARGO_PKG_DESCRIPTION"));
    res.set("LegalCopyright", "MIT licence, XDevTeam");
    if let Err(e) = res.compile() {
        if std::env::var_os("XDEVCMD_REQUIRE_ICON").is_some_and(|v| !v.is_empty()) {
            panic!("could not put the icon into xdevcmd.exe: {e}");
        }
        println!("cargo:warning=xdevcmd.exe is built without its icon: {e}");
    }
}
