/// Parse guard condition expressions like "sensor_A == true" into Rust code.

pub struct ParsedCondition {
    pub device: String,
    pub op: String,
    pub value: String,
}

/// Parse "device_name == true" or "device_name != false" into components.
pub fn parse_guard_expression(expr: &str) -> Option<ParsedCondition> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }

    let device = parts[0].to_string();
    let op = parts[1].to_string();
    let value = parts[2].to_string();

    if op != "==" && op != "!=" {
        return None;
    }

    Some(ParsedCondition { device, op, value })
}

/// Emit Rust code for a guard condition.
/// "sensor_A == true" → "hal.read_digital_input(\"sensor_A\") == true"
pub fn emit_condition(expr: &str) -> String {
    match parse_guard_expression(expr) {
        Some(cond) => {
            let rust_val = match cond.value.as_str() {
                "true" => "true".to_string(),
                "false" => "false".to_string(),
                other => other.to_string(),
            };
            format!(
                "hal.read_digital_input(\"{}\") {} {}",
                cond.device, cond.op, rust_val
            )
        }
        None => format!("/* unparsed: {expr} */ true"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_eq_true() {
        let c = parse_guard_expression("sensor_A == true").unwrap();
        assert_eq!(c.device, "sensor_A");
        assert_eq!(c.op, "==");
        assert_eq!(c.value, "true");
    }

    #[test]
    fn parses_neq_false() {
        let c = parse_guard_expression("start_button != false").unwrap();
        assert_eq!(c.device, "start_button");
        assert_eq!(c.op, "!=");
        assert_eq!(c.value, "false");
    }

    #[test]
    fn emits_hal_read_call() {
        let code = emit_condition("sensor_A == true");
        assert_eq!(code, "hal.read_digital_input(\"sensor_A\") == true");
    }

    #[test]
    fn returns_none_for_invalid() {
        assert!(parse_guard_expression("bad").is_none());
    }
}
