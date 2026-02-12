//! End-to-end Mode B integration test.
//!
//! Spins up a mock Modbus TCP server, creates a backend via the orchestrator
//! config, runs a ScanCycleEngine with a simple extend/retract state machine,
//! and verifies the full pipeline: TOML config → ModbusBackend → engine → I/O.

use rustplc_hal::traits::HalBackend;
use rustplc_orchestrator::{create_backend, OrchestratorConfig};
use rustplc_runtime::engine::ScanCycleEngine;
use rustplc_runtime::timer::TimerBank;
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

/// Simple two-state PLC: Extending → Retracting → Extending ...
#[derive(Debug, Clone, PartialEq)]
enum CylinderState {
    Extending,
    Retracting,
}

/// Scan function: drives valve coils based on state, transitions on sensor input.
fn cylinder_scan(
    state: &mut CylinderState,
    hal: &mut Box<dyn HalBackend>,
    _timers: &mut TimerBank,
) {
    match state {
        CylinderState::Extending => {
            hal.write_digital_output("valve_extend", true);
            hal.write_digital_output("valve_retract", false);
            if hal.read_digital_input("sensor_end") {
                *state = CylinderState::Retracting;
            }
        }
        CylinderState::Retracting => {
            hal.write_digital_output("valve_extend", false);
            hal.write_digital_output("valve_retract", true);
            if hal.read_digital_input("sensor_home") {
                *state = CylinderState::Extending;
            }
        }
    }
}

/// Mock Modbus TCP server that simulates sensor physics:
/// - Tracks coil writes (FC 0x0F)
/// - After valve_extend (coil 0) is ON for 3+ read cycles, sensor_end (DI 1) goes HIGH
/// - After valve_retract (coil 1) is ON for 3+ read cycles, sensor_home (DI 0) goes HIGH
async fn mock_modbus_server(listener: TcpListener) {
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut buf = [0u8; 256];

    let mut coil_state = [false; 16];
    let mut extend_count: u32 = 0;
    let mut retract_count: u32 = 0;

    for _ in 0..50 {
        let n = stream.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }

        let tid = [buf[0], buf[1]];
        let unit_id = buf[6];
        let fc = buf[7];

        match fc {
            // Read Discrete Inputs (FC 0x02)
            0x02 => {
                // Update physics
                if coil_state[0] {
                    extend_count += 1;
                    retract_count = 0;
                } else if coil_state[1] {
                    retract_count += 1;
                    extend_count = 0;
                }

                let sensor_home = retract_count >= 3;
                let sensor_end = extend_count >= 3;

                let count = u16::from_be_bytes([buf[10], buf[11]]);
                let byte_count = ((count + 7) / 8) as u8;

                let mut di_byte: u8 = 0;
                if sensor_home {
                    di_byte |= 1 << 0;
                }
                if sensor_end {
                    di_byte |= 1 << 1;
                }

                let mut resp = Vec::with_capacity(9 + byte_count as usize);
                resp.extend_from_slice(&tid);
                resp.extend_from_slice(&[0x00, 0x00]);
                let len = 3 + byte_count as u16;
                resp.extend_from_slice(&len.to_be_bytes());
                resp.push(unit_id);
                resp.push(fc);
                resp.push(byte_count);
                resp.push(di_byte);
                for _ in 1..byte_count {
                    resp.push(0x00);
                }
                stream.write_all(&resp).await.unwrap();
            }
            // Write Multiple Coils (FC 0x0F)
            0x0F => {
                let start = u16::from_be_bytes([buf[8], buf[9]]) as usize;
                let qty = u16::from_be_bytes([buf[10], buf[11]]) as usize;
                let _byte_count = buf[12];
                // Decode coil values from data bytes
                for i in 0..qty {
                    let byte_idx = 13 + i / 8;
                    let bit_idx = i % 8;
                    let val = (buf[byte_idx] >> bit_idx) & 1 == 1;
                    if start + i < 16 {
                        coil_state[start + i] = val;
                    }
                }

                let mut resp = Vec::with_capacity(12);
                resp.extend_from_slice(&tid);
                resp.extend_from_slice(&[0x00, 0x00, 0x00, 0x06]);
                resp.push(unit_id);
                resp.push(fc);
                resp.extend_from_slice(&buf[8..10]); // start
                resp.extend_from_slice(&buf[10..12]); // qty
                stream.write_all(&resp).await.unwrap();
            }
            _ => break,
        }
    }
}

#[test]
fn mode_b_end_to_end_modbus_scan_cycle() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        // Bind mock server to random port
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(mock_modbus_server(listener));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Build config pointing to mock server
        let toml_str = format!(
            r#"
[mode]
type = "modbus_tcp"

[modbus]
host = "127.0.0.1"
port = {port}
slave_id = 1

[runtime]
cycle_time_ms = 10

[mapping.coils]
valve_extend = 0
valve_retract = 1

[mapping.discrete_inputs]
sensor_home = 0
sensor_end = 1
"#
        );

        let config = OrchestratorConfig::from_toml(&toml_str).unwrap();

        // Create backend via orchestrator factory
        let backend = tokio::task::spawn_blocking(move || {
            create_backend(&config).expect("create_backend should succeed")
        })
        .await
        .unwrap();

        // Build scan cycle engine
        let mut engine = ScanCycleEngine::new(
            backend,
            CylinderState::Extending,
            10,
            cylinder_scan,
        );

        // Run scan cycles in blocking context (ModbusBackend uses internal Runtime)
        let final_state = tokio::task::spawn_blocking(move || {
            // Initially extending
            assert_eq!(engine.state, CylinderState::Extending);

            // Run enough cycles for sensor_end to trigger (3 read cycles)
            // Each step: refresh_inputs → scan → flush_outputs
            engine.run_cycles(5);

            // After 5 cycles with valve_extend ON, sensor_end should have
            // triggered and state should transition to Retracting
            assert_eq!(
                engine.state,
                CylinderState::Retracting,
                "should transition to Retracting after sensor_end triggers"
            );

            // Run more cycles for retract
            engine.run_cycles(5);

            // After retracting with valve_retract ON, sensor_home triggers
            assert_eq!(
                engine.state,
                CylinderState::Extending,
                "should transition back to Extending after sensor_home triggers"
            );

            assert!(engine.cycle_count >= 10, "should have run at least 10 cycles");

            engine.state.clone()
        })
        .await
        .unwrap();

        // Full round-trip verified: Extending → Retracting → Extending
        assert_eq!(final_state, CylinderState::Extending);
    });
}

#[test]
fn mode_a_sim_backend_scan_cycle() {
    // Verify Mode A (SimBackend) also works through the orchestrator
    let toml_str = r#"
[mode]
type = "sim"

[runtime]
cycle_time_ms = 10
"#;
    let config = OrchestratorConfig::from_toml(toml_str).unwrap();
    let backend = create_backend(&config).unwrap();

    let mut engine = ScanCycleEngine::new(
        backend,
        CylinderState::Extending,
        10,
        cylinder_scan,
    );

    // SimBackend always returns false for inputs, so state stays Extending
    engine.run_cycles(10);
    assert_eq!(engine.state, CylinderState::Extending);
    assert_eq!(engine.cycle_count, 10);
}
