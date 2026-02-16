//! Codegen integration test for Mode B.
//!
//! Verifies that generated Rust code uses OrchestratorConfig / create_backend
//! instead of hardcoded SimBackend, and that device names are listed in comments.

use rust_plc::codegen::{generate_main_source, CodegenConfig};
use rust_plc::parser::parse_plc;
use rust_plc::semantic::{build_state_machine, build_timing_model, build_topology_graph};

fn compile_to_source(plc_source: &str) -> String {
    let program = parse_plc(plc_source).expect("parse failed");
    let sm = build_state_machine(&program).expect("state machine failed");
    let config = CodegenConfig::default();
    generate_main_source(&sm, &config)
}

#[test]
fn generated_code_uses_orchestrator_not_sim() {
    let plc = std::fs::read_to_string("../../examples/verification/ex1_safety_pass.plc")
        .expect("read ex1");
    let source = compile_to_source(&plc);

    // Must use orchestrator
    assert!(
        source.contains("OrchestratorConfig"),
        "generated code should reference OrchestratorConfig"
    );
    assert!(
        source.contains("create_backend"),
        "generated code should call create_backend"
    );

    // Must NOT hardcode SimBackend
    assert!(
        !source.contains("SimBackend::new()"),
        "generated code should not hardcode SimBackend"
    );
    assert!(
        !source.contains("use rustplc_hal::sim::SimBackend"),
        "generated code should not import SimBackend"
    );
}

#[test]
fn generated_code_reads_config_from_cli_arg() {
    let plc = std::fs::read_to_string("../../examples/verification/ex1_safety_pass.plc")
        .expect("read ex1");
    let source = compile_to_source(&plc);

    assert!(
        source.contains("hal_config.toml"),
        "generated code should default to hal_config.toml"
    );
    assert!(
        source.contains("env_logger::init()"),
        "generated code should initialize logger"
    );
    assert!(
        source.contains("config.runtime.cycle_time_ms"),
        "generated code should read cycle_time from config"
    );
}

#[test]
fn generated_code_lists_device_names_in_comments() {
    let plc = std::fs::read_to_string("../../examples/verification/ex1_safety_pass.plc")
        .expect("read ex1");
    let source = compile_to_source(&plc);

    // ex1 uses cyl_A, cyl_B as outputs and sensor_A, sensor_B as inputs
    assert!(
        source.contains("Outputs (coils):"),
        "generated code should list output devices"
    );
    assert!(
        source.contains("Inputs (discrete_inputs):"),
        "generated code should list input devices"
    );
    assert!(
        source.contains("cyl_A"),
        "generated code should list cyl_A as output"
    );
    assert!(
        source.contains("sensor_A"),
        "generated code should list sensor_A as input"
    );
}

#[test]
fn generated_cargo_toml_includes_orchestrator_dep() {
    // Test via generate_project to a temp dir
    let plc = std::fs::read_to_string("../../examples/verification/ex1_safety_pass.plc")
        .expect("read ex1");
    let program = parse_plc(&plc).expect("parse failed");
    let sm = build_state_machine(&program).expect("sm failed");
    let topo = build_topology_graph(&program).expect("topo failed");
    let timing = build_timing_model(&program).expect("timing failed");

    let tmp = std::env::temp_dir().join("rustplc_codegen_test");
    let _ = std::fs::remove_dir_all(&tmp);

    let config = CodegenConfig::default();
    rust_plc::codegen::generate_project(&sm, &topo, &timing, &config, &tmp)
        .expect("generate_project failed");

    let cargo_toml = std::fs::read_to_string(tmp.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(
        cargo_toml.contains("rustplc_orchestrator"),
        "Cargo.toml should depend on rustplc_orchestrator"
    );

    let main_rs = std::fs::read_to_string(tmp.join("src/main.rs")).expect("read main.rs");
    assert!(
        main_rs.contains("create_backend"),
        "main.rs should call create_backend"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
