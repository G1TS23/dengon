//! `dengon-verify` — point d'entrée du binaire. Toute la logique est dans
//! la bibliothèque de la crate (`src/lib.rs`, US-305), testable sans lancer
//! de processus.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dengon_verify::run(&args, std::io::stdin().lock()) {
        Ok(report) => {
            println!("{}", report.to_json());
            ExitCode::from(dengon_verify::exit_code(report.verdict))
        }
        Err(e) => {
            eprintln!("dengon-verify : {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
