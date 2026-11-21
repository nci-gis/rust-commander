//! The commit this xtask is built from, for `xtask manual log`'s `built_from`: CI's
//! `GITHUB_SHA`, else `git rev-parse HEAD` with `-dirty` when the tree has changes,
//! else "unknown" (a source archive). Testers use the xtask that came with the xdevcmd
//! they test, so its commit is the build's.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    for f in ["../.git/HEAD", "../.git/index", "../.git/refs"] {
        println!("cargo:rerun-if-changed={f}");
    }
    let commit = match std::env::var("GITHUB_SHA") {
        Ok(sha) if !sha.is_empty() => sha,
        _ => match git(&["rev-parse", "HEAD"]) {
            Some(sha) => {
                let clean = Command::new("git")
                    .args(["diff", "--quiet", "HEAD", "--"])
                    .status()
                    .is_ok_and(|s| s.success());
                if clean {
                    sha
                } else {
                    format!("{sha}-dirty")
                }
            }
            None => "unknown".into(),
        },
    };
    println!("cargo:rustc-env=XTASK_COMMIT={commit}");
}
