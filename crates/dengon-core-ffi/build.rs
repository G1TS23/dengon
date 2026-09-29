//! Régénère `include/dengon_core.h` à chaque build de cette crate, via
//! `cbindgen` (US-307). Le header reste **versionné** : ce script ne fait
//! qu'automatiser sa régénération pour qu'il ne puisse pas diverger de
//! `src/lib.rs` sans qu'un `git diff` le montre (voir la note du module de
//! tête, et l'étape dédiée du job CI `cross-vectors`).
//!
//! cbindgen reparse le code source (`syn`), il n'invoque pas `rustc` : ce
//! script tourne donc sur l'hôte même lors d'une cross-compilation xtensa
//! (comportement standard des build scripts Cargo).

use std::env;
use std::path::PathBuf;

fn main() {
    let Ok(crate_dir) = env::var("CARGO_MANIFEST_DIR") else {
        panic!("CARGO_MANIFEST_DIR absent");
    };
    let out_header = PathBuf::from(&crate_dir).join("include/dengon_core.h");

    let config = match cbindgen::Config::from_file(PathBuf::from(&crate_dir).join("cbindgen.toml"))
    {
        Ok(config) => config,
        Err(err) => panic!("cbindgen.toml invalide : {err}"),
    };

    let bindings = match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(bindings) => bindings,
        Err(err) => panic!("échec de la génération du header cbindgen : {err}"),
    };
    bindings.write_to_file(&out_header);

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
}
