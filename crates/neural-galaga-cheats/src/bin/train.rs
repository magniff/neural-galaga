#![recursion_limit = "256"]

fn main() {
    env_logger::init();

    // CLI: optional `--resume [PATH]`. If `--resume` is given without a path,
    // defaults to "checkpoints/cheats/best".
    let mut resume_from: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--resume" => {
                let next = args.next();
                resume_from = Some(next.unwrap_or_else(|| "checkpoints/cheats/best".to_string()));
            }
            "--help" | "-h" => {
                println!("usage: train [--resume [PATH]]");
                println!(
                    "  --resume [PATH]   continue training from PATH (default: checkpoints/cheats/best)"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("error: unknown argument {other}");
                std::process::exit(2);
            }
        }
    }

    if let Some(ref path) = resume_from {
        log::info!("neural-galaga-cheats training starting (resuming from {path})");
    } else {
        log::info!("neural-galaga-cheats training starting (fresh)");
    }

    neural_galaga_cheats::ppo::train::<burn::backend::Autodiff<burn::backend::Wgpu>>(resume_from);
}
