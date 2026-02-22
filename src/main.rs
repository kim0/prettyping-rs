use std::process::ExitCode;

fn main() -> ExitCode {
    match prettyping_rs::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let exit_code = normalized_exit_code(err.exit_code());
            if let Err(print_err) = err.print() {
                eprintln!("failed to print CLI error: {print_err}");
            }
            ExitCode::from(exit_code)
        }
    }
}

fn normalized_exit_code(code: i32) -> u8 {
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
    use super::normalized_exit_code;

    #[test]
    fn exit_code_contract_is_stable() {
        assert_eq!(normalized_exit_code(0), 0);
        assert_eq!(normalized_exit_code(1), 1);
        assert_eq!(normalized_exit_code(2), 2);
        assert_eq!(normalized_exit_code(3), 3);
        assert_eq!(normalized_exit_code(-1), 1);
    }
}
