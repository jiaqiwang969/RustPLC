use rust_plc::error::PlcError;
use rust_plc::ir::{ConstraintSet, StateMachine, TimingModel, TopologyGraph};
use rust_plc::parser::parse_plc;
use rust_plc::semantic::{
    build_constraint_set, build_state_machine, build_timing_model, build_topology_graph,
};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
struct IrBundle {
    topology: TopologyGraph,
    state_machine: StateMachine,
    constraints: ConstraintSet,
    timing_model: TimingModel,
}

fn main() {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "rust_plc".to_string());

    let Some(path) = args.next() else {
        eprintln!("Usage: {program} <file.plc>");
        std::process::exit(1);
    };

    if args.next().is_some() {
        eprintln!("Usage: {program} <file.plc>");
        std::process::exit(1);
    }

    if Path::new(&path).extension().and_then(|ext| ext.to_str()) != Some("plc") {
        eprintln!("Expected a .plc file path, got: {path}");
        std::process::exit(1);
    }

    let source = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Failed to read PLC file {path}: {err}");
            std::process::exit(1);
        }
    };

    let ir_bundle = match compile_ir_bundle(&source) {
        Ok(ir_bundle) => ir_bundle,
        Err(errors) => {
            for (index, error) in errors.iter().enumerate() {
                if index > 0 {
                    eprintln!();
                }
                eprintln!("{error}");
            }
            std::process::exit(1);
        }
    };

    match serde_json::to_string_pretty(&ir_bundle) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("Failed to serialize IR as JSON: {err}");
            std::process::exit(1);
        }
    }
}

fn compile_ir_bundle(source: &str) -> Result<IrBundle, Vec<PlcError>> {
    let program = parse_plc(source).map_err(|err| vec![err])?;

    let mut errors = Vec::new();
    let topology = collect_stage(build_topology_graph(&program), &mut errors);
    let state_machine = collect_stage(build_state_machine(&program), &mut errors);
    let constraints = collect_stage(build_constraint_set(&program), &mut errors);
    let timing_model = collect_stage(build_timing_model(&program), &mut errors);

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(IrBundle {
        topology: topology.expect("topology exists when semantic errors are empty"),
        state_machine: state_machine.expect("state machine exists when semantic errors are empty"),
        constraints: constraints.expect("constraints exist when semantic errors are empty"),
        timing_model: timing_model.expect("timing model exists when semantic errors are empty"),
    })
}

fn collect_stage<T>(result: Result<T, Vec<PlcError>>, errors: &mut Vec<PlcError>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(mut stage_errors) => {
            errors.append(&mut stage_errors);
            None
        }
    }
}
