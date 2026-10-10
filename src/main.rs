#![deny(unsafe_op_in_unsafe_fn)]

use local_stt::{autostart, download, hwprobe, models, paths};
use std::io::Write;
use std::process::ExitCode;

const HELP: &str = "local-stt: free, local push-to-talk speech-to-text

USAGE:
  local-stt                      start the app (menu bar / tray)
  local-stt doctor               show your hardware and which models fit
  local-stt fetch-model <id>     download a model: small, medium or large-v3
  local-stt demo                 a short tour with sample data (nothing is recorded or pasted)
  local-stt check-update         see whether a newer release is available
  local-stt diagnostics          save a troubleshooting report to share (no audio or dictated text)
  local-stt install              add local-stt to Applications / Start menu and your terminal
  local-stt uninstall            remove what install added (settings and history are kept)
  local-stt --autostart on|off   start local-stt when you log in
  local-stt --version            print the version
  local-stt --help               print this help
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None => local_stt::app::run(),
        Some("doctor") => doctor(),
        Some("fetch-model") => fetch_model(args.get(1).map(String::as_str)),
        Some("--autostart") => set_autostart(args.get(1).map(String::as_str)),
        Some("install") => install(),
        Some("check-update") => check_update(),
        Some("diagnostics") => diagnostics(),
        Some("demo") => local_stt::demo::prepare().and_then(|()| local_stt::app::run()),
        Some("uninstall") => uninstall(),
        Some("--version") | Some("-V") => {
            println!("local-stt {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help") | Some("-h") => {
            print!("{HELP}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}\n\n{HELP}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("local-stt: {e}");
            ExitCode::FAILURE
        }
    }
}

// Works even when the app will not start, which is when a report matters most.
fn diagnostics() -> Result<(), String> {
    paths::ensure_dirs().map_err(|e| format!("cannot create the app data folder: {e}"))?;
    let cfg = local_stt::config::load(&paths::config_path());
    // With the log off, saving a report must not start a new log file.
    local_stt::diag::set_enabled(cfg.diagnostic_log);
    let hw = hwprobe::probe(&paths::data_dir());
    let path = local_stt::diag::export(&cfg, &hw)?;
    println!("Saved {}", without_home(&path));
    println!("It holds app, system and settings details and recent app logs, never audio or dictated text.");
    println!("Read it before you share it.");
    Ok(())
}

fn dash<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "-".into())
}

fn doctor() -> Result<(), String> {
    let hw = hwprobe::probe(&paths::data_dir());
    println!("Hardware");
    println!("  OS           {} ({})", hw.os, hw.arch);
    println!("  CPU          {} ({} threads)", hw.cpu, hw.threads);
    println!("  Memory       {} GB", dash(hw.ram_gb));
    println!(
        "  GPU          {}{}",
        hw.gpu,
        if hw.gpu_accel {
            " (used)"
        } else {
            " (not used)"
        }
    );
    println!("  Free disk    {} GB", dash(hw.free_disk_gb));
    println!("  Data folder  {}", without_home(&paths::data_dir()));
    println!();
    println!("Models");
    for fit in models::evaluate(&hw, &paths::models_dir()) {
        let mut tags = Vec::new();
        if fit.recommended {
            tags.push("Recommended");
        }
        if fit.downloaded {
            tags.push("Downloaded");
        }
        if !fit.supported {
            tags.push("Not supported");
        }
        println!(
            "  {:<18} {:>5} MB  {}",
            fit.label,
            fit.size_mb,
            tags.join(", ")
        );
        for reason in &fit.reasons {
            println!("      {reason}");
        }
    }
    Ok(())
}

// Keeps the account name out of output that people paste into public issues.
fn without_home(path: &std::path::Path) -> String {
    let shown = path.display().to_string();
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"));
    match home {
        Ok(h) if !h.is_empty() && shown.starts_with(&h) => format!("~{}", &shown[h.len()..]),
        _ => shown,
    }
}

fn parse_model(id: Option<&str>) -> Result<models::ModelId, String> {
    match id {
        Some("small") => Ok(models::ModelId::Small),
        Some("medium") => Ok(models::ModelId::Medium),
        Some("large-v3") => Ok(models::ModelId::LargeV3),
        _ => Err("choose a model: small, medium or large-v3".into()),
    }
}

fn fetch_model(id: Option<&str>) -> Result<(), String> {
    let id = parse_model(id)?;
    paths::ensure_dirs().map_err(|e| e.to_string())?;
    let info = models::info(id);
    let dest = models::path(&paths::models_dir(), id);
    if dest.exists() && download::sha256_file(&dest).map_err(|e| e.to_string())? == info.sha256 {
        println!(
            "{} already downloaded and verified: {}",
            info.label,
            dest.display()
        );
        return Ok(());
    }
    let total = info.size_bytes as f64;
    let progress = |done: u64| {
        let pct = (done as f64 / total * 100.0).min(100.0);
        print!("\rDownloading {}: {pct:5.1}%", info.label);
        let _ = std::io::stdout().flush();
    };
    download::download(&models::url(id), info.sha256, &dest, &progress)?;
    println!(
        "\rDownloaded and verified {} -> {}",
        info.label,
        dest.display()
    );
    Ok(())
}

fn check_update() -> Result<(), String> {
    let info = local_stt::update::check()?;
    match (info.available, info.latest.as_deref()) {
        (true, Some(latest)) => println!(
            "local-stt {latest} is available (you have {}).\nRelease notes: {}\nUpdate: uvx local-stt@{latest} install",
            info.current, info.notes_url
        ),
        _ => println!("local-stt {} is up to date.", info.current),
    }
    Ok(())
}

fn install() -> Result<(), String> {
    let exe = autostart::current_exe()?;
    let (_installed, notes) = local_stt::install::install(&exe)?;
    for note in notes {
        println!("{note}");
    }
    Ok(())
}

fn uninstall() -> Result<(), String> {
    println!("{}", autostart::set(false)?);
    for note in local_stt::install::uninstall()? {
        println!("{note}");
    }
    println!(
        "Settings, history and models are kept in {}.",
        without_home(&paths::data_dir())
    );
    Ok(())
}

fn set_autostart(arg: Option<&str>) -> Result<(), String> {
    match arg {
        Some("on") | Some("off") => {
            println!("{}", autostart::set(arg == Some("on"))?);
            Ok(())
        }
        _ => Err("use --autostart on or --autostart off".into()),
    }
}
