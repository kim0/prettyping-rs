use std::process::ExitCode;

use clap::error::ErrorKind;

fn main() -> ExitCode {
    match prettyping_rs::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let exit_code = normalized_exit_code(err.kind(), err.exit_code());
            if let Err(print_err) = err.print() {
                eprintln!("failed to print CLI error: {print_err}");
            }
            ExitCode::from(exit_code)
        }
    }
}

fn normalized_exit_code(kind: ErrorKind, code: i32) -> u8 {
    if kind == ErrorKind::Io {
        return 1;
    }

    match code {
        0 => 0,
        1 => 1,
        2 => 2,
        c if c > 2 => u8::try_from(c).unwrap_or(u8::MAX),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::normalized_exit_code;

    #[test]
    fn exit_code_contract_is_stable() {
        assert_eq!(normalized_exit_code(ErrorKind::DisplayHelp, 0), 0);
        assert_eq!(normalized_exit_code(ErrorKind::DisplayHelp, 1), 1);
        assert_eq!(normalized_exit_code(ErrorKind::DisplayHelp, 2), 2);
        assert_eq!(normalized_exit_code(ErrorKind::DisplayHelp, 3), 3);
        assert_eq!(normalized_exit_code(ErrorKind::DisplayHelp, -1), 1);
    }

    #[test]
    fn runtime_io_errors_are_normalized_to_exit_code_one() {
        assert_eq!(normalized_exit_code(ErrorKind::Io, 2), 1);
    }
}
