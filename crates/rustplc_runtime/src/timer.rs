use std::collections::HashMap;

const MAX_TIMERS: usize = 32;

pub struct TimerBank {
    timers: [(u64, u64, u64, bool); MAX_TIMERS],
    name_map: HashMap<String, usize>,
    next_slot: usize,
}

impl TimerBank {
    pub fn new() -> Self {
        Self {
            timers: [(0, 0, 0, false); MAX_TIMERS],
            name_map: HashMap::new(),
            next_slot: 0,
        }
    }

    pub fn start(&mut self, name: &str, duration_ms: u64) {
        let slot = self.slot_for(name);
        self.timers[slot] = (0, duration_ms, 0, true);
    }

    pub fn cancel(&mut self, name: &str) {
        if let Some(&slot) = self.name_map.get(name) {
            self.timers[slot].3 = false;
        }
    }

    pub fn expired(&self, name: &str) -> bool {
        self.name_map
            .get(name)
            .map(|&slot| {
                let (elapsed, duration, _, active) = self.timers[slot];
                active && elapsed >= duration
            })
            .unwrap_or(false)
    }

    pub fn tick(&mut self, cycle_time_ms: u64) {
        for timer in &mut self.timers {
            if timer.3 {
                timer.0 += cycle_time_ms;
            }
        }
    }

    fn slot_for(&mut self, name: &str) -> usize {
        if let Some(&slot) = self.name_map.get(name) {
            return slot;
        }
        let slot = self.next_slot;
        self.name_map.insert(name.to_string(), slot);
        self.next_slot = (self.next_slot + 1).min(MAX_TIMERS - 1);
        slot
    }
}

impl Default for TimerBank {
    fn default() -> Self {
        Self::new()
    }
}
