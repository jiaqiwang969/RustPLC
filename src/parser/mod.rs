use pest::Parser;

#[derive(pest_derive::Parser)]
#[grammar = "parser/plc.pest"]
pub struct PlcParser;

pub fn parse_topology(input: &str) -> Result<(), pest::error::Error<Rule>> {
    PlcParser::parse(Rule::topology_file, input).map(|_| ())
}

pub fn parse_constraints(input: &str) -> Result<(), pest::error::Error<Rule>> {
    PlcParser::parse(Rule::constraints_file, input).map(|_| ())
}

pub fn parse_tasks(input: &str) -> Result<(), pest::error::Error<Rule>> {
    PlcParser::parse(Rule::tasks_file, input).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{parse_constraints, parse_tasks, parse_topology};

    #[test]
    fn parses_prd_5_3_topology_example() {
        let input = r#"
[topology]

# ===== controller ports =====
device Y0: digital_output               # digital output port
device Y1: digital_output
device Y2: digital_output               # alarm light output
device X0: digital_input                # digital input port
device X1: digital_input
device X2: digital_input
device X3: digital_input
device X4: digital_input                # start button

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
"#;

        assert!(parse_topology(input).is_ok());
    }

    #[test]
    fn parses_all_topology_device_types_and_property_shapes() {
        let input = r#"
[topology]

device Y3: digital_output
device X5: digital_input

device estop: digital_input {
    connected_to: X5,
    debounce: 10ms,
    inverted: true
}

device spindle_valve: solenoid_valve {
    connected_to: Y3,
    response_time: 25ms,
    type: "3/2"
}

device spindle_cyl: cylinder {
    connected_to: spindle_valve,
    stroke_time: 120ms,
    retract_time: 110ms,
    stroke: 80mm,
    type: compact
}

device spindle_sensor: sensor {
    connected_to: X5,
    detects: spindle_cyl.extended,
    type: optical
}

device spindle_motor: motor {
    connected_to: Y3,
    rated_speed: 60rpm,
    ramp_time: 300ms
}
"#;

        assert!(parse_topology(input).is_ok());
    }

    #[test]
    fn parses_prd_5_4_constraints_example() {
        let input = r#"
[constraints]

# ===== 状态互斥 (Safety) =====
safety: cyl_A.extended conflicts_with cyl_B.extended
    reason: "A缸和B缸同时伸出会导致机械碰撞"

safety: valve_A.on conflicts_with valve_B.on
    reason: "气源压力不足以同时驱动两个阀"

# ===== 时序约束 (Timing) =====
timing: task.init must_complete_within 5000ms
    reason: "初始化超过5秒视为异常"

timing: task.init.step_extend_A must_complete_within 500ms
    reason: "单步动作不应超过500ms"

# ===== 因果链声明 (Causality) =====
causality: Y0 -> valve_A -> cyl_A -> sensor_A_ext
    reason: "Y0 驱动 valve_A 推动 cyl_A 由 sensor_A_ext 检测"

causality: Y1 -> valve_B -> cyl_B -> sensor_B_ext
    reason: "Y1 驱动 valve_B 推动 cyl_B 由 sensor_B_ext 检测"
"#;

        assert!(parse_constraints(input).is_ok());
    }

    #[test]
    fn parses_requires_and_must_start_after_constraints() {
        let input = r#"
[constraints]

safety: sensor_A_ext.on requires valve_A.on
timing: task.ready must_start_after 120ms
causality: X0 -> relay_A -> valve_A
"#;

        assert!(parse_constraints(input).is_ok());
    }

    #[test]
    fn parses_prd_5_5_1_basic_sequence_tasks_example() {
        let input = r#"
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
"#;

        assert!(parse_tasks(input).is_ok());
    }

    #[test]
    fn parses_prd_5_5_2_wait_and_jump_tasks_example() {
        let input = r#"
[tasks]

task ready:
    step wait_start:
        wait: start_button == true
        allow_indefinite_wait: true
    on_complete: goto main_cycle
"#;

        assert!(parse_tasks(input).is_ok());
    }

    #[test]
    fn parses_prd_5_5_3_fault_handler_tasks_example() {
        let input = r#"
[tasks]

task fault_handler:
    step safe_position:
        action: retract cyl_A
        action: retract cyl_B
    step alarm:
        action: set alarm_light on
        action: log "动作超时，已执行安全复位"
    on_complete: goto ready
"#;

        assert!(parse_tasks(input).is_ok());
    }

    #[test]
    fn parses_prd_5_5_4_parallel_tasks_example() {
        let input = r#"
[tasks]

task parallel_demo:
    step move_together:
        parallel:
            branch_A:
                action: extend cyl_A
                wait: sensor_A_ext == true
                timeout: 600ms -> goto fault_handler
            branch_B:
                action: extend cyl_B
                wait: sensor_B_ext == true
                timeout: 800ms -> goto fault_handler
    on_complete: goto next_task
"#;

        assert!(parse_tasks(input).is_ok());
    }

    #[test]
    fn parses_prd_5_5_5_race_tasks_example() {
        let input = r#"
[tasks]

task search_position:
    step start_motor:
        action: set motor on
    step detect:
        race:
            branch_A:
                wait: sensor_A == true
                then: goto process_A
            branch_B:
                wait: sensor_B == true
                then: goto process_B
        timeout: 2000ms -> goto fault_handler
    on_complete: unreachable
"#;

        assert!(parse_tasks(input).is_ok());
    }
}
