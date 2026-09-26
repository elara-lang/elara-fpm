use elara_http::{DevServerConfig, parse_cli, run_dev_server};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cli = match parse_cli(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("elara-http: {e}");
            eprintln!("usage: elara-http -S <addr>:<port> [-t <docroot>]");
            return ExitCode::from(2);
        }
    };
    let Some(listen) = cli.listen else {
        eprintln!("elara-http: -S <addr>:<port> is required");
        eprintln!("usage: elara-http -S <addr>:<port> [-t <docroot>]");
        return ExitCode::from(2);
    };
    if !cli.docroot.is_dir() {
        eprintln!(
            "elara-http: document root is not a directory: {}",
            cli.docroot.display()
        );
        return ExitCode::from(1);
    }
    let cfg = DevServerConfig {
        listen,
        docroot: cli.docroot,
    };
    if let Err(e) = run_dev_server(cfg) {
        eprintln!("elara-http: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
