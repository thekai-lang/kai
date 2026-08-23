use kai_driver::{cli, pipeline, report::render_multi};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match cli::parse_args(&args) {
        Ok(cli::Command::Build { input, output }) => build(&input, output.as_deref()),
        Ok(cli::Command::Run { input }) => run(&input),
        Ok(cli::Command::Help) => {
            print!("{}", cli::usage());
            ExitCode::SUCCESS
        }
        Ok(cli::Command::Version) => {
            println!("kai {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Err(usage) => {
            eprintln!("{usage}");
            ExitCode::from(2)
        }
    }
}

fn build(input: &str, output: Option<&str>) -> ExitCode {
    match pipeline::compile_file(Path::new(input)) {
        Ok(ir) => {
            let out_path = output
                .map(str::to_owned)
                .unwrap_or_else(|| default_output(input));
            match std::fs::write(&out_path, &ir) {
                Ok(()) => {
                    println!("wrote {out_path}");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: cannot write {out_path}: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(failure) => report_failure(&failure),
    }
}

fn run(input: &str) -> ExitCode {
    match pipeline::jit_file(Path::new(input)) {
        Ok(code) => ExitCode::from(code as u8),
        Err(failure) => report_failure(&failure),
    }
}

fn report_failure(failure: &pipeline::Failure) -> ExitCode {
    eprint!(
        "{}",
        render_multi(&failure.diagnostics, &failure.sources)
    );
    eprintln!("compilation failed at the `{}` phase", failure.phase);
    ExitCode::FAILURE
}

/// `foo/bar.kai` -> `bar.ll` next to the source.
fn default_output(input: &str) -> String {
    let stem = std::path::Path::new(input).file_stem().map_or_else(
        || "kai_out".to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    format!("{stem}.ll")
}
