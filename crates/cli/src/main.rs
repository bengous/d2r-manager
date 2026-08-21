use std::io::Write;
use std::process::ExitCode;

use clap::Parser;

use cli::options::Options;

fn main() -> ExitCode {
    let options = Options::parse();
    let mut out = std::io::stdout().lock();
    match cli::run(&options, &mut out) {
        Ok(()) => {
            let _ = out.flush();
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("d2r-manager: {error:#}");
            ExitCode::FAILURE
        }
    }
}
