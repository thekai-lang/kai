//! Command-line surface. Hand-rolled for now — the command set is tiny;
//! revisit if subcommands/flags grow beyond a screenful.

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// `kai build <file.kai> [-o <out.ll>]`
    Build {
        input: String,
        output: Option<String>,
    },
    /// `kai run <file.kai>`
    Run {
        input: String,
    },
    Help,
    Version,
}

const USAGE: &str = "Kai compiler

USAGE:
    kai build <file.kai> [-o <out.ll>]    Compile to LLVM IR
    kai run <file.kai>                    Compile and execute via JIT
    kai --version                         Print version
    kai --help                            Print this help";

pub fn usage() -> &'static str {
    USAGE
}

/// Parses raw args (without argv[0]). Err carries the full usage text.
pub fn parse_args(args: &[String]) -> Result<Command, String> {
    match args {
        [] => Err(USAGE.to_string()),
        [flag] if flag == "--help" || flag == "-h" => Ok(Command::Help),
        [flag] if flag == "--version" || flag == "-V" => Ok(Command::Version),
        [command, rest @ ..] => match command.as_str() {
            "build" => parse_build(rest),
            "run" => parse_positional(rest, |input| Command::Run { input }),
            other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
        },
    }
}

fn parse_build(rest: &[String]) -> Result<Command, String> {
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;

    let mut idx = 0;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-o" => {
                idx += 1;
                output = Some(rest.get(idx).ok_or_else(missing_output_value)?.clone());
            }
            path if input.is_none() && !path.starts_with('-') => input = Some(path.to_string()),
            other => return Err(format!("unexpected argument `{other}`\n\n{USAGE}")),
        }
        idx += 1;
    }

    match input {
        Some(input) => Ok(Command::Build { input, output }),
        None => Err(format!("`kai build` requires an input file\n\n{USAGE}")),
    }
}

fn parse_positional(rest: &[String], make: impl Fn(String) -> Command) -> Result<Command, String> {
    match rest {
        [input] if !input.starts_with('-') => Ok(make(input.clone())),
        _ => Err(format!("expected exactly one input file\n\n{USAGE}")),
    }
}

fn missing_output_value() -> String {
    format!("`-o` requires a value\n\n{USAGE}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_build_with_output() {
        let cmd = parse_args(&args(&["build", "a.kai", "-o", "out.ll"])).unwrap();
        assert_eq!(
            cmd,
            Command::Build {
                input: "a.kai".into(),
                output: Some("out.ll".into())
            }
        );
    }

    #[test]
    fn parses_run() {
        assert_eq!(
            parse_args(&args(&["run", "a.kai"])).unwrap(),
            Command::Run {
                input: "a.kai".into()
            }
        );
    }

    #[test]
    fn rejects_o_without_value() {
        assert!(parse_args(&args(&["build", "a.kai", "-o"])).is_err());
    }

    #[test]
    fn empty_args_is_usage() {
        assert!(parse_args(&[]).is_err());
    }
}
