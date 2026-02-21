use std::process::ExitCode;

fn main() -> ExitCode {
    match prettyping_rs::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let exit_code = err.exit_code();
            if let Err(print_err) = err.print() {
                eprintln!("failed to print CLI error: {print_err}");
            }
            let exit_code_u8 = u8::try_from(exit_code).unwrap_or(1);
            ExitCode::from(exit_code_u8)
        }
    }
}
