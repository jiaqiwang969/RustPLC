use rustplc_hal::traits::HalBackend;
use rustplc_orchestrator::{create_backend, OrchestratorConfig};
use std::time::Duration;

fn usage() -> ! {
    eprintln!(
        "Usage: rustplc_blink <hal_config.toml> [device_name] [period_ms] [count]\n\
         \n\
         Examples:\n\
           cargo run -p rustplc_orchestrator --bin blink -- config/hal_modbus_tcp.toml valve_extend 200 6\n\
           cargo run -p rustplc_orchestrator --bin blink -- config/hal_icesugar_pro_rtu.toml valve_extend 200 6\n\
         \n\
         Notes:\n\
         - count=0 means blink forever.\n\
         - Always forces the output OFF on exit (best effort)."
    );
    std::process::exit(2)
}

fn parse_u64(s: &str, name: &str) -> u64 {
    s.parse::<u64>().unwrap_or_else(|_| {
        eprintln!("Invalid {name}: {s}");
        usage()
    })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let config_path = args.next().unwrap_or_else(|| usage());
    let device = args.next().unwrap_or_else(|| "valve_extend".to_string());
    let period_ms = args
        .next()
        .map(|s| parse_u64(&s, "period_ms"))
        .unwrap_or(200);
    let count = args
        .next()
        .map(|s| parse_u64(&s, "count"))
        .unwrap_or(6);

    let config = OrchestratorConfig::from_file(&config_path)
        .unwrap_or_else(|e| panic!("failed to load HAL config: {e}"));
    let mut hal = create_backend(&config).expect("failed to create HAL backend");

    // Keep a conservative cycle for Modbus RTU/TCP links.
    let half = Duration::from_millis(period_ms.max(1) / 2);

    let mut iter: u64 = 0;
    loop {
        if count != 0 && iter >= count {
            break;
        }

        let on = iter % 2 == 0;
        hal.write_digital_output(&device, on);
        if let Err(e) = hal.flush_outputs() {
            eprintln!("flush_outputs failed: {e}");
            break;
        }
        std::thread::sleep(half);
        iter += 1;
    }

    // Best-effort fail-safe.
    hal.write_digital_output(&device, false);
    let _ = hal.flush_outputs();
}
