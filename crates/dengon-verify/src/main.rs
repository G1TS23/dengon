//! `dengon-verify` — point d'entrée du binaire. Toute la logique est dans
//! la bibliothèque de la crate (`src/lib.rs`, US-305), testable sans lancer
//! de processus.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = match std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect::<Result<Vec<String>, _>>()
    {
        Ok(args) => args,
        Err(_) => {
            eprintln!(
                "dengon-verify : argument non UTF-8\n{}",
                dengon_verify::USAGE
            );
            return ExitCode::from(dengon_verify::EXIT_USAGE);
        }
    };
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
