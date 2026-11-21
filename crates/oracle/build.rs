//! Embeds the repository root this crate is compiled from (`KARN_ROOT`), where the
//! pinned corpus and decks are read. The checkout identity from `.cargo/config.toml` is
//! tracked so that building this checkout recompiles over artifacts of another one.
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=KARN_CHECKOUT");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest.ancestors().find(|d| d.join("resources/oracle/cards.jsonl").exists()).unwrap_or(&manifest);
    if let Ok(checkout) = std::env::var("KARN_CHECKOUT") {
        let same = |a: &Path, b: &Path| a.canonicalize().ok().is_some_and(|a| b.canonicalize().ok() == Some(a));
        assert!(same(Path::new(&checkout), root),
            "cargo ran with the configuration of {} but compiles the sources of {}: run cargo from the checkout it builds",
            Path::new(&checkout).canonicalize().unwrap_or_else(|_| checkout.clone().into()).display(), root.display());
    }
    claim_target(root);
    println!("cargo:rustc-env=KARN_ROOT={}", root.display());
}

/// The target directory names the checkout it was first built from (`karn-checkout`,
/// as scripts/rust_tools.py writes it): building another checkout into it is refused, since
/// cargo would otherwise reuse this checkout's compiled crates there. This runs only when
/// this script does; the scripts check before every build.
fn claim_target(root: &Path) {
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    // the target root: where a marker or cargo's own tag already is, else four levels up from
    // <target>/<profile>/build/<crate>-<hash>/out (a directory a script made has no tag yet)
    let target = out.ancestors()
        .find(|d| d.join("karn-checkout").exists() || d.join("CACHEDIR.TAG").exists())
        .or_else(|| out.ancestors().nth(4));
    let Some(target) = target else { return };
    let marker = target.join("karn-checkout");
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    match std::fs::read_to_string(&marker) {
        Ok(owner) => {
            let owner = owner.trim();
            assert!(Path::new(owner).canonicalize().ok().as_deref() == Some(root.as_path()),
                "{} was built from {owner}, not this checkout {}: build each checkout into its own target directory",
                target.display(), root.display());
        }
        Err(_) => std::fs::write(&marker, format!("{}\n", root.display())).expect("write the target's checkout marker"),
    }
}
