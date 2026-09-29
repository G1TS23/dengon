//! Point d'entrée du simulateur.
//!
//! `dengon-sim [--graine N] crates/dengon-sim/scenarios/*.ron`
//! `dengon-sim --demo` — les 5 scénarios réels du DoD (US-304), narrés.
//! — toute la logique est dans [`dengon_sim::cli`], testée.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sortie = dengon_sim::cli::executer(&args);
    print!("{}", sortie.texte);
    if sortie.succes {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
