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
        assert_eq!(have, want, "{} differs from LICENSE: copy it again", p.display());
    }
}
