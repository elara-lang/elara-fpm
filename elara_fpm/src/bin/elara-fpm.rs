use elara_fpm::{
    DevServerConfig, FastCgiConfig, parse_cli, run_cgi_once_with, run_dev_server,
    run_fastcgi_worker, usage_text,
};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cli = match parse_cli(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("elara-fpm: {e}");
            eprint!("{}", usage_text());
            return ExitCode::from(2);
        }
    };
    if cli.help {
        print!("{}", usage_text());
        return ExitCode::SUCCESS;
    }
    if !cli.docroot.is_dir() {
        eprintln!(
            "elara-fpm: document root is not a directory: {}",
            cli.docroot.display()
        );
        return ExitCode::from(1);
    }
    if let Some(fcgi) = cli.fastcgi {
        let cfg = FastCgiConfig {
            listen: fcgi,
            docroot: cli.docroot,
            process: cli.process,
        };
        if let Err(e) = run_fastcgi_worker(&cfg) {
            eprintln!("elara-fpm: {e}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }
    if let Some(listen) = cli.listen {
        let cfg = DevServerConfig {
            listen,
            docroot: cli.docroot,
            process: cli.process,
        };
        if let Err(e) = run_dev_server(cfg) {
            eprintln!("elara-fpm: {e}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }
    let code = run_cgi_once_with(&cli.docroot, &cli.process);
    ExitCode::from(code as u8)
}
