# 外部项目引用

RustPLC 数字孪生平台依赖以下外部项目。它们各自独立维护，此目录仅包含说明文件，指向实际项目位置。

| 目录 | 项目 | 角色 |
|------|------|------|
| `esxi-arm/` | 196-ESXI-ARM | ARM VM 集群（QEMU aarch64，多主控仿真） |
| `gnucap/` | 188-Gnucap | 电路仿真（电磁阀线圈、传感器信号调理） |
| `jtufem/` | jtufem-rs | FEM 结构力学求解器（应力、模态、疲劳） |
| `fpga/` | iCESugar-pro | FPGA 硬件（Lattice ECP5 25K LUT，确定性 I/O） |
