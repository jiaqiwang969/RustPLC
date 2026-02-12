<p align="center">
  <h1 align="center">RustPLC</h1>
  <p align="center">
    <strong>工业控制数字孪生平台</strong><br>
    声明物理事实与安全意图，让编译器证明它是安全的；<br>
    然后生成确定性执行内核，驱动虚拟工厂或真实硬件。
  </p>
</p>

<p align="center">
  <a href="#完整闭环">闭环架构</a> ·
  <a href="#限界上下文ddd">DDD 设计</a> ·
  <a href="#dsl-语言一览">DSL</a> ·
  <a href="#四大验证引擎">验证</a> ·
  <a href="#快速开始">快速开始</a> ·
  <a href="#路线图">路线图</a>
</p>

---

## 完整闭环

这不是一个分层架构，而是一条从源码到物理世界的完整数据流。每个箭头都是真实的数据传递，不是抽象的"依赖"。

```
.plc 源码
  │
  ▼
199-RustPLC ──→ 形式化验证 (安全/活性/时序/因果)
  │
  ▼ codegen
Rust binary (状态机 + HAL trait)
  │
  ├──→ 模式 A: 纯虚拟 ──→ SimBackend (CI/单元测试)
  │
  ├──→ 模式 B: 虚拟工厂 ──→ 196-ESXI-ARM
  │         VM1: runtime (Modbus master)
  │         VM2: 虚拟 I/O slave
  │              │
  │              ├── 188-Gnucap: 线圈吸合 15ms → DI 延迟注入
  │              │                传感器信号上升沿 → 滤波特性
  │              │
  │              └── jtufem-rs: 气缸活塞杆受力 → 变形量
  │                             夹具结构 → 应力分布
  │                             支架共振 → 模态频率
  │                             ↓
  │                             物理仿真参数 (行程时间、
  │                             最大负载、疲劳寿命)
  │
  └──→ 模式 C: 虚实混合 ──→ iCESugar-pro FPGA
            UART Modbus RTU slave (硬件状态机)
            PMOD GPIO → 继电器/LED/按钮/传感器
            ↓
            真实电气信号，真实时序
```

关键点：三种模式共享同一份生成代码，只是 `HalBackend` 的实现不同。验证在编译期完成，运行时不再做安全检查——因为已经被数学证明了。

## 限界上下文（DDD）

按 Domain-Driven Design 划分为 5 个限界上下文（Bounded Context），每个上下文有明确的职责边界和上下游关系。这个划分决定了 crate 的拆分方式，也为后续重构提供了稳定的接缝。

```
┌─────────────────────────────────────────────────────────────────┐
│  BC1: 验证域 (Verification Domain)                              │
│  职责: .plc → AST → IR → 形式化证明                              │
│  输出: 验证报告 + IR (拓扑图/状态机/约束集/时序模型)               │
│  crate: rustplc_compiler                                        │
│                                                                 │
│  上游: 无 (源头)                                                 │
│  下游: BC2 (IR 作为代码生成的输入)                                │
├─────────────────────────────────────────────────────────────────┤
│  BC2: 执行域 (Execution Domain)                                  │
│  职责: IR → Rust 状态机 + 扫描周期引擎                            │
│  输出: 可执行 binary (PlcState enum + scan_cycle fn)             │
│  crate: rustplc_codegen, rustplc_runtime                         │
│                                                                 │
│  上游: BC1 (消费 IR)                                             │
│  下游: BC3 (通过 HalBackend trait 解耦)                          │
├─────────────────────────────────────────────────────────────────┤
│  BC3: 硬件适配域 (Hardware Adaptation Domain)                    │
│  职责: 统一 I/O 接口，屏蔽物理差异                                │
│  核心契约: HalBackend trait                                      │
│  实现:                                                           │
│    SimBackend ──→ 模式 A (内存 HashMap)                          │
│    ModbusBackend ──→ 模式 B/C (tokio-modbus)                    │
│    FpgaBackend ──→ 模式 C (iCESugar-pro SPI/UART)              │
│  crate: rustplc_hal, rustplc_modbus, rustplc_fpga               │
│                                                                 │
│  上游: BC2 (被 ScanCycleEngine 调用)                             │
│  下游: BC4 (Modbus 帧 → 虚拟从站), BC5 (GPIO → 实体硬件)        │
├─────────────────────────────────────────────────────────────────┤
│  BC4: 仿真域 (Simulation Domain)                                 │
│  职责: 虚拟 I/O 从站 + 电路/力学仿真闭环                         │
│  数据流:                                                         │
│    Modbus 写 coil → 188-Gnucap 电路仿真                         │
│      → 线圈吸合延迟 15ms → DI 状态变更                           │
│      → 传感器 RC 滤波 → 信号上升沿延迟                            │
│    Modbus 写 coil → jtufem-rs FEM 仿真                          │
│      → 气缸活塞杆应力 → 行程时间修正                              │
│      → 夹具变形量 → 精度影响                                     │
│      → 支架模态频率 → 共振预警                                    │
│      → 疲劳寿命 → 维护周期预测                                    │
│  外部项目: 196-ESXI-ARM, 188-Gnucap, jtufem-rs                  │
│                                                                 │
│  上游: BC3 (接收 Modbus 帧)                                      │
│  下游: BC3 (仿真结果回写为 DI 状态，闭环)                         │
├─────────────────────────────────────────────────────────────────┤
│  BC5: 硬件域 (Physical Hardware Domain)                          │
│  职责: FPGA 确定性 I/O + 真实传感器/执行器                        │
│  数据流:                                                         │
│    UART Modbus RTU → FPGA 硬件状态机                             │
│      → PMOD GPIO → 继电器/电磁阀/LED                             │
│      → PMOD GPIO ← 按钮/接近传感器/磁性开关                      │
│      → 真实电气信号，纳秒级确定性时序                              │
│  外部项目: iCESugar-pro v1.3 (Lattice ECP5)                     │
│                                                                 │
│  上游: BC3 (接收 Modbus RTU 帧)                                  │
│  下游: BC3 (GPIO 状态回读为 DI，闭环)                             │
└─────────────────────────────────────────────────────────────────┘
```

上下文之间的集成模式：

| 上游 → 下游 | 集成模式 | 契约 |
|-------------|---------|------|
| BC1 → BC2 | 共享内核 (Shared Kernel) | `rustplc_ir` crate (StateMachine, TopologyGraph) |
| BC2 → BC3 | 依赖倒置 (Dependency Inversion) | `HalBackend` trait |
| BC3 → BC4 | 发布语言 (Published Language) | Modbus RTU/TCP 协议帧 |
| BC3 → BC5 | 发布语言 (Published Language) | Modbus RTU 协议帧 |
| BC4 → BC3 | 遵从者 (Conformist) | Modbus 寄存器映射 (coils/DI) |
| BC5 → BC3 | 遵从者 (Conformist) | Modbus 寄存器映射 (coils/DI) |

BC4 和 BC5 都是闭环的下半段——它们接收控制指令，经过仿真或真实物理过程，将结果回写为输入信号，形成完整的控制闭环。

## 项目结构

```
RustPLC/
├── crates/
│   ├── rustplc_compiler/       # BC1: 解析 → AST → IR → 验证
│   ├── rustplc_ir/             # 共享内核: IR 数据结构 (BC1↔BC2)
│   ├── rustplc_codegen/        # BC2: IR → Rust 状态机代码生成
│   ├── rustplc_runtime/        # BC2: 扫描周期引擎 + 定时器组
│   ├── rustplc_hal/            # BC3: HalBackend trait + SimBackend + 配置
│   ├── rustplc_modbus/         # BC3: [stub] Modbus RTU/TCP 后端
│   ├── rustplc_fpga/           # BC3/BC5: [stub] FPGA I/O 后端
│   └── rustplc_orchestrator/   # 跨 BC: [stub] 模式选择 + 启动编排
├── extern/                     # BC4/BC5: 外部项目引用
│   ├── esxi-arm/               #   196-ESXI-ARM (QEMU ARM VM 集群)
│   ├── gnucap/                 #   188-Gnucap (电路仿真)
│   ├── jtufem/                 #   jtufem-rs (FEM 结构力学)
│   └── fpga/                   #   iCESugar-pro (FPGA 硬件)
├── config/                     # BC3: HAL 配置模板
│   ├── hal_sim.toml            #   SimBackend (模式 A)
│   ├── hal_modbus.toml         #   Modbus (模式 B)
│   └── hal_fpga.toml           #   FPGA (模式 C)
├── examples/
│   ├── verification/           # 验证引擎正向/反向用例 (8 个 .plc)
│   ├── industrial/             # 工业场景示例 (3 个 .plc)
│   └── integration/            # [计划中] 多系统集成示例
├── docs/
│   ├── rustplc_intro.tex       # 技术文档 v2 (XeLaTeX 中文)
│   └── rustplc_v3.tex          # 技术文档 v3 (含 HAL/运行时/数字孪生)
└── generated/                  # 代码生成输出目录
```

## 核心契约

整个平台的解耦点是两个契约：IR 数据结构和 HalBackend trait。

### 契约一：IR（BC1 → BC2 的共享内核）

编译器输出、代码生成器输入、验证引擎输入——都是同一份 IR：

```
IR = {
  TopologyGraph    petgraph::DiGraph    设备连接关系
  StateMachine     states + transitions  控制流模型
  ConstraintSet    safety/timing/causal  验证目标
  TimingModel      action → 时间区间     时序分析
}
```

### 契约二：HalBackend trait（BC2 → BC3 的依赖倒置）

```rust
pub trait HalBackend {
    fn read_digital_input(&self, device: &str) -> bool;
    fn write_digital_output(&mut self, device: &str, value: bool);
    fn refresh_inputs(&mut self) -> Result<(), HalError>;
    fn flush_outputs(&mut self) -> Result<(), HalError>;
}
```

三种模式的差异全部封装在这个 trait 的实现里：

| 实现 | 模式 | refresh_inputs() 做什么 | flush_outputs() 做什么 |
|------|------|------------------------|----------------------|
| SimBackend | A | 从 HashMap 读 | 写入 HashMap |
| ModbusBackend | B | Modbus TCP read_discrete_inputs | Modbus TCP write_coils |
| FpgaBackend | C | UART Modbus RTU read | UART Modbus RTU write |

### 扫描周期引擎（BC2 核心）

```
每个周期 (默认 50ms):
  1. hal.refresh_inputs()   ← 读取所有输入 (来自仿真/FPGA/内存)
  2. scan_cycle(&state, &hal, &timers)  ← 执行生成的状态机
  3. hal.flush_outputs()    ← 写出所有输出 (到仿真/FPGA/内存)
  4. timers.tick(cycle_ms)  ← 推进定时器组
```

`ScanCycleEngine<S, H: HalBackend>` 是泛型的——S 是生成的 `PlcState` enum，H 是任意 HAL 后端。编译器生成的 `scan_cycle` 函数是一个纯粹的 `match` 状态机，没有 I/O 副作用，所有 I/O 通过 HAL trait 完成。

## 四大验证引擎

| 引擎 | 检查内容 | 方法 | 输出 |
|------|---------|------|------|
| Safety | 状态互斥冲突 | BMC + k-归纳 | 完备证明 or 违反路径 |
| Liveness | 死锁 / 活锁 | SCC 分析 + 可达性 | 死锁点 + 修复建议 |
| Timing | 时序包络 | 最坏关键路径计算 | 路径时间 vs 约束上界 |
| Causality | 因果链完整性 | 拓扑图 BFS | 断裂位置 + 接线建议 |

四个引擎并行运行，一次编译暴露所有问题。验证在 BC1 完成后，BC2 的运行时不再做任何安全检查。

## DSL 语言一览

一个 `.plc` 文件由三个段组成：

```plc
[topology]          # 声明物理设备与连接关系
[constraints]       # 声明安全、时序、因果约束
[tasks]             # 声明控制逻辑（状态机）
```

### 拓扑 — 描述物理世界

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

`connected_to` 建立因果链：Y0 → valve_A → cyl_A → sensor_A_ext。编译器据此构建拓扑图，验证因果完整性，计算信号传播延迟。

### 约束 — 声明安全红线

```plc
[constraints]
safety: cyl_A.extended conflicts_with cyl_B.extended
    reason: "A缸和B缸不能同时伸出"
timing: task.init must_complete_within 5000ms
causality: Y0 -> valve_A -> cyl_A -> sensor_A_ext
```

### 任务 — 控制逻辑即状态机

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

支持 `parallel`（并行分支）、`race`（竞争分支）、`allow_indefinite_wait`（人工等待豁免）等控制结构。

## 仿真闭环详解（模式 B）

模式 B 是最复杂也最有价值的模式——纯软件构建完整的虚拟工厂。

```
196-ESXI-ARM 虚拟机集群
┌──────────────────────────────────────────────────────┐
│  VM1: PLC 主控 (Cortex-A53 + PREEMPT_RT)             │
│    RustPLC runtime                                    │
│    ScanCycleEngine<PlcState, ModbusBackend>           │
│      │                                                │
│      │ Modbus TCP write_coils / read_discrete_inputs  │
│      ▼                                                │
│  VM2: 虚拟 I/O 从站 (Modbus slave daemon)             │
│    ┌─────────────────────────────────────────┐        │
│    │  coils[] ←── 主控写入                    │        │
│    │    │                                     │        │
│    │    ├──→ 188-Gnucap 电路仿真              │        │
│    │    │      线圈吸合: 0→1 延迟 15ms        │        │
│    │    │      传感器 RC 滤波: 上升沿 5ms      │        │
│    │    │      ↓                              │        │
│    │    │      DI 延迟注入                     │        │
│    │    │                                     │        │
│    │    └──→ jtufem-rs FEM 仿真               │        │
│    │           气缸推力 500N → 活塞杆应力      │        │
│    │           行程 300mm → 变形量 0.02mm      │        │
│    │           10^6 次循环 → 疲劳寿命          │        │
│    │           ↓                              │        │
│    │           物理参数修正                     │        │
│    │                                          │        │
│    │  discrete_inputs[] ──→ 主控读取           │        │
│    └─────────────────────────────────────────┘        │
│                                                       │
│  VM3: SCADA/HMI (OPC UA + Web Dashboard)              │
└──────────────────────────────────────────────────────┘
```

闭环数据流：主控写 coil → 电路仿真计算延迟 → 力学仿真计算物理响应 → 更新 DI → 主控读 DI → 状态机跳转。整个过程在虚拟机内完成，不需要任何实体硬件。

## 快速开始

```bash
# 克隆
git clone https://github.com/xxrust/RustPLC.git
cd RustPLC

# 编译
cargo build --release

# 验证一个 .plc 文件
cargo run --release -p rust_plc -- examples/industrial/conveyor_stamp.plc
```

输出：

```
验证通过：
  - Safety: 完备证明（深度 8）
  - Liveness: 通过
  - Timing: 通过
  - Causality: 通过
```

### 代码生成

```bash
# 验证通过后生成 Rust 可执行项目
cargo run --release -p rust_plc -- examples/industrial/two_cylinder.plc --generate generated/

# 编译并运行生成的代码
cd generated && cargo run
```

### 可选：启用 Z3 求解器

```bash
cargo build --release --features z3-solver
```

## 示例

### 验证示例 (examples/verification/)

| 文件 | 场景 | 验证结果 |
|------|------|---------|
| `ex1_safety_pass.plc` | 双缸顺序执行 | 全部通过 |
| `ex2_safety_fail.plc` | 双缸并行执行 | Safety 失败 |
| `ex3_liveness_fail.plc` | 等待无超时 | Liveness 失败 |
| `ex4_timing_fail.plc` | 时序约束过紧 | Timing 失败 |
| `ex5_causality_fail.plc` | 因果链断裂 | Causality 失败 |
| `ex6_all_pass.plc` | 单缸完整场景 | 全部通过 |

### 工业场景示例 (examples/industrial/)

| 文件 | 场景 | 验证结果 |
|------|------|---------|
| `conveyor_stamp.plc` | 传送带冲压系统 | 全部通过 |
| `two_cylinder.plc` | 双缸顺序动作 + 安全互斥 | 全部通过 |
| `half_rotation.plc` | 电机半圈旋转 + race 竞争分支 | 全部通过 |

## 测试

```bash
# 运行全部 59 个测试（48 单元 + 5 集成 + 6 端到端验证）
cargo test --workspace
```

## 路线图

### Phase 1: 验证域 (BC1) — 已完成

- [x] DSL 设计与 pest PEG 解析器 (150 条规则)
- [x] AST → IR 语义分析 (拓扑图/状态机/约束集/时序模型)
- [x] Safety 引擎: BMC + k-归纳 (可选 Z3)
- [x] Liveness 引擎: SCC + 死锁检测
- [x] Timing 引擎: 最坏关键路径
- [x] Causality 引擎: BFS 可达性
- [x] 结构化错误报告 (行号 + 修复建议)

### Phase 2: 执行域 (BC2) — 已完成

- [x] 代码生成: IR → PlcState enum + scan_cycle 函数
- [x] ScanCycleEngine 泛型扫描周期引擎
- [x] TimerBank 32 槽定时器组

### Phase 3: 硬件适配域 (BC3) — 进行中

- [x] HalBackend trait 定义
- [x] SimBackend 实现 (模式 A)
- [x] DeviceMapping TOML 配置
- [ ] ModbusBackend: tokio-modbus RTU/TCP (模式 B/C)
- [ ] FpgaBackend: iCESugar-pro SPI/UART (模式 C)
- [ ] Orchestrator: 配置驱动的模式选择

### Phase 4: 仿真域 (BC4) — 计划中

- [ ] Modbus slave daemon (虚拟 I/O 从站)
- [ ] 188-Gnucap 桥接: 线圈延迟 → DI 注入
- [ ] jtufem-rs 桥接: 力学参数 → 物理响应修正
- [ ] 196-ESXI-ARM VM 编排脚本

### Phase 5: 硬件域 (BC5) — 计划中

- [ ] iCESugar-pro Verilog: Modbus RTU slave 状态机
- [ ] PMOD GPIO 驱动 (继电器/LED/传感器)
- [ ] 虚实混合模式联调

### 未来

- [ ] 模拟量 I/O + PID 控制
- [ ] 多控制器协同 (多 VM master)
- [ ] 图形化 DSL 编辑器

## 技术栈

| 技术 | 用途 | 所属 BC |
|------|------|--------|
| Rust 2024 Edition | 内存安全，零成本抽象 | 全局 |
| pest | PEG 解析器生成器 | BC1 |
| petgraph | 图数据结构 (拓扑图 + SCC) | BC1 |
| serde / toml | 配置序列化 | BC3 |
| Z3 (可选) | SMT 求解器 | BC1 |
| tokio-modbus (计划) | Modbus RTU/TCP | BC3 |
| QEMU aarch64 (外部) | ARM VM 集群 | BC4 |
| Gnucap (外部) | SPICE 电路仿真 | BC4 |
| jtufem-rs (外部) | FEM 结构力学 | BC4 |
| Yosys + nextpnr (外部) | FPGA 综合 | BC5 |

## 成本

全部技术栈 100% 开源 (MIT/Apache)。硬件部分（模式 C）总成本约 $120：

| 硬件 | 价格 | 用途 |
|------|------|------|
| iCESugar-pro v1.3 | ~$50 | FPGA 开发板 |
| PMOD 继电器模块 | ~$15 | 数字输出 |
| PMOD 按钮/传感器 | ~$15 | 数字输入 |
| USB-UART 适配器 | ~$10 | Modbus RTU 通信 |
| 杜邦线/面包板 | ~$10 | 接线 |

模式 A 和模式 B 不需要任何硬件。

## License

MIT

---

<p align="center">
  <sub>用 Rust 写的，所以它不会 panic。好吧，至少不会在生产线上。</sub>
</p>
