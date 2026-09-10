//! Point d'entrée du simulateur.
//!
//! Invocation visée : `dengon-sim crates/dengon-sim/scenarios/direct.ron`.
//! La lecture des scénarios (`ron`) arrive avec P1.12.

fn main() {
    println!(
        "dengon-sim {} — squelette (graine {:#x})",
        env!("CARGO_PKG_VERSION"),
        dengon_sim::SEED_PAR_DEFAUT
    );
}
