#![forbid(unsafe_code)]

mod director;
mod pc_overlay;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use leader_core::{build_topology, Machine};
use leader_svg::{render, RenderConfig};

fn main() {
    if let Err(error) = run() {
        eprintln!("leader-cli: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "render".to_owned());
    let options = Options::parse(args.collect())?;
    match command.as_str() {
        "render" => render_cmd(options),
        "trace" => trace_cmd(options),
        "stats" => stats_cmd(options),
        "help" | "--help" | "-h" => {
            help();
            Ok(())
        }
        other => Err(format!("unknown command '{other}'")),
    }
}

#[derive(Debug, Clone)]
struct Options {
    seed: String,
    output: PathBuf,
    max_frames: u32,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let (mut seed, mut output, mut max_frames) = (
            "leader-invader-dev".to_owned(),
            PathBuf::from("generated/Leader.svg"),
            5000_u32,
        );
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--seed" => {
                    index += 1;
                    seed = args.get(index).ok_or("--seed requires value")?.clone();
                }
                "--output" | "-o" => {
                    index += 1;
                    output = PathBuf::from(args.get(index).ok_or("--output requires path")?);
                }
                "--max-frames" => {
                    index += 1;
                    max_frames = args
                        .get(index)
                        .ok_or("--max-frames requires value")?
                        .parse()
                        .map_err(|error| format!("invalid frame count: {error}"))?;
                }
                other => return Err(format!("unknown option '{other}'")),
            }
            index += 1;
        }
        Ok(Self {
            seed,
            output,
            max_frames,
        })
    }
}

fn render_cmd(options: Options) -> Result<(), String> {
    let topology = build_topology();
    let trace = Machine::run_match(&options.seed, options.max_frames);
    if !trace.finished {
        return Err(format!(
            "match did not clear within {} frames",
            options.max_frames
        ));
    }
    let config = RenderConfig::default();
    let svg = render(&topology, &trace, config);
    let svg = director::apply_camera(svg, &topology, &trace, config);
    let svg = pc_overlay::apply(svg, &topology, &trace, config);
    write(&options.output, svg.as_bytes())?;
    let trace_path = options.output.with_file_name("trace.json");
    write(&trace_path, trace.to_json().as_bytes())?;
    println!(
        "rendered {} nodes / {} links / {} frames / {} kills -> {}",
        topology.nodes.len(),
        topology.links.len(),
        trace.total_frames,
        trace.kills.len(),
        options.output.display()
    );
    Ok(())
}

fn trace_cmd(mut options: Options) -> Result<(), String> {
    if options.output == PathBuf::from("generated/Leader.svg") {
        options.output = PathBuf::from("generated/trace.json");
    }
    let trace = Machine::run_match(&options.seed, options.max_frames);
    write(&options.output, trace.to_json().as_bytes())?;
    println!(
        "frames={} kills={} clear={}",
        trace.total_frames,
        trace.kills.len(),
        trace.finished
    );
    if trace.finished {
        Ok(())
    } else {
        Err("trace hit frame limit".to_owned())
    }
}

fn stats_cmd(options: Options) -> Result<(), String> {
    let topology = build_topology();
    let trace = Machine::run_match(&options.seed, options.max_frames);
    println!("topology.nodes={}", topology.nodes.len());
    println!("topology.links={}", topology.links.len());
    println!("trace.frames={}", trace.frames.len());
    println!("trace.micro_samples={}", trace.micro_samples.len());
    println!("trace.kills={}", trace.kills.len());
    println!("trace.finished={}", trace.finished);
    println!("trace.final_score={}", trace.final_score);
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn help() {
    println!(
        "leader-cli\n\nrender [--seed TEXT] [--output PATH] [--max-frames N]\ntrace  [--seed TEXT] [--output PATH]\nstats  [--seed TEXT]\n\nSame source + same seed => same deterministic replay."
    );
}
