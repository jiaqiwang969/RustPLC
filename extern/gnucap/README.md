# 188-Gnucap — 电路仿真

## 角色

在数字孪生平台中充当 **Layer 6: 电路仿真**。

模拟 PLC 输出端到执行器之间的电气特性：
- DO → 继电器线圈 → 电磁阀（吸合延迟 ~15ms）
- DI ← 传感器触点 ← 信号调理电路（RC 滤波特性）

## 架构位置

```
RustPLC runtime
  → HAL write_digital_output("valve_1", true)
    → Modbus coil write
      → Gnucap: 线圈电路仿真
        → 电磁阀吸合延迟 15ms
          → 气缸动作
```

## 本地路径

```
/Users/jqwang/188-Gnucap
```

## 接口

- spec → 网表 → SPICE 仿真 → 指标提取
- 仿真输出注入为 DI 延迟参数
- socat 虚拟串口对接 Modbus RTU slave
