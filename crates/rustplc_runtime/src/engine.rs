use rustplc_hal::traits::HalBackend;
use std::time::{Duration, Instant};

use crate::timer::TimerBank;

pub struct ScanCycleEngine<S, H: HalBackend> {
    pub hal: H,
    pub state: S,
    pub timers: TimerBank,
    pub cycle_time: Duration,
    pub cycle_count: u64,
    scan_fn: fn(&mut S, &mut H, &mut TimerBank),
}

impl<S, H: HalBackend> ScanCycleEngine<S, H> {
    pub fn new(
        hal: H,
        initial_state: S,
        cycle_time_ms: u64,
        scan_fn: fn(&mut S, &mut H, &mut TimerBank),
    ) -> Self {
        Self {
            hal,
            state: initial_state,
            timers: TimerBank::new(),
            cycle_time: Duration::from_millis(cycle_time_ms),
            cycle_count: 0,
            scan_fn,
        }
    }

    pub fn run_cycles(&mut self, count: u64) {
        for _ in 0..count {
            self.step();
        }
    }

    pub fn step(&mut self) {
        let _ = self.hal.refresh_inputs();
        (self.scan_fn)(&mut self.state, &mut self.hal, &mut self.timers);
        let _ = self.hal.flush_outputs();
        self.timers.tick(self.cycle_time.as_millis() as u64);
        self.cycle_count += 1;
    }

    pub fn run_realtime(&mut self) {
        loop {
            let t0 = Instant::now();
            self.step();
            let elapsed = t0.elapsed();
            if elapsed < self.cycle_time {
                std::thread::sleep(self.cycle_time - elapsed);
            }
        }
    }
}
