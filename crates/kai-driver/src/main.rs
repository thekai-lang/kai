use kai_driver::{cli, pipeline, report::render_multi};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match cli::parse_args(&args) {
        Ok(cli::Command::Build { input, output }) => build(&input, output.as_deref()),
        Ok(cli::Command::Run { input }) => run(&input),
        Ok(cli::Command::Check { input, schema }) => check(&input, schema),
        Ok(cli::Command::SyncSqlPostgres { version, conn_str }) => sync_sql_postgres(version, &conn_str),
        Ok(cli::Command::SyncApiOpenapi { version, url, service }) => sync_api_openapi(version, &url, &service),
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


fn check(input: &str, schema: bool) -> ExitCode {
    match pipeline::check_file(Path::new(input), schema) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => report_failure(&failure),
    }
}

fn sync_sql_postgres(version: u32, conn_str: &str) -> ExitCode {
    // Generate snapshot output path: .kai/snapshots/sql/v{version}.json
    let out_dir = Path::new(".kai/snapshots/sql");
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        eprintln!("Failed to create snapshot directory: {}", e);
        return ExitCode::from(1);
    }
    let out_path = out_dir.join(format!("v{}.json", version));

    match kai_sync::postgres::sync_schema(version, conn_str, out_path.to_str().unwrap()) {
        Ok(_) => {
            println!("Snapshot v{} generated successfully at {}", version, out_path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kai sync error: {}", e);
            ExitCode::from(1)
        }
    }
}

fn sync_api_openapi(version: u32, url: &str, service: &str) -> ExitCode {
    let out_dir = Path::new(".kai/snapshots/api").join(service);
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("Failed to create snapshot directory: {}", e);
        return ExitCode::from(1);
    }
    let out_path = out_dir.join(format!("v{}.json", version));

    match kai_sync::openapi::sync_openapi(version, url, service, out_path.to_str().unwrap()) {
        Ok(_) => {
            println!("Snapshot v{} for service '{}' generated successfully at {}", version, service, out_path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kai sync error: {}", e);
            ExitCode::from(1)
        }
    }
}
