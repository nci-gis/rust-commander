//! What a release ships besides the program: the files `cargo package` takes from each
//! crate. Each test reads files outside this crate and is skipped where they are absent,
//! as in a crate downloaded from crates.io.

use std::fs;
use std::path::{Path, PathBuf};

fn top() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// crates.io gets each crate's directory and nothing above it, so each crate carries its
/// own LICENSE. A copy that drifts from the repository's one is a wrong licence.
#[test]
fn each_crate_ships_the_top_level_licence() {
    let Ok(want) = fs::read_to_string(top().join("LICENSE")) else {
        eprintln!("no top-level LICENSE here; skipped");
        return;
    };
    for krate in ["xdevcmd", "xdevcmd-core"] {
        let p = top().join("crates").join(krate).join("LICENSE");
        let have = fs::read_to_string(&p).unwrap_or_default();
        assert_eq!(
            have,
            want,
            "{} differs from LICENSE: copy it again",
            p.display()
        );
    }
}

/// The archive name release.yml gives a target for a tag, read from its `package` step,
/// so a change there that the Scoop and winget files do not follow fails here.
fn released_archive(tag: &str, target: &str) -> Option<String> {
    let yml = fs::read_to_string(top().join(".github/workflows/release.yml")).ok()?;
    assert!(
        yml.contains(&format!("target: {target} }}")),
        "release.yml no longer builds {target}"
    );
    let line = yml
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("name=\"xdevcmd-"))
        .expect("release.yml's package step names the archive");
    let pattern = line
        .strip_prefix("name=\"")
        .and_then(|l| l.strip_suffix('"'))
        .expect("name=\"…\"");
    assert!(
        yml.contains("7z a -tzip \"$name.zip\" \"$name\""),
        "the Windows archive is no longer $name.zip holding the folder $name"
    );
    Some(
        pattern
            .replace("${GITHUB_REF_NAME}", tag)
            .replace("${{ matrix.target }}", target),
    )
}

const WINDOWS: &str = "x86_64-pc-windows-msvc";

fn download(tag: &str, file: &str) -> String {
    format!(
        "{}/releases/download/{tag}/{file}",
        env!("CARGO_PKG_REPOSITORY")
    )
}

#[test]
fn the_scoop_manifest_points_at_the_archive_release_yml_makes() {
    let Some(folder) = released_archive("v$version", WINDOWS) else {
        eprintln!("no release.yml here; skipped");
        return;
    };
    let text = fs::read_to_string(top().join("packaging/scoop/xdevcmd.json")).unwrap();
    let m: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(m["bin"], "xdevcmd.exe");
    assert_eq!(m["homepage"], env!("CARGO_PKG_REPOSITORY"));
    assert_eq!(m["license"], env!("CARGO_PKG_LICENSE"));
    assert_eq!(m["description"], env!("CARGO_PKG_DESCRIPTION"));

    // The pattern Scoop fills for each new release.
    let auto = &m["autoupdate"]["architecture"]["64bit"];
    assert_eq!(auto["url"], download("v$version", &format!("{folder}.zip")));
    assert_eq!(auto["extract_dir"], folder.as_str());
    assert_eq!(auto["hash"]["url"], download("v$version", "SHA256SUMS"));

    // The filled-in version is that pattern with its version.
    let v = m["version"].as_str().unwrap();
    let now = &m["architecture"]["64bit"];
    let folder_v = folder.replace("$version", v);
    assert_eq!(
        now["url"],
        download(&format!("v{v}"), &format!("{folder_v}.zip"))
    );
    assert_eq!(now["extract_dir"], folder_v.as_str());
}

#[test]
fn the_winget_installer_points_at_the_archive_release_yml_makes() {
    let Some(folder) = released_archive("v<VERSION>", WINDOWS) else {
        eprintln!("no release.yml here; skipped");
        return;
    };
    let dir = top().join("packaging/winget");
    // Checked out on Windows, git may give the file CRLF line ends (core.autocrlf, as
    // on the CI's runner); winget reads either, so compare the lines, not the ends.
    let installer = fs::read_to_string(dir.join("XDevTeam.xdevcmd.installer.yaml"))
        .unwrap()
        .replace("\r\n", "\n");
    let url = download("v<VERSION>", &format!("{folder}.zip"));
    assert!(
        installer.contains(&format!("InstallerUrl: {url}\n")),
        "want {url}\n{installer}"
    );
    let exe = format!("RelativeFilePath: {folder}\\xdevcmd.exe\n");
    assert!(installer.contains(&exe), "want {exe}\n{installer}");
    for f in ["", ".installer", ".locale.en-US"] {
        let text = fs::read_to_string(dir.join(format!("XDevTeam.xdevcmd{f}.yaml")))
            .unwrap()
            .replace("\r\n", "\n");
        assert!(
            text.contains("PackageIdentifier: XDevTeam.xdevcmd\n"),
            "{f}"
        );
        assert!(text.contains("PackageVersion: <VERSION>\n"), "{f}");
    }
}

/// Every target release.yml builds, as its matrix names them.
const TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    WINDOWS,
];

/// `cargo binstall xdevcmd` reads `[package.metadata.binstall]` in this crate's
/// Cargo.toml. Filled in as binstall fills it, for each target, it must name the archive
/// release.yml uploads and the program inside it.
#[test]
fn cargo_binstall_finds_the_archive_release_yml_makes() {
    let text =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let cargo: toml::Table = toml::from_str(&text).unwrap();
    let meta = &cargo["package"]["metadata"]["binstall"];
    for target in TARGETS {
        let Some(folder) = released_archive("v0.3.0", target) else {
            eprintln!("no release.yml here; skipped");
            return;
        };
        let (suffix, format, ext) = if *target == WINDOWS {
            (".zip", "zip", ".exe")
        } else {
            (".tar.gz", "tgz", "")
        };
        let key = |k: &str| {
            meta.get("overrides")
                .and_then(|o| o.get(*target))
                .and_then(|o| o.get(k))
                .or_else(|| meta.get(k))
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("no {k} for {target}"))
                .to_string()
        };
        let fill = |s: String| {
            s.replace("{ repo }", env!("CARGO_PKG_REPOSITORY"))
                .replace("{ version }", "0.3.0")
                .replace("{ name }", env!("CARGO_PKG_NAME"))
                .replace("{ target }", target)
                .replace("{ archive-suffix }", suffix)
                .replace("{ bin }", "xdevcmd")
                .replace("{ binary-ext }", ext)
        };
        assert_eq!(key("pkg-fmt"), format, "{target}");
        assert_eq!(
            fill(key("pkg-url")),
            download("v0.3.0", &format!("{folder}{suffix}")),
            "{target}"
        );
        assert_eq!(
            fill(key("bin-dir")),
            format!("{folder}/xdevcmd{ext}"),
            "{target}"
        );
    }
}

/// Every archive carries `docs/` beside the binary and `themes/` (Help → Documentation
/// names it first), and its index, `docs/README.md`, lists every page.
#[test]
fn every_archive_carries_the_docs_beside_the_binary() {
    let Ok(yml) = fs::read_to_string(top().join(".github/workflows/release.yml")) else {
        eprintln!("no release.yml here; skipped");
        return;
    };
    let package = yml
        .split("- name: package")
        .nth(1)
        .and_then(|s| s.split("- uses:").next())
        .expect("release.yml has a package step");
    for line in [
        "cp -r themes \"$name/\"",
        "mkdir \"$name/docs\"",
        "cp docs/*.md \"$name/docs/\"",
    ] {
        assert!(package.contains(line), "the package step lost: {line}");
    }
    // Before the archive is made, for both kinds of archive.
    let docs_at = package.find("cp docs/*.md").unwrap();
    assert!(docs_at < package.find("7z a").unwrap());
    assert!(docs_at < package.find("tar czf").unwrap());

    let index = fs::read_to_string(top().join("docs/README.md")).expect("docs/README.md");
    for e in fs::read_dir(top().join("docs")).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.ends_with(".md") && name != "README.md" {
            assert!(
                index.contains(&format!("]({name})")),
                "docs/README.md does not list {name}"
            );
        }
    }
}
