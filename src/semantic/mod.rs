use crate::ast::{
    ActionStatement, BinaryValue as AstBinaryValue, ComparisonOperator, ConditionExpression,
    DeviceType, GotoDirective, LiteralValue, OnCompleteDirective, ParallelBlock, PlcProgram,
    RaceBlock, StepStatement, TaskDeclaration, TasksSection, TimeUnit, TimeoutDirective,
    TopologySection, WaitStatement,
};
use crate::error::PlcError;
use crate::ir::{
    BinaryValue as IrBinaryValue, ConnectionType, Device, DeviceKind, State, StateMachine,
    TimerOperation, TimerOperationKind, TopologyGraph, Transition, TransitionAction,
    TransitionGuard,
};
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct DeviceNode {
    index: NodeIndex,
    kind: DeviceKind,
}

pub fn build_topology_graph(program: &PlcProgram) -> Result<TopologyGraph, Vec<PlcError>> {
    build_topology_from_ast(&program.topology)
}

pub fn build_state_machine(program: &PlcProgram) -> Result<StateMachine, Vec<PlcError>> {
    build_state_machine_from_ast(&program.tasks)
}

pub fn build_topology_from_ast(topology: &TopologySection) -> Result<TopologyGraph, Vec<PlcError>> {
    let mut topology_graph = TopologyGraph::new();
    let mut device_nodes = HashMap::<String, DeviceNode>::new();
    let mut errors = Vec::new();

    for device in &topology.devices {
        let kind = ast_type_to_ir_kind(&device.device_type);
        let index = topology_graph.add_device(Device {
            name: device.name.clone(),
            kind: kind.clone(),
        });

        device_nodes.insert(device.name.clone(), DeviceNode { index, kind });
    }

    for device in &topology.devices {
        let Some(target_name) = device.attributes.connected_to.as_deref() else {
            continue;
        };

        let Some(target_node) = device_nodes.get(target_name) else {
            errors.push(PlcError::semantic(
                device.line,
                format!(
                    "设备 {} 的 connected_to 引用了未定义设备 {}",
                    device.name, target_name
                ),
            ));
            continue;
        };

        let Some(current_node) = device_nodes.get(&device.name) else {
            continue;
        };

        let Some(connection_type) = connection_type_for(&target_node.kind, &current_node.kind)
        else {
            errors.push(PlcError::semantic(
                device.line,
                format!(
                    "设备 {} ({}) 不能 connected_to 设备 {} ({})",
                    device.name,
                    device_kind_name(&current_node.kind),
                    target_name,
                    device_kind_name(&target_node.kind)
                ),
            ));
            continue;
        };

        // `A connected_to B` means B provides upstream linkage into A.
        topology_graph.add_connection(target_node.index, current_node.index, connection_type);
    }

    if errors.is_empty() {
        Ok(topology_graph)
    } else {
        Err(errors)
    }
}

pub fn build_state_machine_from_ast(tasks: &TasksSection) -> Result<StateMachine, Vec<PlcError>> {
    let mut builder = StateMachineBuilder::default();
    let mut errors = Vec::new();

    if tasks.tasks.is_empty() {
        errors.push(PlcError::semantic(1, "[tasks] 段至少需要一个 task"));
        return Err(errors);
    }

    let mut task_initial_states = HashMap::<String, State>::new();

    for task in &tasks.tasks {
        if task.steps.is_empty() {
            errors.push(PlcError::semantic(
                task.line,
                format!("task {} 至少需要一个 step", task.name),
            ));
            continue;
        }

        let initial_state = State {
            task_name: task.name.clone(),
            step_name: task.steps[0].name.clone(),
        };

        if task_initial_states
            .insert(task.name.clone(), initial_state)
            .is_some()
        {
            errors.push(PlcError::semantic(
                task.line,
                format!("task {} 重复定义", task.name),
            ));
        }

        for step in &task.steps {
            builder.add_state(&task.name, &step.name);
        }
    }

    let Some(initial) = tasks.tasks.iter().find_map(|task| {
        task.steps.first().map(|step| State {
            task_name: task.name.clone(),
            step_name: step.name.clone(),
        })
    }) else {
        errors.push(PlcError::semantic(1, "未找到可执行的 task/step 初始状态"));
        return Err(errors);
    };

    let mut task_on_complete_targets = HashMap::<String, Option<State>>::new();
    for task in &tasks.tasks {
        let on_complete_target = match &task.on_complete {
            Some(OnCompleteDirective::Goto { step }) => {
                let line = task.on_complete_line.unwrap_or(task.line);
                resolve_task_target(step, line, &task_initial_states, &mut errors, "on_complete")
            }
            _ => None,
        };
        task_on_complete_targets.insert(task.name.clone(), on_complete_target);
    }

    for task in &tasks.tasks {
        for (step_index, step) in task.steps.iter().enumerate() {
            let from_state = State {
                task_name: task.name.clone(),
                step_name: step.name.clone(),
            };
            let completion_target =
                completion_target_for_step(task, step_index, &task_on_complete_targets);

            let analyzed = analyze_statements(&step.statements);

            for (block_index, block) in analyzed.parallel_blocks.iter().enumerate() {
                build_parallel_block(
                    &mut builder,
                    task,
                    &step.name,
                    &from_state,
                    block_index,
                    block,
                    completion_target.clone(),
                    &task_initial_states,
                    &mut errors,
                    analyzed.actions.clone(),
                );
            }

            for (block_index, block) in analyzed.race_blocks.iter().enumerate() {
                build_race_block(
                    &mut builder,
                    task,
                    &step.name,
                    &from_state,
                    block_index,
                    block,
                    completion_target.clone(),
                    &task_initial_states,
                    &mut errors,
                    analyzed.actions.clone(),
                );
            }

            for goto in &analyzed.gotos {
                if let Some(target) = resolve_task_target(
                    &goto.step,
                    goto.line,
                    &task_initial_states,
                    &mut errors,
                    "goto",
                ) {
                    builder.add_transition(
                        from_state.clone(),
                        target,
                        TransitionGuard::Always,
                        analyzed.actions.clone(),
                        Vec::new(),
                    );
                }
            }

            for (timeout_index, timeout) in analyzed.timeouts.iter().enumerate() {
                if let Some(target) = resolve_task_target(
                    &timeout.target.step,
                    timeout.target.line,
                    &task_initial_states,
                    &mut errors,
                    "timeout -> goto",
                ) {
                    let duration_ms = duration_to_ms(timeout);
                    builder.add_transition(
                        from_state.clone(),
                        target,
                        TransitionGuard::Timeout { duration_ms },
                        Vec::new(),
                        vec![TimerOperation {
                            timer_name: format!(
                                "{}.{}.timeout_{}",
                                task.name,
                                step.name,
                                timeout_index + 1
                            ),
                            operation: TimerOperationKind::Start,
                            duration_ms: Some(duration_ms),
                        }],
                    );
                }
            }

            for wait_expression in &analyzed.waits {
                if let Some(target) = completion_target.clone() {
                    builder.add_transition(
                        from_state.clone(),
                        target,
                        TransitionGuard::Condition {
                            expression: wait_expression.clone(),
                        },
                        analyzed.actions.clone(),
                        Vec::new(),
                    );
                }
            }

            let has_control_flow = !analyzed.waits.is_empty()
                || !analyzed.gotos.is_empty()
                || !analyzed.parallel_blocks.is_empty()
                || !analyzed.race_blocks.is_empty();
            if !has_control_flow {
                if let Some(target) = completion_target {
                    builder.add_transition(
                        from_state,
                        target,
                        TransitionGuard::Always,
                        analyzed.actions,
                        Vec::new(),
                    );
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(StateMachine {
            states: builder.states,
            transitions: builder.transitions,
            initial,
        })
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, Default)]
struct StateMachineBuilder {
    states: Vec<State>,
    transitions: Vec<Transition>,
    seen_states: HashSet<(String, String)>,
}

impl StateMachineBuilder {
    fn add_state(&mut self, task_name: &str, step_name: &str) -> State {
        let key = (task_name.to_string(), step_name.to_string());
        if self.seen_states.insert(key.clone()) {
            self.states.push(State {
                task_name: key.0.clone(),
                step_name: key.1.clone(),
            });
        }

        State {
            task_name: key.0,
            step_name: key.1,
        }
    }

    fn add_transition(
        &mut self,
        from: State,
        to: State,
        guard: TransitionGuard,
        actions: Vec<TransitionAction>,
        timers: Vec<TimerOperation>,
    ) {
        self.transitions.push(Transition {
            from,
            to,
            guard,
            actions,
            timers,
        });
    }
}

#[derive(Debug, Clone, Default)]
struct AnalyzedStatements {
    actions: Vec<TransitionAction>,
    waits: Vec<String>,
    gotos: Vec<GotoDirective>,
    timeouts: Vec<TimeoutDirective>,
    parallel_blocks: Vec<ParallelBlock>,
    race_blocks: Vec<RaceBlock>,
}

fn completion_target_for_step(
    task: &TaskDeclaration,
    step_index: usize,
    task_on_complete_targets: &HashMap<String, Option<State>>,
) -> Option<State> {
    if step_index + 1 < task.steps.len() {
        return Some(State {
            task_name: task.name.clone(),
            step_name: task.steps[step_index + 1].name.clone(),
        });
    }

    task_on_complete_targets
        .get(&task.name)
        .cloned()
        .unwrap_or(None)
}

fn analyze_statements(statements: &[StepStatement]) -> AnalyzedStatements {
    let mut analyzed = AnalyzedStatements::default();

    for statement in statements {
        match statement {
            StepStatement::Action(action) => {
                analyzed.actions.push(action_to_transition_action(action));
            }
            StepStatement::Wait(wait) => {
                analyzed.waits.push(wait_to_guard_expression(wait));
            }
            StepStatement::Timeout(timeout) => analyzed.timeouts.push(timeout.clone()),
            StepStatement::Goto(goto) => analyzed.gotos.push(goto.clone()),
            StepStatement::Parallel(block) => analyzed.parallel_blocks.push(block.clone()),
            StepStatement::Race(block) => analyzed.race_blocks.push(block.clone()),
            StepStatement::AllowIndefiniteWait(_) => {}
        }
    }

    analyzed
}

fn build_parallel_block(
    builder: &mut StateMachineBuilder,
    task: &TaskDeclaration,
    step_name: &str,
    source_state: &State,
    block_index: usize,
    block: &ParallelBlock,
    completion_target: Option<State>,
    task_initial_states: &HashMap<String, State>,
    errors: &mut Vec<PlcError>,
    parent_actions: Vec<TransitionAction>,
) {
    let fork_state_name = format!("{step_name}__parallel_{}_fork", block_index + 1);
    let join_state_name = format!("{step_name}__parallel_{}_join", block_index + 1);

    let fork_state = builder.add_state(&task.name, &fork_state_name);
    let join_state = builder.add_state(&task.name, &join_state_name);

    builder.add_transition(
        source_state.clone(),
        fork_state.clone(),
        TransitionGuard::Always,
        parent_actions,
        Vec::new(),
    );

    for (branch_index, branch) in block.branches.iter().enumerate() {
        let branch_state_name = format!(
            "{step_name}__parallel_{}_branch_{}",
            block_index + 1,
            branch_index + 1
        );
        let branch_state = builder.add_state(&task.name, &branch_state_name);

        builder.add_transition(
            fork_state.clone(),
            branch_state.clone(),
            TransitionGuard::Always,
            Vec::new(),
            Vec::new(),
        );

        let analyzed = analyze_statements(&branch.statements);

        for goto in &analyzed.gotos {
            if let Some(target) =
                resolve_task_target(&goto.step, goto.line, task_initial_states, errors, "goto")
            {
                builder.add_transition(
                    branch_state.clone(),
                    target,
                    TransitionGuard::Always,
                    analyzed.actions.clone(),
                    Vec::new(),
                );
            }
        }

        for (timeout_index, timeout) in analyzed.timeouts.iter().enumerate() {
            if let Some(target) = resolve_task_target(
                &timeout.target.step,
                timeout.target.line,
                task_initial_states,
                errors,
                "timeout -> goto",
            ) {
                let duration_ms = duration_to_ms(timeout);
                builder.add_transition(
                    branch_state.clone(),
                    target,
                    TransitionGuard::Timeout { duration_ms },
                    Vec::new(),
                    vec![TimerOperation {
                        timer_name: format!(
                            "{}.{}.parallel_{}_branch_{}.timeout_{}",
                            task.name,
                            step_name,
                            block_index + 1,
                            branch_index + 1,
                            timeout_index + 1
                        ),
                        operation: TimerOperationKind::Start,
                        duration_ms: Some(duration_ms),
                    }],
                );
            }
        }

        for wait_expression in &analyzed.waits {
            builder.add_transition(
                branch_state.clone(),
                join_state.clone(),
                TransitionGuard::Condition {
                    expression: wait_expression.clone(),
                },
                analyzed.actions.clone(),
                Vec::new(),
            );
        }

        for (nested_parallel_index, nested_parallel) in analyzed.parallel_blocks.iter().enumerate()
        {
            build_parallel_block(
                builder,
                task,
                &format!(
                    "{step_name}__parallel_{}_branch_{}",
                    block_index + 1,
                    branch_index + 1
                ),
                &branch_state,
                nested_parallel_index,
                nested_parallel,
                Some(join_state.clone()),
                task_initial_states,
                errors,
                analyzed.actions.clone(),
            );
        }

        for (nested_race_index, nested_race) in analyzed.race_blocks.iter().enumerate() {
            build_race_block(
                builder,
                task,
                &format!(
                    "{step_name}__parallel_{}_branch_{}",
                    block_index + 1,
                    branch_index + 1
                ),
                &branch_state,
                nested_race_index,
                nested_race,
                Some(join_state.clone()),
                task_initial_states,
                errors,
                analyzed.actions.clone(),
            );
        }

        let has_control_flow = !analyzed.waits.is_empty()
            || !analyzed.gotos.is_empty()
            || !analyzed.parallel_blocks.is_empty()
            || !analyzed.race_blocks.is_empty();
        if !has_control_flow {
            builder.add_transition(
                branch_state,
                join_state.clone(),
                TransitionGuard::Always,
                analyzed.actions,
                Vec::new(),
            );
        }
    }

    if let Some(target) = completion_target {
        builder.add_transition(
            join_state,
            target,
            TransitionGuard::Always,
            Vec::new(),
            Vec::new(),
        );
    }
}

fn build_race_block(
    builder: &mut StateMachineBuilder,
    task: &TaskDeclaration,
    step_name: &str,
    source_state: &State,
    block_index: usize,
    block: &RaceBlock,
    completion_target: Option<State>,
    task_initial_states: &HashMap<String, State>,
    errors: &mut Vec<PlcError>,
    parent_actions: Vec<TransitionAction>,
) {
    let decision_state_name = format!("{step_name}__race_{}_decision", block_index + 1);
    let decision_state = builder.add_state(&task.name, &decision_state_name);

    builder.add_transition(
        source_state.clone(),
        decision_state.clone(),
        TransitionGuard::Always,
        parent_actions,
        Vec::new(),
    );

    for (branch_index, branch) in block.branches.iter().enumerate() {
        let branch_state_name = format!(
            "{step_name}__race_{}_branch_{}",
            block_index + 1,
            branch_index + 1
        );
        let branch_state = builder.add_state(&task.name, &branch_state_name);

        builder.add_transition(
            decision_state.clone(),
            branch_state.clone(),
            TransitionGuard::Always,
            Vec::new(),
            Vec::new(),
        );

        let analyzed = analyze_statements(&branch.statements);
        let branch_completion_target = branch
            .then_goto
            .as_ref()
            .and_then(|goto| {
                resolve_task_target(
                    &goto.step,
                    goto.line,
                    task_initial_states,
                    errors,
                    "race then goto",
                )
            })
            .or_else(|| completion_target.clone());

        for goto in &analyzed.gotos {
            if let Some(target) =
                resolve_task_target(&goto.step, goto.line, task_initial_states, errors, "goto")
            {
                builder.add_transition(
                    branch_state.clone(),
                    target,
                    TransitionGuard::Always,
                    analyzed.actions.clone(),
                    Vec::new(),
                );
            }
        }

        for (timeout_index, timeout) in analyzed.timeouts.iter().enumerate() {
            if let Some(target) = resolve_task_target(
                &timeout.target.step,
                timeout.target.line,
                task_initial_states,
                errors,
                "timeout -> goto",
            ) {
                let duration_ms = duration_to_ms(timeout);
                builder.add_transition(
                    branch_state.clone(),
                    target,
                    TransitionGuard::Timeout { duration_ms },
                    Vec::new(),
                    vec![TimerOperation {
                        timer_name: format!(
                            "{}.{}.race_{}_branch_{}.timeout_{}",
                            task.name,
                            step_name,
                            block_index + 1,
                            branch_index + 1,
                            timeout_index + 1
                        ),
                        operation: TimerOperationKind::Start,
                        duration_ms: Some(duration_ms),
                    }],
                );
            }
        }

        for wait_expression in &analyzed.waits {
            if let Some(target) = branch_completion_target.clone() {
                builder.add_transition(
                    branch_state.clone(),
                    target,
                    TransitionGuard::Condition {
                        expression: wait_expression.clone(),
                    },
                    analyzed.actions.clone(),
                    Vec::new(),
                );
            }
        }

        for (nested_parallel_index, nested_parallel) in analyzed.parallel_blocks.iter().enumerate()
        {
            build_parallel_block(
                builder,
                task,
                &format!(
                    "{step_name}__race_{}_branch_{}",
                    block_index + 1,
                    branch_index + 1
                ),
                &branch_state,
                nested_parallel_index,
                nested_parallel,
                branch_completion_target.clone(),
                task_initial_states,
                errors,
                analyzed.actions.clone(),
            );
        }

        for (nested_race_index, nested_race) in analyzed.race_blocks.iter().enumerate() {
            build_race_block(
                builder,
                task,
                &format!(
                    "{step_name}__race_{}_branch_{}",
                    block_index + 1,
                    branch_index + 1
                ),
                &branch_state,
                nested_race_index,
                nested_race,
                branch_completion_target.clone(),
                task_initial_states,
                errors,
                analyzed.actions.clone(),
            );
        }

        let has_control_flow = !analyzed.waits.is_empty()
            || !analyzed.gotos.is_empty()
            || !analyzed.parallel_blocks.is_empty()
            || !analyzed.race_blocks.is_empty();
        if !has_control_flow {
            if let Some(target) = branch_completion_target {
                builder.add_transition(
                    branch_state,
                    target,
                    TransitionGuard::Always,
                    analyzed.actions,
                    Vec::new(),
                );
            }
        }
    }
}

fn resolve_task_target(
    target_task: &str,
    line: usize,
    task_initial_states: &HashMap<String, State>,
    errors: &mut Vec<PlcError>,
    source: &str,
) -> Option<State> {
    let Some(state) = task_initial_states.get(target_task) else {
        errors.push(PlcError::semantic(
            line,
            format!("{source} 引用了未定义 task {target_task}"),
        ));
        return None;
    };

    Some(state.clone())
}

fn action_to_transition_action(action: &ActionStatement) -> TransitionAction {
    match action {
        ActionStatement::Extend { target } => TransitionAction::Extend {
            target: target.clone(),
        },
        ActionStatement::Retract { target } => TransitionAction::Retract {
            target: target.clone(),
        },
        ActionStatement::Set { target, value } => TransitionAction::Set {
            target: target.clone(),
            value: match value {
                AstBinaryValue::On => IrBinaryValue::On,
                AstBinaryValue::Off => IrBinaryValue::Off,
            },
        },
        ActionStatement::Log { message } => TransitionAction::Log {
            message: message.clone(),
        },
    }
}

fn wait_to_guard_expression(wait: &WaitStatement) -> String {
    condition_to_expression(&wait.condition)
}

fn condition_to_expression(condition: &ConditionExpression) -> String {
    let operator = match condition.operator {
        ComparisonOperator::Eq => "==",
        ComparisonOperator::Neq => "!=",
    };

    format!(
        "{} {} {}",
        condition.left,
        operator,
        literal_to_expression(&condition.right)
    )
}

fn literal_to_expression(literal: &LiteralValue) -> String {
    match literal {
        LiteralValue::Boolean(value) => value.to_string(),
        LiteralValue::Number(value) => value.to_string(),
        LiteralValue::String(value) => format!("\"{}\"", value),
        LiteralValue::State(state) => format!("{}.{}", state.device, state.state),
    }
}

fn duration_to_ms(timeout: &TimeoutDirective) -> u64 {
    match timeout.duration.unit {
        TimeUnit::Ms => timeout.duration.value,
        TimeUnit::S => timeout.duration.value.saturating_mul(1000),
    }
}

fn ast_type_to_ir_kind(device_type: &DeviceType) -> DeviceKind {
    match device_type {
        DeviceType::DigitalOutput => DeviceKind::DigitalOutput,
        DeviceType::DigitalInput => DeviceKind::DigitalInput,
        DeviceType::SolenoidValve => DeviceKind::SolenoidValve,
        DeviceType::Cylinder => DeviceKind::Cylinder,
        DeviceType::Sensor => DeviceKind::Sensor,
        DeviceType::Motor => DeviceKind::Motor,
    }
}

fn connection_type_for(from: &DeviceKind, to: &DeviceKind) -> Option<ConnectionType> {
    match (from, to) {
        (DeviceKind::DigitalOutput, DeviceKind::SolenoidValve)
        | (DeviceKind::DigitalOutput, DeviceKind::Motor)
        | (DeviceKind::DigitalInput, DeviceKind::Sensor) => Some(ConnectionType::Electrical),
        (DeviceKind::SolenoidValve, DeviceKind::Cylinder) => Some(ConnectionType::Pneumatic),
        (DeviceKind::DigitalInput, DeviceKind::DigitalInput)
        | (DeviceKind::DigitalOutput, DeviceKind::DigitalOutput) => Some(ConnectionType::Logical),
        _ => None,
    }
}

fn device_kind_name(kind: &DeviceKind) -> &'static str {
    match kind {
        DeviceKind::DigitalOutput => "digital_output",
        DeviceKind::DigitalInput => "digital_input",
        DeviceKind::SolenoidValve => "solenoid_valve",
        DeviceKind::Cylinder => "cylinder",
        DeviceKind::Sensor => "sensor",
        DeviceKind::Motor => "motor",
    }
}

#[cfg(test)]
mod tests {
    use super::{build_state_machine, build_topology_graph};
    use crate::ir::{ConnectionType, TransitionGuard};
    use crate::parser::parse_plc;
    use petgraph::visit::EdgeRef;

    #[test]
    fn builds_topology_graph_from_prd_5_3_topology() {
        let input = r#"
[topology]

# ===== controller ports =====
device Y0: digital_output
device Y1: digital_output
device Y2: digital_output
device X0: digital_input
device X1: digital_input
device X2: digital_input
device X3: digital_input
device X4: digital_input

# ===== operator panel =====
device start_button: digital_input {
    connected_to: X4,
    debounce: 20ms
}

device alarm_light: digital_output {
    connected_to: Y2
}

# ===== solenoid valves =====
device valve_A: solenoid_valve {
    type: "5/2",
    connected_to: Y0,
    response_time: 15ms
}

device valve_B: solenoid_valve {
    type: "5/2",
    connected_to: Y1,
    response_time: 15ms
}

# ===== cylinders =====
device cyl_A: cylinder {
    type: double_acting,
    connected_to: valve_A,
    stroke: 100mm,
    stroke_time: 200ms,
    retract_time: 180ms
}

device cyl_B: cylinder {
    type: double_acting,
    connected_to: valve_B,
    stroke: 150mm,
    stroke_time: 300ms,
    retract_time: 250ms
}

# ===== sensors =====
device sensor_A_ext: sensor {
    type: magnetic,
    connected_to: X0,
    detects: cyl_A.extended
}

device sensor_A_ret: sensor {
    type: magnetic,
    connected_to: X1,
    detects: cyl_A.retracted
}

device sensor_B_ext: sensor {
    type: magnetic,
    connected_to: X2,
    detects: cyl_B.extended
}

device sensor_B_ret: sensor {
    type: magnetic,
    connected_to: X3,
    detects: cyl_B.retracted
}

[constraints]

[tasks]
"#;

        let program = parse_plc(input).expect("PRD 5.3 示例应能成功解析为 AST");
        let topology = build_topology_graph(&program).expect("PRD 5.3 示例应能成功构建拓扑图");

        assert_eq!(topology.graph.node_count(), 18);
        assert_eq!(topology.graph.edge_count(), 10);

        let has_pneumatic_edge = topology.graph.edge_references().any(|edge| {
            let source = &topology.graph[edge.source()].name;
            let target = &topology.graph[edge.target()].name;
            source == "valve_A" && target == "cyl_A" && edge.weight() == &ConnectionType::Pneumatic
        });
        assert!(has_pneumatic_edge, "应包含 valve_A -> cyl_A 气路连接");

        let has_electrical_edge = topology.graph.edge_references().any(|edge| {
            let source = &topology.graph[edge.source()].name;
            let target = &topology.graph[edge.target()].name;
            source == "Y0" && target == "valve_A" && edge.weight() == &ConnectionType::Electrical
        });
        assert!(has_electrical_edge, "应包含 Y0 -> valve_A 电气连接");
    }

    #[test]
    fn reports_error_when_connected_to_references_undefined_device() {
        let input = r#"
[topology]
device Y0: digital_output

device valve_A: solenoid_valve {
    connected_to: Y9,
    response_time: 15ms
}

[constraints]

[tasks]
"#;

        let program = parse_plc(input).expect("测试输入应能解析为 AST");
        let errors = build_topology_graph(&program).expect_err("未定义 connected_to 引用应报错");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line(), 5);
        assert!(
            errors[0].to_string().contains("未定义设备 Y9"),
            "错误消息应包含未定义设备名"
        );
    }

    #[test]
    fn reports_error_when_connection_types_are_incompatible() {
        let input = r#"
[topology]
device cyl_A: cylinder {
    connected_to: valve_A,
    stroke_time: 200ms,
    retract_time: 180ms
}

device valve_A: solenoid_valve {
    connected_to: Y0,
    response_time: 15ms
}

device sensor_bad: sensor {
    connected_to: cyl_A,
    detects: cyl_A.extended
}

device Y0: digital_output

[constraints]

[tasks]
"#;

        let program = parse_plc(input).expect("测试输入应能解析为 AST");
        let errors = build_topology_graph(&program).expect_err("不兼容连接类型应报错");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line(), 14);
        assert!(
            errors[0].to_string().contains("sensor") && errors[0].to_string().contains("cylinder"),
            "错误消息应包含不兼容的设备类型"
        );
    }

    #[test]
    fn builds_state_machine_from_prd_5_5_1_sequence_example() {
        let input = r#"
[topology]

[constraints]

[tasks]

task init:
    step extend_A:
        action: extend cyl_A
        wait: sensor_A_ext == true
        timeout: 600ms -> goto fault_handler

    step retract_A:
        action: retract cyl_A
        wait: sensor_A_ret == true
        timeout: 500ms -> goto fault_handler

    step extend_B:
        action: extend cyl_B
        wait: sensor_B_ext == true
        timeout: 800ms -> goto fault_handler

    step retract_B:
        action: retract cyl_B
        wait: sensor_B_ret == true
        timeout: 700ms -> goto fault_handler

    on_complete: goto ready

task fault_handler:
    step safe_position:
        action: retract cyl_A
    on_complete: goto ready

task ready:
    step wait_start:
        wait: start_button == true
        allow_indefinite_wait: true
    on_complete: goto init
"#;

        let program = parse_plc(input).expect("PRD 5.5.1 示例应能成功解析为 AST");
        let state_machine = build_state_machine(&program).expect("应能从 5.5.1 示例构建状态机");

        assert!(
            state_machine
                .states
                .iter()
                .any(|state| state.task_name == "init" && state.step_name == "extend_A")
        );
        assert!(
            state_machine
                .states
                .iter()
                .any(|state| state.task_name == "init" && state.step_name == "retract_B")
        );

        let has_wait_transition = state_machine.transitions.iter().any(|transition| {
            transition.from.task_name == "init"
                && transition.from.step_name == "extend_A"
                && transition.to.task_name == "init"
                && transition.to.step_name == "retract_A"
                && matches!(
                    transition.guard,
                    TransitionGuard::Condition { ref expression }
                        if expression == "sensor_A_ext == true"
                )
        });
        assert!(has_wait_transition, "应存在 wait 条件驱动的顺序转移");

        let has_timeout_transition = state_machine.transitions.iter().any(|transition| {
            transition.from.task_name == "init"
                && transition.from.step_name == "extend_A"
                && transition.to.task_name == "fault_handler"
                && transition.to.step_name == "safe_position"
                && matches!(
                    transition.guard,
                    TransitionGuard::Timeout { duration_ms } if duration_ms == 600
                )
        });
        assert!(has_timeout_transition, "timeout 应创建带定时守卫的跳转");

        let has_on_complete_goto = state_machine.transitions.iter().any(|transition| {
            transition.from.task_name == "init"
                && transition.from.step_name == "retract_B"
                && transition.to.task_name == "ready"
                && transition.to.step_name == "wait_start"
                && matches!(
                    transition.guard,
                    TransitionGuard::Condition { ref expression }
                        if expression == "sensor_B_ret == true"
                )
        });
        assert!(
            has_on_complete_goto,
            "最后一步应能够通过 on_complete 跳转到 ready"
        );
    }

    #[test]
    fn builds_state_machine_race_branches_from_prd_9_example() {
        let input = r#"
[topology]

[constraints]

[tasks]

task search:
    step start_motor:
        action: set motor_ctrl on
    step detect:
        race:
            branch_A:
                wait: sensor_A == true
                then: goto process_A
            branch_B:
                wait: sensor_B == true
                then: goto process_B
        timeout: 800ms -> goto motor_fault

task process_A:
    step stop_motor:
        action: set motor_ctrl off
    on_complete: goto ready

task process_B:
    step stop_motor:
        action: set motor_ctrl off
    on_complete: goto ready

task motor_fault:
    step emergency_stop:
        action: set motor_ctrl off
    on_complete: goto ready

task ready:
    step wait_start:
        wait: start_button == true
        allow_indefinite_wait: true
    on_complete: goto search
"#;

        let program = parse_plc(input).expect("PRD 9 示例应能成功解析为 AST");
        let state_machine = build_state_machine(&program).expect("应能构建 race 状态机");

        assert!(state_machine.states.iter().any(
            |state| state.task_name == "search" && state.step_name == "detect__race_1_decision"
        ));
        assert!(state_machine.states.iter().any(
            |state| state.task_name == "search" && state.step_name == "detect__race_1_branch_1"
        ));

        let has_branch_a_transition = state_machine.transitions.iter().any(|transition| {
            transition.from.task_name == "search"
                && transition.from.step_name == "detect__race_1_branch_1"
                && transition.to.task_name == "process_A"
                && transition.to.step_name == "stop_motor"
                && matches!(
                    transition.guard,
                    TransitionGuard::Condition { ref expression }
                        if expression == "sensor_A == true"
                )
        });
        assert!(has_branch_a_transition, "race 分支 A 应创建条件跳转");

        let has_branch_b_transition = state_machine.transitions.iter().any(|transition| {
            transition.from.task_name == "search"
                && transition.from.step_name == "detect__race_1_branch_2"
                && transition.to.task_name == "process_B"
                && transition.to.step_name == "stop_motor"
                && matches!(
                    transition.guard,
                    TransitionGuard::Condition { ref expression }
                        if expression == "sensor_B == true"
                )
        });
        assert!(has_branch_b_transition, "race 分支 B 应创建条件跳转");

        let has_timeout_transition = state_machine.transitions.iter().any(|transition| {
            transition.from.task_name == "search"
                && transition.from.step_name == "detect"
                && transition.to.task_name == "motor_fault"
                && transition.to.step_name == "emergency_stop"
                && matches!(
                    transition.guard,
                    TransitionGuard::Timeout { duration_ms } if duration_ms == 800
                )
        });
        assert!(
            has_timeout_transition,
            "race 所在 step 应保留 timeout 守卫跳转"
        );
    }

    #[test]
    fn reports_undefined_goto_target_with_line_number() {
        let input = r#"
[topology]

[constraints]

[tasks]

task init:
    step start:
        goto missing_task
"#;

        let program = parse_plc(input).expect("测试输入应能成功解析为 AST");
        let errors = build_state_machine(&program).expect_err("未定义 goto 目标应返回语义错误");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line(), 10);
        assert!(
            errors[0].to_string().contains("未定义 task missing_task"),
            "错误消息应包含未定义 task 名称"
        );
    }
}
