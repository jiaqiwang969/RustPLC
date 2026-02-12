# 196-ESXI-ARM — ARM 虚拟机集群

## 角色

在数字孪生平台中充当 **Layer 5: 虚拟主控**。

通过 QEMU aarch64 模拟多台 ARM Cortex-A53 工业控制器，每台 VM 运行一个 RustPLC runtime 实例，VM 之间通过 Modbus TCP 通信，构成完整的虚拟工厂。

## 架构位置

```
VM1: PLC 主控（Cortex-A53 + PREEMPT_RT）
  └── RustPLC runtime（Modbus master）
VM2: 远程 I/O 从站（Modbus slave）
  └── Gnucap 电路仿真 + jtufem 物理仿真
VM3: SCADA/HMI
  └── OPC UA + Web Dashboard
```

## 本地路径

```
/Users/jqwang/196-ESXI-ARM
```

## 接口

- Modbus TCP（VM 间通信）
- SSH（管理通道）
- 共享文件系统（.plc 源文件分发）
