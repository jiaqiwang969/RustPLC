use crate::ast::{DeviceType, PlcProgram, TopologySection};
use crate::error::PlcError;
use crate::ir::{ConnectionType, Device, DeviceKind, TopologyGraph};
use petgraph::graph::NodeIndex;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct DeviceNode {
    index: NodeIndex,
    kind: DeviceKind,
}

pub fn build_topology_graph(program: &PlcProgram) -> Result<TopologyGraph, Vec<PlcError>> {
    build_topology_from_ast(&program.topology)
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
    use super::build_topology_graph;
    use crate::ir::ConnectionType;
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
}
