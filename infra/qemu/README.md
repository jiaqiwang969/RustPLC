# QEMU Mode B 运维说明（RustPLC）

本文档记录 `infra/qemu/` 的常用操作与已踩坑结论，重点避免 Modbus 地址偏移问题。

## 1) 常用操作

1. 启动 VM（后台）

```bash
bash infra/qemu/boot.sh ubuntu --bg
```

2. 查 VM IP（按固定 MAC）

```bash
source infra/qemu/_lib.sh
find_guest_ip "52:54:00:12:34:01"
```

3. 部署 Modbus slave 到 VM

```bash
bash infra/qemu/deploy-slave.sh
```

4. 检查 slave 进程与端口

```bash
ssh -o StrictHostKeyChecking=no ubuntu@<VM_IP> \
  "ps aux | grep modbus_slave.py | grep -v grep; ss -tlnp | grep 502; sudo systemctl is-active modbus-slave.service"
```

5. 从 host 侧检查 TCP 连通性

```bash
nc -z -w 3 <VM_IP> 502 && echo "TCP OK" || echo "TCP FAIL"
```

## 2) Modbus 地址基准（关键）

### 统一约定

- RustPLC `ModbusBackend` 按 **0-based** 地址读写：
  - `read_discrete_inputs(0, N)`
  - `write_multiple_coils(0, [...])`
- 因此配置文件中的 mapping（`[mapping.coils]` / `[mapping.discrete_inputs]`）也是 **0-based**。

### VM slave 必须满足

- `pymodbus` 的 `ModbusSequentialDataBlock` 起始地址必须用 `0`。
- 在 slave 代码中调用 `getValues` / `setValues` 时，地址必须直接用映射值，不要再手动 `+1`。

### 已修复实现

- `infra/qemu/modbus-slave/modbus_slave.py` 现已按 0-based 实现：
  - datastore 从 `0` 起始；
  - 读写 coils/DI 不再 `+1`；
  - 初始状态设为 `home=true, end=false`（缩回态）。

## 3) 设备名映射约定（第二常见坑）

- `.plc` 文件里的设备名必须与 HAL TOML 的 `mapping` key 完全一致。
- 若名字不一致（例如 `.plc` 用 `cyl_A`，TOML 用 `valve_extend`），HAL 缓存会写不到目标地址，表现为“网络通但动作不对”。

## 4) ex1 专用说明

- `examples/verification/ex1_safety_pass.plc` 使用设备名：
  - 输出：`cyl_A`, `cyl_B`
  - 输入：`sensor_A`, `sensor_B`
- 对应配置：`config/hal_modbus_tcp_ex1.toml`
- 当前 VM slave 只模拟“一组伸/缩 + home/end”，因此 ex1 配置采用兼容映射：
  - `cyl_A -> coil0 (extend)`
  - `cyl_B -> coil1 (retract)`
  - `sensor_A -> DI1 (end)`
  - `sensor_B -> DI0 (home)`

## 5) 故障排查速查

1. 先看连通：
   - `nc -z -w 3 <VM_IP> 502`
2. 再看服务：
   - `ssh ubuntu@<VM_IP> "sudo systemctl is-active modbus-slave.service"`
   - `ssh ubuntu@<VM_IP> "sudo journalctl -u modbus-slave --no-pager -n 80"`
3. 若日志反复只写同一组 coils，重点排查：
   - `.plc` 设备名与 TOML mapping 是否一致；
   - slave 是否出现 1-based/0-based 偏移。

