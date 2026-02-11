use rust_plc::error::PlcError;
use rust_plc::parser::parse_plc;
use rust_plc::semantic::{
    build_constraint_set, build_state_machine, build_timing_model, build_topology_graph,
};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn example_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(file_name)
}

fn read_example(file_name: &str) -> String {
    let path = example_path(file_name);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read example {}: {err}", path.display()))
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

fn compile_source_to_json(source: &str) -> Result<Value, Vec<PlcError>> {
    let program = parse_plc(source).map_err(|err| vec![err])?;

    let mut errors = Vec::new();
    let topology = collect_stage(build_topology_graph(&program), &mut errors);
    let state_machine = collect_stage(build_state_machine(&program), &mut errors);
    let constraints = collect_stage(build_constraint_set(&program), &mut errors);
    let timing_model = collect_stage(build_timing_model(&program), &mut errors);

    if !errors.is_empty() {
        return Err(errors);
    }

    let payload = json!({
        "topology": topology.expect("topology exists when semantic errors are empty"),
        "state_machine": state_machine.expect("state machine exists when semantic errors are empty"),
        "constraints": constraints.expect("constraints exist when semantic errors are empty"),
        "timing_model": timing_model.expect("timing model exists when semantic errors are empty"),
    });

    let serialized = serde_json::to_string_pretty(&payload).expect("IR payload should serialize");
    let decoded: Value =
        serde_json::from_str(&serialized).expect("serialized IR payload should be valid JSON");

    Ok(decoded)
}

#[test]
fn parses_two_cylinder_example_into_ir_json() {
    let source = read_example("two_cylinder.plc");
    let ir_json = compile_source_to_json(&source).expect("two_cylinder example should compile");

    assert!(ir_json.get("topology").is_some());
    assert!(ir_json.get("state_machine").is_some());
    assert!(ir_json.get("constraints").is_some());
    assert!(ir_json.get("timing_model").is_some());

    let states = ir_json["state_machine"]["states"]
        .as_array()
        .expect("state machine should include states array");
    assert!(!states.is_empty(), "state machine should have states");
}

#[test]
fn parses_half_rotation_example_into_ir_json() {
    let source = read_example("half_rotation.plc");
    let ir_json = compile_source_to_json(&source).expect("half_rotation example should compile");

    let transitions = ir_json["state_machine"]["transitions"]
        .as_array()
        .expect("state machine should include transitions array");
    assert!(
        !transitions.is_empty(),
        "state machine should have transitions"
    );

    let timing_rules = ir_json["constraints"]["timing"]
        .as_array()
        .expect("constraints should include timing array");
    assert_eq!(
        timing_rules.len(),
        1,
        "half_rotation should define one timing rule"
    );
}

#[test]
fn reports_undefined_device_for_error_example() {
    let source = read_example("error_missing_device.plc");
    let errors = compile_source_to_json(&source)
        .expect_err("error_missing_device should fail semantic checks");

    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains("未定义设备 Y9")),
        "error output should include missing device name"
    );
}

#[test]
fn cli_prints_ir_json_for_two_cylinder_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_rust_plc"))
        .arg(example_path("two_cylinder.plc"))
        .output()
        .expect("should run rust_plc binary");

    assert!(
        output.status.success(),
        "CLI should succeed for valid example, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded: Value =
        serde_json::from_slice(&output.stdout).expect("CLI stdout should be valid JSON");
    assert!(decoded.get("topology").is_some());
    assert!(decoded.get("state_machine").is_some());
    assert!(decoded.get("constraints").is_some());
    assert!(decoded.get("timing_model").is_some());
}
