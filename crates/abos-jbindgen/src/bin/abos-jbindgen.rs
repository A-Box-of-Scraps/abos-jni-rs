//! Command-line interface to the Java binding generator.
mod cli;

fn main() -> std::process::ExitCode {
	match cli::run() {
		Ok(()) => std::process::ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("{error}");
			std::process::ExitCode::FAILURE
		}
	}
}
