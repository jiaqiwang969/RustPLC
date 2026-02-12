use rustplc_ir::{
    BinaryValue, StateMachine, State, Transition, TransitionAction, TransitionGuard,
    TimerOperationKind,
};
use std::collections::HashMap;
use std::fmt::Write;

use crate::expression;

/// Convert "task_name.step_name" to PascalCase enum variant name.
fn state_to_variant(state: &State) -> String {
    let mut result = String::new();
    for part in state.task_name.split('_') {
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            result.push(c.to_ascii_uppercase());
            result.extend(chars);
        }
    }
    for part in state.step_name.split('_') {
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            result.push(c.to_ascii_uppercase());
            result.extend(chars);
        }
    }
    result
}

pub fn emit_state_enum(out: &mut String, sm: &StateMachine) {
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq)]").unwrap();
    writeln!(out, "pub enum PlcState {{").unwrap();
    for state in &sm.states {
        writeln!(
            out,
            "    {}, // {}.{}",
            state_to_variant(state),
            state.task_name,
            state.step_name
        )
        .unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // impl initial()
    let initial = state_to_variant(&sm.initial);
    writeln!(out, "impl PlcState {{").unwrap();
    writeln!(out, "    pub fn initial() -> Self {{").unwrap();
    writeln!(out, "        PlcState::{initial}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}

pub fn emit_scan_cycle_fn(out: &mut String, sm: &StateMachine) {
    writeln!(
        out,
        "fn scan_cycle(state: &mut PlcState, hal: &mut impl HalBackend, timers: &mut TimerBank) {{"
    )
    .unwrap();
    writeln!(out, "    match *state {{").unwrap();

    // Group transitions by source state
    let mut by_source: HashMap<(String, String), Vec<&Transition>> = HashMap::new();
    for t in &sm.transitions {
        by_source
            .entry((t.from.task_name.clone(), t.from.step_name.clone()))
            .or_default()
            .push(t);
    }

    for state in &sm.states {
        let variant = state_to_variant(state);
        let key = (state.task_name.clone(), state.step_name.clone());
        let transitions = by_source.get(&key);

        writeln!(out, "        PlcState::{variant} => {{").unwrap();

        if let Some(transitions) = transitions {
            // Collect entry actions: from the first condition/always transition
            // These are the actions that happen when we *enter* this state
            let entry_actions: Vec<&TransitionAction> = transitions
                .iter()
                .find(|t| matches!(t.guard, TransitionGuard::Always | TransitionGuard::Condition { .. }))
                .map(|t| t.actions.iter().collect())
                .unwrap_or_default();

            // Emit entry actions once
            emit_actions(out, &entry_actions);

            // Collect and deduplicate timer starts
            let mut seen_timers = std::collections::HashSet::new();
            for t in transitions.iter() {
                for timer_op in &t.timers {
                    if matches!(timer_op.operation, TimerOperationKind::Start) {
                        if let Some(dur) = timer_op.duration_ms {
                            if seen_timers.insert(timer_op.timer_name.clone()) {
                                writeln!(
                                    out,
                                    "            timers.start(\"{}\", {});",
                                    timer_op.timer_name, dur
                                )
                                .unwrap();
                            }
                        }
                    }
                }
            }

            // Classify transitions by guard type
            let timeouts: Vec<&Transition> = transitions
                .iter()
                .filter(|t| matches!(t.guard, TransitionGuard::Timeout { .. }))
                .copied()
                .collect();
            let conditions: Vec<&Transition> = transitions
                .iter()
                .filter(|t| matches!(t.guard, TransitionGuard::Condition { .. }))
                .copied()
                .collect();
            let always: Vec<&Transition> = transitions
                .iter()
                .filter(|t| matches!(t.guard, TransitionGuard::Always))
                .copied()
                .collect();

            let mut has_prior_branch = false;

            // Timeout checks first (highest priority)
            for t in &timeouts {
                let timer_name = t
                    .timers
                    .iter()
                    .find(|op| matches!(op.operation, TimerOperationKind::Start))
                    .map(|op| op.timer_name.as_str())
                    .unwrap_or("unknown_timer");
                let target = state_to_variant(&t.to);
                let keyword = if has_prior_branch { "else if" } else { "if" };
                writeln!(out, "            {keyword} timers.expired(\"{timer_name}\") {{").unwrap();
                emit_transition_side_effects(out, &t.actions, &entry_actions, 16);
                writeln!(out, "                *state = PlcState::{target};").unwrap();
                writeln!(out, "            }}").unwrap();
                has_prior_branch = true;
            }

            // Condition checks
            for t in &conditions {
                if let TransitionGuard::Condition { expression } = &t.guard {
                    let cond = expression::emit_condition(expression);
                    let target = state_to_variant(&t.to);
                    let keyword = if has_prior_branch { "else if" } else { "if" };
                    writeln!(out, "            {keyword} {cond} {{").unwrap();
                    emit_transition_side_effects(out, &t.actions, &entry_actions, 16);
                    writeln!(out, "                *state = PlcState::{target};").unwrap();
                    writeln!(out, "            }}").unwrap();
                    has_prior_branch = true;
                }
            }

            // Always transitions (unconditional, no guard)
            for t in &always {
                let target = state_to_variant(&t.to);
                // Side effects only (entry actions already emitted above)
                emit_transition_side_effects(out, &t.actions, &entry_actions, 12);
                writeln!(out, "            *state = PlcState::{target};").unwrap();
            }
        }

        writeln!(out, "        }}").unwrap();
    }

    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}

fn emit_actions(out: &mut String, actions: &[&TransitionAction]) {
    for action in actions {
        emit_one_action(out, action, 12);
    }
}

/// Emit only the actions in `transition_actions` that are NOT already in `entry_actions`.
/// This avoids duplicating entry actions inside guard branches.
fn emit_transition_side_effects(
    out: &mut String,
    transition_actions: &[TransitionAction],
    entry_actions: &[&TransitionAction],
    indent: usize,
) {
    for action in transition_actions {
        if !entry_actions.iter().any(|ea| *ea == action) {
            emit_one_action(out, action, indent);
        }
    }
}

fn emit_one_action(out: &mut String, action: &TransitionAction, indent: usize) {
    let pad: String = " ".repeat(indent);
    match action {
        TransitionAction::Extend { target } => {
            writeln!(out, "{pad}hal.write_digital_output(\"{target}\", true);").unwrap();
        }
        TransitionAction::Retract { target } => {
            writeln!(out, "{pad}hal.write_digital_output(\"{target}\", false);").unwrap();
        }
        TransitionAction::Set { target, value } => {
            let val = matches!(value, BinaryValue::On);
            writeln!(out, "{pad}hal.write_digital_output(\"{target}\", {val});").unwrap();
        }
        TransitionAction::Log { message } => {
            writeln!(out, "{pad}log::info!(\"{message}\");").unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustplc_ir::{State, StateMachine, Transition, TransitionGuard, TransitionAction};

    fn simple_sm() -> StateMachine {
        StateMachine {
            states: vec![
                State { task_name: "main".into(), step_name: "step_a".into() },
                State { task_name: "main".into(), step_name: "step_b".into() },
            ],
            transitions: vec![Transition {
                from: State { task_name: "main".into(), step_name: "step_a".into() },
                to: State { task_name: "main".into(), step_name: "step_b".into() },
                guard: TransitionGuard::Always,
                actions: vec![TransitionAction::Extend { target: "cyl_A".into() }],
                timers: vec![],
            }],
            initial: State { task_name: "main".into(), step_name: "step_a".into() },
        }
    }

    #[test]
    fn state_to_variant_converts_correctly() {
        let s = State { task_name: "fault_handler".into(), step_name: "safe_stop".into() };
        assert_eq!(state_to_variant(&s), "FaultHandlerSafeStop");
    }

    #[test]
    fn emits_state_enum_with_variants() {
        let sm = simple_sm();
        let mut out = String::new();
        emit_state_enum(&mut out, &sm);
        assert!(out.contains("MainStepA"));
        assert!(out.contains("MainStepB"));
        assert!(out.contains("fn initial()"));
    }

    #[test]
    fn emits_scan_cycle_with_match_arms() {
        let sm = simple_sm();
        let mut out = String::new();
        emit_scan_cycle_fn(&mut out, &sm);
        assert!(out.contains("PlcState::MainStepA =>"));
        assert!(out.contains("write_digital_output(\"cyl_A\", true)"));
        assert!(out.contains("*state = PlcState::MainStepB"));
    }
}
