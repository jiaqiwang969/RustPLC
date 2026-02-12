use rust_plc::codegen::{self, CodegenConfig};
use rust_plc::error::PlcError;
use rust_plc::ir::{ConstraintSet, StateMachine, TimingModel, TopologyGraph};
use rust_plc::parser::parse_plc;
use rust_plc::semantic::{
    build_constraint_set, build_state_machine, build_timing_model, build_topology_graph,
};
use rust_plc::verification::{VerificationSummary, verify_all};
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
    verification: VerificationSummary,
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    let program = args.remove(0);

    if args.is_empty() {
        eprintln!("Usage: {program} <file.plc> [--generate <output_dir>]");
        std::process::exit(1);
    }

    let path = args.remove(0);

    // Parse --generate flag
    let generate_dir = if args.first().map(|a| a.as_str()) == Some("--generate") {
        args.remove(0); // consume --generate
        let dir = if args.is_empty() {
            "generated".to_string()
        } else {
            args.remove(0)
        };
        Some(dir)
    } else {
        None
    };

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

    let ir_bundle = match compile_pipeline(&source) {
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

    print_success_summary(&ir_bundle.verification);

    if let Some(dir) = generate_dir {
        let config = CodegenConfig::default();
        let output_path = Path::new(&dir);
        match codegen::generate_project(
            &ir_bundle.state_machine,
            &ir_bundle.topology,
            &ir_bundle.timing_model,
            &config,
            output_path,
        ) {
            Ok(()) => {
                eprintln!("代码已生成到 {dir}/");
            }
            Err(err) => {
                eprintln!("代码生成失败: {err}");
                std::process::exit(1);
            }
        }
    } else {
        match serde_json::to_string_pretty(&ir_bundle) {
            Ok(json) => println!("{json}"),
            Err(err) => {
                eprintln!("Failed to serialize IR as JSON: {err}");
                std::process::exit(1);
            }
        }
    }
}

fn compile_pipeline(source: &str) -> Result<IrBundle, Vec<String>> {
    let program = parse_plc(source).map_err(|err| vec![err.to_string()])?;

    let mut errors = Vec::new();
    let topology = collect_stage(build_topology_graph(&program), &mut errors);
    let state_machine = collect_stage(build_state_machine(&program), &mut errors);
    let constraints = collect_stage(build_constraint_set(&program), &mut errors);
    let timing_model = collect_stage(build_timing_model(&program), &mut errors);

    if !errors.is_empty() {
        return Err(errors.into_iter().map(|error| error.to_string()).collect());
    }

    let topology = topology.expect("topology exists when semantic errors are empty");
    let state_machine = state_machine.expect("state machine exists when semantic errors are empty");
    let constraints = constraints.expect("constraints exist when semantic errors are empty");
    let timing_model = timing_model.expect("timing model exists when semantic errors are empty");

    let verification =
        verify_all(&program, &topology, &constraints, &state_machine).map_err(|issues| {
            issues
                .into_iter()
                .map(|issue| issue.to_string())
                .collect::<Vec<_>>()
        })?;

    Ok(IrBundle {
        topology,
        state_machine,
        constraints,
        timing_model,
        verification,
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

fn print_success_summary(summary: &VerificationSummary) {
    eprintln!("验证通过：");
    eprintln!(
        "  - Safety: {}（深度 {}）",
        summary.safety.level, summary.safety.explored_depth
    );

    for warning in &summary.safety.warnings {
        eprintln!("    {warning}");
    }

    eprintln!("  - Liveness: {}", summary.liveness.level);
    eprintln!("  - Timing: {}", summary.timing.level);
    eprintln!("  - Causality: {}", summary.causality.level);
}
