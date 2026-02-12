<p align="center">
  <h1 align="center">RustPLC</h1>
  <p align="center">
    <strong>形式化验证的工业控制编译器</strong><br>
    不写程序控制设备 —— 声明物理事实与意图，让编译器证明它是安全的。
  </p>
</p>

<p align="center">
  <a href="#快速开始">快速开始</a> ·
  <a href="#为什么需要-rustplc">为什么</a> ·
  <a href="#dsl-语言一览">DSL 语言</a> ·
  <a href="#四大验证引擎">验证引擎</a> ·
  <a href="#示例">示例</a>
</p>

---

## 为什么需要 RustPLC

传统 PLC 编程（梯形图 / ST / FBD）依赖工程师的经验来保证安全性。当系统复杂度上升，人工审查的可靠性急剧下降——气缸碰撞、死锁、超时这些问题往往在现场调试时才暴露。

RustPLC 换了一种思路：

- 用声明式 DSL 描述物理拓扑、控制逻辑和安全约束
- 编译期自动执行形式化验证，在代码运行之前证明安全性
- 错误信息精确到行号，附带修复建议

**安全不靠测试覆盖率，靠数学证明。**

## 四大验证引擎

| 引擎 | 检查内容 | 方法 |
|------|---------|------|
| **Safety** | 状态互斥冲突（如两缸同时伸出） | 有界模型检查 + k-归纳 |
| **Liveness** | 死锁 / 活锁（无超时的 wait、零出度状态） | SCC 分析 + 可达性检查 |
| **Timing** | 时序包络（动作是否在约束时间内完成） | 最坏关键路径计算 |
| **Causality** | 因果链完整性（信号能否从输出传递到传感器） | 拓扑图 BFS 最短路径 |

四个引擎并行运行，一次编译暴露所有问题。

## DSL 语言一览

一个 `.plc` 文件由三个段组成：

```plc
[topology]          # 声明物理设备与连接关系
[constraints]       # 声明安全、时序、因果约束
[tasks]             # 声明控制逻辑（状态机）
```

### 拓扑 —— 描述你的物理世界

```plc
[topology]

device Y0: digital_output
device valve_A: solenoid_valve {
    connected_to: Y0
    response_time: 20ms
}
device cyl_A: cylinder {
    connected_to: valve_A
    stroke_time: 300ms
    retract_time: 300ms
}
device sensor_A_ext: sensor {
    connected_to: X0
    detects: cyl_A.extended
}
```

支持的设备类型：`digital_output` / `digital_input` / `solenoid_valve` / `cylinder` / `motor` / `sensor`

### 约束 —— 声明安全红线

```plc
[constraints]

safety: cyl_A.extended conflicts_with cyl_B.extended
    reason: "A缸和B缸不能同时伸出"

timing: task.init must_complete_within 5000ms

causality: Y0 -> valve_A -> cyl_A -> sensor_A_ext
```

### 任务 —— 控制逻辑即状态机

```plc
[tasks]

task init:
    step extend_A:
        action: extend cyl_A
        wait: sensor_A_ext == true
        timeout: 500ms -> goto fault_handler
    step retract_A:
        action: retract cyl_A
        wait: sensor_A_ret == true
        timeout: 500ms -> goto fault_handler
    on_complete: goto ready
```

还支持 `parallel`（并行分支）、`race`（竞争分支，先到先得）、`allow_indefinite_wait`（人工等待豁免）等控制结构。

## 快速开始

```bash
# 克隆
git clone https://github.com/xxrust/RustPLC.git
cd RustPLC

# 编译
cargo build --release

# 验证一个 .plc 文件
cargo run --release -- examples/two_cylinder.plc
```

输出：

```
验证通过：
  - Safety: 完备证明（深度 8）
  - Liveness: 通过
  - Timing: 通过
  - Causality: 通过
```

验证通过后，编译器输出完整的 IR（JSON 格式），包含拓扑图、状态机、约束集、时序模型和验证摘要。

### 可选：启用 Z3 求解器

```bash
cargo build --release --features z3-solver
```

启用后 Safety 引擎将使用 Z3 SMT 求解器进行更强的互斥性证明。

## 示例

`examples/` 目录包含四个示例文件：

| 文件 | 场景 | 验证结果 |
|------|------|---------|
| `two_cylinder.plc` | 双缸顺序动作 + 安全互斥 | 全部通过 |
| `half_rotation.plc` | 电机半圈旋转 + race 竞争分支 | 全部通过 |
| `error_all_verifiers.plc` | 故意触发四个引擎全部报错 | 四项失败 |
| `error_missing_device.plc` | 引用未定义设备 | 语义错误 |

### 错误报告示例

当验证失败时，错误信息精确定位问题并给出建议：

```
ERROR [liveness] 潜在死锁
  位置: task main.step_wait
  原因: wait 条件缺少 timeout 分支
  建议: 请添加 timeout: <时长> -> goto <恢复 task>

ERROR [timing] 时序超限
  位置: task main
  约束: must_complete_within 50ms
  实际最坏路径: 220ms
  建议: 请增大约束值或优化动作时序

ERROR [causality] 因果链断裂
  声明链路: Y0 -> valve_A -> cyl_B -> sensor_B_ext
  断裂位置: valve_A -> cyl_B
  建议: 请检查 cyl_B 的 connected_to 配置
```

## 编译流水线

```
.plc 源文件
    │
    ▼
  Parser (pest PEG)        ──→  AST
    │
    ▼
  Semantic Analysis         ──→  IR（拓扑图 / 状态机 / 约束集 / 时序模型）
    │
    ▼
  Formal Verification       ──→  Safety + Liveness + Timing + Causality
    │
    ▼
  JSON IR 输出（未来：代码生成 → 确定性执行内核）
```

## 测试

```bash
# 运行全部 52 个测试（41 单元 + 5 集成 + 6 端到端验证）
cargo test
```

测试覆盖：解析器、语义分析、四个验证引擎的正向/反向用例，以及从单缸到双工位工业场景的端到端验证。

## 技术栈

- **Rust 2024 Edition** —— 内存安全，零成本抽象
- **pest** —— PEG 解析器生成器
- **petgraph** —— 图数据结构（拓扑图 + 状态机）
- **Z3**（可选）—— SMT 求解器，增强安全性证明

## 路线图

- [x] DSL 设计与解析器
- [x] AST → IR 语义分析
- [x] 四大形式化验证引擎
- [x] 结构化错误报告（行号 + 修复建议）
- [ ] 代码生成 → 确定性 Rust 执行内核
- [ ] 硬件抽象层（EtherCAT / Modbus / GPIO）
- [ ] 模拟量 I/O 与 PID 控制
- [ ] 多控制器协同
- [ ] 图形化 DSL 编辑器

## License

MIT

---

<p align="center">
  <sub>用 Rust 写的，所以它不会 panic。好吧，至少不会在生产线上。</sub>
</p>
