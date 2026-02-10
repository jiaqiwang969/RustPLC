use pest::Parser;

#[derive(pest_derive::Parser)]
#[grammar = "parser/plc.pest"]
pub struct PlcParser;

pub fn parse_topology(input: &str) -> Result<(), pest::error::Error<Rule>> {
    PlcParser::parse(Rule::topology_file, input).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::parse_topology;

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
}
