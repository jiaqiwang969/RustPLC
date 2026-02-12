# iCESugar-pro — FPGA 硬件 I/O

## 角色

在数字孪生平台中充当 **Layer 8: 实体硬件 I/O**。

提供确定性实时数字 I/O，用于硬件在环（Hardware-in-Loop）验证：
- FPGA 内部实现 Modbus RTU slave 状态机
- PMOD GPIO 驱动继电器、LED、按钮、传感器
- 硬件级时序精度（纳秒级），远超软件定时器

## 硬件规格

| 参数 | 值 |
|------|-----|
| FPGA 芯片 | Lattice ECP5-25K |
| LUT 数量 | 24,576 |
| 接口 | USB-C (JTAG/SPI) |
| PMOD 插槽 | 4 × 12-pin |
| 时钟 | 25 MHz 晶振 |

## 架构位置

```
RustPLC runtime
  → HAL FpgaBackend
    → USB-SPI 桥接
      → FPGA Modbus RTU slave 状态机
        → PMOD GPIO
          → 继电器 / LED / 传感器
```

## 接口

- USB-SPI：主机与 FPGA 通信
- Modbus RTU：FPGA 内部协议栈
- PMOD GPIO：物理 I/O 信号
