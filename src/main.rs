use clap::{Parser, Subcommand};
use omacdiag::{
    discovery,
    engine::{self, Answer, EventSink, OperatorAction, RunContext},
    model::*,
    planner, probes, process, protocol, report,
};
use serde_json::json;
use std::{
    collections::BTreeSet,
    io::{self, Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

#[derive(Parser)]
#[command(
    version,
    about = "Linux diagnostics for Intel Macs (2009–2017); detect, test, report"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Inventory {
        #[arg(long)]
        json: bool,
    },
    Equipment,
    Plan {
        #[arg(long, value_enum, default_value = "quick")]
        profile: Profile,
        #[arg(long, value_enum, default_value = "guided")]
        mode: Mode,
        #[arg(long)]
        equipment: Vec<String>,
        #[arg(long)]
        test: Vec<String>,
    },
    Run {
        #[arg(long, value_enum, default_value = "quick")]
        profile: Profile,
        #[arg(long, value_enum, default_value = "automatic")]
        mode: Mode,
        #[arg(long)]
        equipment: Vec<String>,
        #[arg(long)]
        test: Vec<String>,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        microphone: Option<String>,
        #[arg(long)]
        acknowledge_preflight: bool,
    },
    Session {
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Fixture {
        path: PathBuf,
    },
    #[command(name = "_worker", hide = true)]
    Worker,
    #[command(name = "_backlight_cycle", hide = true)]
    BacklightCycle,
}
fn main() {
    process::install_signals();
    if let Err(e) = run_cli() {
        eprintln!("omacdiag: {e}");
        std::process::exit(1);
    }
}
fn run_cli() -> Result<(), String> {
    match Cli::parse().command {
        Commands::Inventory { json } => {
            let i = discovery::discover();
            if json {
                print_json(&i)?;
            } else {
                println!(
                    "{} {} — {}",
                    i.machine.manufacturer, i.machine.model, i.machine.distribution
                );
                for d in i.devices {
                    println!("{}\t{:?}\t{}", d.id, d.recognition, d.name);
                }
                for w in i.warnings {
                    eprintln!("{w}");
                }
            }
        }
        Commands::Equipment => print_json(&planner::preparation())?,
        Commands::Plan {
            profile,
            mode,
            equipment,
            test,
        } => {
            let inventory = discovery::discover();
            let selected = selection(test);
            planner::validate(
                &inventory,
                profile,
                mode,
                &engine::readiness(&equipment),
                selected.as_ref(),
            )?;
            print_json(&planner::build(
                &inventory,
                profile,
                mode,
                &engine::readiness(&equipment),
                selected.as_ref(),
            ))?;
        }
        Commands::Run {
            profile,
            mode,
            equipment,
            test,
            output,
            microphone,
            acknowledge_preflight,
        } => {
            let inventory = discovery::discover();
            let selected = selection(test);
            planner::validate(
                &inventory,
                profile,
                mode,
                &engine::readiness(&equipment),
                selected.as_ref(),
            )?;
            let plan = planner::build(
                &inventory,
                profile,
                mode,
                &engine::readiness(&equipment),
                selected.as_ref(),
            );
            eprintln!("Equipment preflight (before any functional tests):");
            for e in &plan.equipment {
                eprintln!("- {} [{:?}]: {}", e.name, e.readiness, e.instructions);
            }
            for notice in &plan.notices {
                eprintln!("{notice}");
            }
            eprintln!(
                "{} selected checks; estimated {} seconds plus physical actions",
                plan.tests.len(),
                plan.estimated_seconds
            );
            if !acknowledge_preflight {
                return Err("Review `omacdiag equipment` and `omacdiag plan`; supply --acknowledge-preflight to run. Mark available kit with --equipment ID.".into());
            }
            let report = Arc::new(Mutex::new(Report::new(inventory, plan)));
            let output = output.unwrap_or_else(|| engine::default_output(&report.lock().unwrap()));
            let emit: EventSink = Arc::new(|kind, payload| {
                if kind == "test_started" && payload["test"]["probe"] == "keyboard_backlight" {
                    eprintln!(
                        "\n{}\n{}",
                        payload["test"]["name"]
                            .as_str()
                            .unwrap_or("Keyboard backlight"),
                        payload["test"]["instructions"]
                            .as_str()
                            .unwrap_or("Watch the keyboard backlight change.")
                    );
                } else if kind == "result" {
                    eprintln!(
                        "{}: {} ({})",
                        payload["test_id"].as_str().unwrap_or(""),
                        payload["outcome"].as_str().unwrap_or(""),
                        payload["reason_code"].as_str().unwrap_or("")
                    );
                }
            });
            let answer: Answer = Arc::new(|test, stage, result, _| {
                eprintln!("\n{}\n{}", test.name, test.instructions);
                if stage == "prepare" {
                    eprintln!(
                        "Perform the action, then enter details as JSON (fixture_path, port_label, peer, address as applicable). Enter skip to skip."
                    );
                    let line = terminal_line()?;
                    if line.trim() == "skip" {
                        return Some(OperatorAction::Observe(observation(
                            Outcome::Skipped,
                            String::new(),
                            json!({}),
                        )));
                    }
                    let details = if line.trim().is_empty() {
                        json!({})
                    } else {
                        match serde_json::from_str(&line) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("Invalid JSON: {e}; check skipped");
                                return Some(OperatorAction::Observe(observation(
                                    Outcome::Skipped,
                                    e.to_string(),
                                    json!({}),
                                )));
                            }
                        }
                    };
                    Some(OperatorAction::Observe(observation(
                        Outcome::Inconclusive,
                        String::new(),
                        details,
                    )))
                } else {
                    if let Some(r) = result {
                        eprintln!("Evidence: {} — {}", r.reason_code, r.summary);
                    }
                    eprintln!(
                        "Assess the observed function: p=pass, f=fail, i=inconclusive, s=skip, r=retake. Optional note after the letter."
                    );
                    let line = terminal_line()?;
                    let (v, note) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
                    if v == "r" {
                        return Some(OperatorAction::Retake);
                    }
                    Some(OperatorAction::Observe(observation(
                        match v {
                            "p" => Outcome::Passed,
                            "f" => Outcome::Failed,
                            "s" => Outcome::Skipped,
                            _ => Outcome::Inconclusive,
                        },
                        note.into(),
                        json!({}),
                    )))
                }
            });
            engine::run(RunContext {
                report: report.clone(),
                output: output.clone(),
                cancel: Arc::new(AtomicBool::new(false)),
                emit,
                answer,
                microphone_id: microphone,
            })?;
            let r = report.lock().unwrap();
            print_json(&*r)?;
            eprintln!("Report: {}", output.display());
        }
        Commands::Session { output } => protocol::session(output)?,
        Commands::Fixture { path } => {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())?;
            file.write_all(&probes::fixture_data())
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        Commands::Worker => {
            let mut data = Vec::new();
            io::stdin()
                .take(4 * 1024 * 1024)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            let req: probes::WorkerRequest =
                serde_json::from_slice(&data).map_err(|e| e.to_string())?;
            let r = probes::execute(&req);
            protocol::emit_raw(json!({"type":"worker_result","payload":r}));
        }
        Commands::BacklightCycle => probes::backlight::guardian_main()?,
    }
    Ok(())
}
fn selection(values: Vec<String>) -> Option<BTreeSet<String>> {
    if values.is_empty() {
        None
    } else {
        Some(values.into_iter().collect())
    }
}
fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    serde_json::to_writer_pretty(io::stdout().lock(), value).map_err(|e| e.to_string())?;
    println!();
    Ok(())
}
fn terminal_line() -> Option<String> {
    use std::{
        sync::{OnceLock, mpsc},
        time::Duration,
    };
    static INPUT: OnceLock<Mutex<mpsc::Receiver<String>>> = OnceLock::new();
    let input = INPUT.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            loop {
                let mut line = String::new();
                match io::stdin().read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Mutex::new(rx)
    });
    loop {
        if process::INTERRUPTED.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        match input.lock().ok()?.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => return Some(line),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return None,
        }
    }
}
fn observation(verdict: Outcome, note: String, details: serde_json::Value) -> Observation {
    Observation {
        verdict,
        note,
        details,
        recorded_at: report::timestamp(),
    }
}
