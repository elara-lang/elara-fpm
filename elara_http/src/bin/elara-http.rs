use elara_http::{DevServerConfig, parse_cli, run_cgi_once, run_dev_server};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cli = match parse_cli(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("elara-http: {e}");
            eprintln!("usage: elara-http [-S <addr>:<port>] [-t <docroot>]");
            return ExitCode::from(2);
        }
    };
    if !cli.docroot.is_dir() {
        eprintln!(
            "elara-http: document root is not a directory: {}",
            cli.docroot.display()
        );
        return ExitCode::from(1);
    }
    if let Some(listen) = cli.listen {
        let cfg = DevServerConfig {
            listen,
            docroot: cli.docroot,
        };
        if let Err(e) = run_dev_server(cfg) {
            eprintln!("elara-http: {e}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }
    // No -S: CGI mode (ScriptAlias / front-end CGI).
    let code = run_cgi_once(&cli.docroot);
    ExitCode::from(code as u8)
}
