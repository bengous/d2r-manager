use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use clap::Parser;

use cli::options::Options;

fn main() -> ExitCode {
    let options = Options::parse();
    let mut out = std::io::stdout().lock();
    match cli::run(&options, &mut out).and_then(|()| Ok(out.flush()?)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|failure| failure.kind() == ErrorKind::BrokenPipe)
            {
                return ExitCode::SUCCESS;
            }
            eprintln!("d2r-manager: {error}");
            ExitCode::FAILURE
        }
    }
}
