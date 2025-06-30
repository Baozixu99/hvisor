# Proj410——基于飞腾平台的Type-1型Hypervisor移植优化

- 队名：GPNPU
- 队员：包子旭、沈铭、郭睆
- 指导老师：张羽
- 学校：西北工业大学

## 项目概述

随着信创产业的不断推进，国产处理器平台对虚拟化技术提出了更高的性能、安全性和自主可控性要求。在教育、国防、工业控制等关键领域，**轻量级、高隔离度、可裁剪的虚拟化解决方案**正逐步成为主流选择。

本项目依托飞腾公司推出的国产开源教育平台 —— **飞腾派开发板**，围绕其对 **ARMv8-A 架构**的良好支持，展开对虚拟化系统的适配与验证。我们选用由 **矽望社区（Syswonder Community）主导开发的开源项目 [hvisor](https://github.com/syswonder/hvisor)** 作为核心虚拟化内核。**hvisor 是用 Rust 实现的 Type-1 Hypervisor**，采用**分离内核（Separation Kernel）架构**，强调虚拟机之间的资源强隔离与最小可信计算基（TCB），已在多个国产平台上展现出良好的可移植性与运行性能。

在本项目中，我们基于 **hvisor 的 dev 分支**，以 `2025 年 5 月 5 日` 合并的 [`#149 pull request`](https://github.com/syswonder/hvisor/pull/149)（rk3588_dev 分支）为开发基线，完成了对飞腾派平台的移植、裁剪与功能验证。目前，系统已成功在飞腾派上启动多个虚拟机（VM），并完成了包括 **GICv3 中断控制、共享内存通信机制、Virtio 系列虚拟设备（如 console、block、net）** 在内的核心功能验证，为后续在国产平台构建可信虚拟化基础奠定了基础。

#### 开发计划

#####  1.初赛阶段

|  时间节点   |                           开发内容                           |
| :---------:   | :----------------------------------------------------------: |
| **5月25日** | 解读赛题要求，完成报名流程，明确项目目标，制定初步开发计划并分配任务。 |
| **5月30日** |   收到飞腾派开发板，阅读硬件手册，配置交叉编译与调试环境。   |
| **6月2日**  | 在 hvisor 中添加对 **Phytium-Pi** 平台的支持，完成代码与硬件配置解耦；在 `platform/` 目录下分类存放架构与开发板相关配置。 |
| **6月4日**  | 根据飞腾派内存布局规划加载地址，完成 hvisor、Root Linux、Non-root Linux 镜像加载地址划分；实现对 **PL011 UART** 的驱动适配，支持串口通信。 |
| **6月6日**  | 完成 hvisor 核心功能启动，进入主循环并具备 Zone 创建与管理能力。 |
| **6月15日** | 成功创建并启动 **Root Linux（Zone1）**，实现 console 显示与基本 shell 操作。 |
| **6月27日** | 成功创建并启动 **Non-root Linux（Zone2）**，验证中断注入与 Virtio 设备初始化路径。 |





| 时间节点         | 问题现象                                     | 核心原因                                                     | 解决方案                                                     | 技术要点                                                     |
| ---------------- | -------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------ | ------------------------------------------------------------ |
| **加载地址适配** | U-Boot执行`bootm 0x40400000`失败，地址被保护 | 飞腾派DRAM起始地址为`0x80000000`，原加载地址无效             | 1. 修改Linker脚本基地址为`0x80400000`2. 更新Makefile的QEMU参数和mkimage命令 | • 通过`bdinfo`确认DRAM布局• 修改链接脚本`linker.ld`• 调整mkimage的加载地址参数 |
| **串口驱动缺失** | 卡在`Starting kernel...`无后续输出           | hvisor缺少飞腾派PL011串口驱动                                | 新增`phytium_pi.rs`驱动模块并注册                            | • 实现PL011寄存器映射(`0x2800d000`)• 添加putchar/getchar接口• 集成到全局console系统 |
| **多核启动失败** | 卡在"Using 4 cpu(s)"，CPU2/3未初始化         | 飞腾派MPIDR值非标准(`0x200`,`0x201`,`0x00`,`0x100`)          | 1. 新增`mpidr_phytium`特性2. 重构`boot_cpuid_get`汇编逻辑3. 修改`mpidr_to_cpuid`映射 | • MPIDR到cpuid的硬编码映射• 解决栈空间冲突• 适配GICv3中断路由 |
| **早期日志缺失** | Linux内核早期初始化日志不可见                | 缺少earlycon支持                                             | 修改设备树：`earlycon=pl011,0x2800d000`                      | • 启用内核极早期输出• 解决PSCI/GIC调试盲区                   |
| **内存冲突卡死** | 卡在`arch_timer: cp15 timer(s) runn`         | GICv3 ITS分配地址(`0x80480000`)覆盖hvisor代码区(`0x80400000`) | 迁移hvisor加载地址到`0x90100000`                             | • 分析ITS内存分配机制• 对齐Linux内核加载惯例• 更新U-Boot加载命令 |
| **MMIO未处理**   | 频繁出现`unhandled mmio`错误                 | 外设寄存器未在hvisor中映射                                   | 1. 在`ROOT_ZONE_MEMORY_REGIONS`添加缺失区域2. 禁用未使用设备节点 | • 通过日志反推设备地址• 设备树节点状态管理• 精确的MMIO区域映射 |
| **用户空间阻塞** | 卡在`[ OK ] Listening on Load/Save RF ...`   | serial-getty服务启动失败                                     | 内核参数添加`init=/bin/sh`绕过systemd                        | • 精简初始化流程• 直接进入BusyBox shell• 规避串口终端依赖    |

### 架构图



### 快速运行



### 1.适配飞腾派的加载地址

在尝试通过 U-Boot 阶段执行 TFTP 下载镜像并启动hvisor的过程中，遇到了一个问题：使用 `bootm 0x40400000 - 0x40000000` 命令启动失败，因为地址 `0x40400000 - 0x40000000` 被保护，无法从该地址加载 hvisor：

``` shell
setenv serverip 192.168.137.1; setenv ipaddr 192.168.137.2; setenv loadaddr 0x40400000; setenv fdt_addr 0x40000000; setenv zone0_kernel_addr 0xa0400000; setenv zone0_fdt_addr 0xa0000000; tftp ${loadaddr} ${serverip}:hvisor.bin; tftp ${fdt_addr} ${serverip}:phytium-pi-board-v2.dtb; tftp ${zone0_kernel_addr} ${serverip}:Image; tftp ${zone0_fdt_addr} ${serverip}:linux1.dtb; bootm ${loadaddr} - ${fdt_addr};
```

为了解决上述问题，我们决定直接启动飞腾派定制的 Linux 内核，并进入内核后使用 `bdinfo` 命令查看到飞腾派的 DRAM 起始地址为 `0x80000000`。基于此信息，我们对hvisor的链接脚本以及相关 Makefile 进行了修改，以确保hvisor可以从正确的内存地址加载，首先修改hvisor的链接脚本 `platform/aarch64/phytium-pi/linker.ld`，将基地址设置为 `0x80400000`，以便与新的加载地址相匹配，接下来，需要更新 Makefile 中的相关参数，以反映新的加载地址。具体修改如下:

``` makefile

QEMU_ARGS += -device loader,file="$(hvisor_bin)",addr=0x80400000,force-raw=on
QEMU_ARGS += -device loader,file="$(zone0_kernel)",addr=0xa0400000,force-raw=on
QEMU_ARGS += -device loader,file="$(zone0_dtb)",addr=0xa0000000,force-raw=on

QEMU_ARGS += -drive if=none,file=$(FSIMG1),id=Xa003e000,format=raw
QEMU_ARGS += -device virtio-blk-device,drive=Xa003e000,bus=virtio-mmio-bus.31

$(hvisor_bin): elf
	@if ! command -v mkimage > /dev/null; then \
		sudo apt update && sudo apt install u-boot-tools; \
	fi && \
	$(OBJCOPY) $(hvisor_elf) --strip-all -O binary $(hvisor_bin).tmp && \
	mkimage -n hvisor_img -A arm64 -O linux -C none -T kernel -a 0x80400000 \
	-e 0x80400000 -d $(hvisor_bin).tmp $(hvisor_bin) && \
	rm -rf $(hvisor_bin).tmp

```

### 2.实现飞腾派平台的 PL011 串口驱动

在完成hvisor的加载地址适配后，我们通过 U-Boot 成功将 hVisor、Linux 内核镜像以及设备树（DTB）加载到内存，并尝试启动内核。然而，在执行 `bootm` 命令后，系统卡在了输出 `Starting kernel ...` 阶段，没有任何后续日志输出，导致调试信息无法查看，难以进一步定位问题。
经过对启动流程的分析与排查，最终确认问题是由于 **hvisor 缺乏对飞腾派平台串口驱动的支持** 所致。这导致hvisor在启动过程中无法通过串口输出调试信息，进而表现为“卡死”现象，实则程序仍在运行，只是没有控制台输出可供观察。

``` shell
		uart@2800d000 {
			compatible = "arm,pl011\0arm,primecell";
			reg = <0x00 0x2800d000 0x00 0x1000>;
			interrupts = <0x00 0x54 0x04>;
			clocks = <0x0c 0x0c>;
			clock-names = "uartclk\0apb_pclk";
			status = "okay";
			phandle = <0x22>;
		};
```

从该定义可以看出：

- 飞腾派使用的是 **ARM PL011 UART 控制器**；
- 其寄存器起始物理地址为 `0x2800d000`；
- 支持中断和标准 PL011 功能；
- 串口处于启用状态（`status = "okay"`）。

因此，为了使hvisor能够正常输出调试信息，需要为其添加对 PL011 串口控制器的支持。在hvisor源码src/device/uart/目录中新增了一个针对飞腾派平台的串口驱动模块：`phytium_pi.rs`，内容如下：

``` rust

#![allow(dead_code)]
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::register_structs;
use tock_registers::registers::{ReadOnly, ReadWrite, WriteOnly};

use crate::memory::addr::{PhysAddr, VirtAddr};
use spin::Mutex;

pub const UART_BASE_PHYS: PhysAddr = 0x2800d000;  //phytium_pi uart address

lazy_static! {
    static ref UART: Mutex<Pl011Uart> = {
        let mut uart = Pl011Uart::new(UART_BASE_PHYS);
        uart.init();
        Mutex::new(uart)
    };
}

register_structs! {
    Pl011UartRegs {
        (0x00 => dr: ReadWrite<u32>),
        (0x04 => _reserved0),
        (0x18 => fr: ReadOnly<u32>),
        (0x1c => _reserved1),
        (0x30 => cr: ReadWrite<u32>),
        (0x34 => ifls: ReadWrite<u32>),
        (0x38 => imsc: ReadWrite<u32>),
        (0x3c => ris: ReadOnly<u32>),
        (0x40 => mis: ReadOnly<u32>),
        (0x44 => icr: WriteOnly<u32>),
        (0x48 => @END),
    }
}

struct Pl011Uart {
    base_vaddr: VirtAddr,
}

impl Pl011Uart {
    const fn new(base_vaddr: VirtAddr) -> Self {
        Self { base_vaddr }
    }

    const fn regs(&self) -> &Pl011UartRegs {
        unsafe { &*(self.base_vaddr as *const _) }
    }

    fn init(&mut self) {
        self.regs().icr.set(0x3ff);
        self.regs().ifls.set(0);
        self.regs().imsc.set(1 << 4);
        self.regs().cr.set((1 << 0) | (1 << 8) | (1 << 9));
    }

    fn putchar(&mut self, c: u8) {
        while self.regs().fr.get() & (1 << 5) != 0 {}
        self.regs().dr.set(c as u32)
    }

    fn getchar(&mut self) -> Option<u8> {
        if self.regs().fr.get() & (1 << 4) == 0 {
            Some(self.regs().dr.get() as u8)
        } else {
            None
        }
    }
}

pub fn console_putchar(c: u8) {
    UART.lock().putchar(c)
}

pub fn console_getchar() -> Option<u8> {
    UART.lock().getchar()
}

```

实现了字符的发送与接收功能；加入了并发访问保护机制；在实现了上述 PL011 串口驱动后，重新构建并加载 hVisor，成功看到串口输出的日志信息，不再卡在 `Starting kernel ...` 界面。串口驱动已正确初始化；hvisor 能够正常输出调试信息；

### 3.解决hvisor启动时卡在 "Using 4 cpu(s) on this system." 及 CPU2/3 未正常初始化问题

在启动hvisor的过程中，系统卡在了输出 `Using 4 cpu(s) on this system.` 这一行，进一步调试发现程序卡死在 `cpu_start` 函数中。经过详细分析，确认问题出在 PSCI（Power State Coordination Interface）调用链中的 `psci::cpu_on` 函数，而根本原因在于未能正确获取到多核 CPU 的唯一标识符 `cpuid`。

![image-20250606221240189](C:\Users\Administrator\AppData\Roaming\Typora\typora-user-images\image-20250606221240189.png)

通过查看设备树信息，我们确认飞腾派平台的 CPU 节点确实使用了 `psci` 作为启用方式，并且其底层依赖 SMC 指令与固件通信来唤醒 CPU。然而，飞腾派的 CPU ID 编码方式不同于常见的 ARM 平台，其四颗核心对应的 MPIDR 值分别为 `0x200`, `0x201`, `0x00`, `0x100`，而不是标准的 `0x01`, `0x02`, `0x03`, `0x04`。这导致默认的 CPU ID 映射逻辑无法正确识别这些值，从而引发唤醒失败或后续执行异常。

```json
cpu@0 {
			device_type = "cpu";
			compatible = "phytium,ftc310\0arm,armv8";
			reg = <0x00 0x200>;
			enable-method = "psci";
			clocks = <0x09 0x02>;
			capacity-dmips-mhz = <0xb22>;
			phandle = <0x07>;
		};

		cpu@1 {
			device_type = "cpu";
			compatible = "phytium,ftc310\0arm,armv8";
			reg = <0x00 0x201>;
			enable-method = "psci";
			clocks = <0x09 0x02>;
			capacity-dmips-mhz = <0xb22>;
			phandle = <0x08>;
		};

		cpu@100 {
			device_type = "cpu";
			compatible = "phytium,ftc664\0arm,armv8";
			reg = <0x00 0x00>;
			enable-method = "psci";
			clocks = <0x09 0x00>;
			capacity-dmips-mhz = <0x161c>;
			phandle = <0x05>;
		};

		cpu@101 {
			device_type = "cpu";
			compatible = "phytium,ftc664\0arm,armv8";
			reg = <0x00 0x100>;
			enable-method = "psci";
			clocks = <0x09 0x01>;
			capacity-dmips-mhz = <0x161c>;
			phandle = <0x06>;
		};

	psci {
		compatible = "arm,psci-1.0";
		method = "smc";
		cpu_suspend = <0xc4000001>;
		cpu_off = <0x84000002>;
		cpu_on = <0xc4000003>;
		sys_poweroff = <0x84000008>;
		sys_reset = <0x84000009>;
	};
```

为此，我们在 `cpu_start` 函数中新增了 `mpidr_phytium` 特性标志，用于适配飞腾平台特有的 MPIDR 映射规则。根据 cpuid（0~3）手动将其转换为对应的核心地址：CPU0 对应 `0x200`，CPU1 对应 `0x201`，CPU2 对应 `0x00`，CPU3 对应 `0x100`。这样确保了每个 CPU 都能被正确唤醒并跳转至指定的入口地址执行初始化代码。
``` rust
pub fn cpu_start(cpuid: usize, start_addr: usize, opaque: usize) {
    if cfg!(feature = "mpidr_phytium") {
        let new_cpuid = match cpuid {
            1 => {
                0x201 
            }
            2 => {
                0x00
            }
            3 => {
                0x100
            }
            _ => {
                panic!("Invalid cpuid: {}", cpuid);
            }
        };
        psci::cpu_on(new_cpuid as u64, start_addr as _, opaque as _).unwrap_or_else(|err| {
            println!("psci cpu_on failed: {:?}", err);
            if let psci::error::Error::AlreadyOn = err {
            } else {
                panic!("can't wake up cpu {}", cpuid);
            }
        })
    } else {
        psci::cpu_on(cpuid as u64 | 0x80000000, start_addr as _, opaque as _).unwrap_or_else(|err| {
            println!("psci cpu_on failed: {:?}", err);
            if let psci::error::Error::AlreadyOn = err {
            } else {
                panic!("can't wake up cpu {}", cpuid);
            }
        })
    }   
}
```

然而，在实现上述逻辑后，虽然 CPU0 和 CPU1 能够顺利进入 `rust_main` 函数完成初始化流程，但 CPU2 和 CPU3 却始终没有执行任何用户级代码。通过对 `entry.rs` 文件的深入排查，发现问题出在 `boot_cpuid_get()` 函数中——该函数由一段汇编代码实现，负责从 `mpidr_el1` 寄存器中读取当前 CPU 的 ID 值。

原始实现中，该函数仅提取了 `mpidr_el1` 的低 8 位（即 `x17 & #0xff`），这种做法适用于大多数通用平台，但在飞腾派上却存在严重偏差。由于飞腾派使用的 MPIDR 是非标准的 16 位编码方式，CPU2 和 CPU3 的 MPIDR 分别为 `0x00` 和 `0x100`，而这两个值在与 `#0xff` 做 AND 操作后都变成了 `0x00`，最终都被错误地解析为 `cpuid=0`，也就是主核 CPU0 的 ID。这一错误导致 CPU2 和 CPU3 在初始化阶段共享了 CPU0 的栈空间，从而引发了严重的栈冲突和执行异常，表现为它们虽然成功被唤醒，但并未进入 `rust_main` 执行任何初始化动作。

``` rust
pub unsafe extern "C" fn boot_cpuid_get() {
    core::arch::asm!(
        "
        mrs x17, mpidr_el1   
        and x17, x17, #0xff
        ret
    ",
        options(noreturn)
    )
}
```

为了彻底解决这个问题，我们重新实现了 `boot_cpuid_get` 的汇编逻辑，新增了对 `mpidr_phytium` 特性的支持，将 MPIDR 的低 16 位全部保留，并通过条件判断明确区分四个不同的核心，分别返回正确的 cpuid 值（0、1、2、3）。这样就确保了每个 CPU 都拥有独立的栈空间，避免了资源冲突。

``` rust
#[cfg(feature = "mpidr_phytium")]
#[naked]
#[no_mangle]
pub unsafe extern "C" fn boot_cpuid_get() {
    core::arch::asm!(
        "
        mrs x17, mpidr_el1
        and x17, x17, #0xffff   // low 16位
        
        
        cmp x17, #0x200         // cpu0
        b.eq 3f
        cmp x17, #0x201         // cpu1
        b.eq 4f
        
        // 再检查其他核心
        cmp x17, #0x00          // cpu2
        b.eq 1f
        cmp x17, #0x100         // cpu3
        b.eq 2f
        
        mov x17, #-1            // unknown CPU
        b 5f
        
    1:  mov x17, #2; b 5f        // back 2
    2:  mov x17, #3; b 5f        // back 3
    3:  mov x17, #0; b 5f        // back 0
    4:  mov x17, #1              // back 1
    5:  ret
        ",
        options(noreturn)
    );
}
```



与此同时，我们还在 `mpidr_to_cpuid` 函数中加入了对 `mpidr_phytium` 的特性支持。该函数的作用是在系统运行期间根据任意给定的 MPIDR 值反向解析出对应的 CPU ID。通过匹配 `(mpidr & 0xffff)` 的值，我们能够准确地将 `0x00`、`0x100`、`0x200`、`0x201` 四个 MPIDR 值映射为 `cpuid=2`、`3`、`0`、`1`，从而保证整个系统中对 CPU ID 的统一性和一致性。

``` rust
pub fn mpidr_to_cpuid(mpidr: u64) -> u64 {
    #[cfg(feature = "mpidr_rockchip")]
    {
        (mpidr >> 8) & 0xff
    }

    #[cfg(feature = "mpidr_phytium")]
    {
        match (mpidr & 0xffff) as u32 {
            0x00 => 2,
            0x100 => 3,
            0x200 => 0,
            0x201 => 1,
            _ => panic!("Unknown MPIDR: {:#x}", mpidr),
        }
    }

    #[cfg(not(any(feature = "mpidr_rockchip", feature = "mpidr_phytium")))]
    {
        mpidr & 0xff00ffffff
    }
}

```

此外，针对 GICv3 中断控制器的支持，我们也为飞腾平台适配了相应的 MPIDR 到物理 CPU ID 的映射逻辑，以确保中断路由和目标选择的准确性。

综上所述，本次修复主要集中在两个方面：

1. **修正了 CPU 启动时的 MPIDR 到 cpuid 的映射逻辑**，确保每个 CPU 都能被正确唤醒并分配独立的栈空间；

2. **完善了系统运行期的 MPIDR 到 cpuid 的解析机制**，使得包括中断控制在内的多个模块都能准确识别当前运行的 CPU 核心。
### 4.修复无法看到linux初始化过程问题

在完成hvisor的串口驱动适配后，成功恢复了控制台输出功能，但此时仍然存在一个问题：**看不到Linux 内核启动过程中日志输出**，在早期内核初始化阶段，如 GIC、时钟、调度器等模块的调试信息未能及时打印出来。通过分析设备树配置和内核启动流程，发现问题是由于 **缺少 earlycon 支持** 所致。为此，我们在设备树中的 `chosen` 节点中添加并启用了如下参数：

``` shell

	chosen {
		stdout-path = "serial1:115200n8";
		// bootargs = "console=ttyAMA1,115200 earlyprintk  root=/dev/mmcblk0p1 rw rootwait";
		bootargs = "clk_ignore_unused console=ttyAMA1,115200 earlycon=pl011,0x2800d000,115200 root=/dev/mmcblk0p1 rootwait rw";
	};

```

#### 启用了 `earlycon=pl011,0x2800d000`

- 该参数是 **earlycon 驱动的标准格式**，明确告诉内核使用 `pl011` 类型串口，并指定其 MMIO 地址。

- 内核在非常早的阶段（甚至还没初始化 `printk`）时，就可以输出日志 —— 这对于早期内核挂死、卡在 PSCI 或 GIC 等初始化流程的情况 **非常关键**。

- 原本的 `earlyprintk` 方式依赖于编译时默认平台和串口配置。

  通过这一修改，终于看到了内核最早启动阶段开始的部分日志输出，为后续调试提供了强有力的支撑。

### 5.卡死在[    0.000000] arch_timer: cp15 timer(s) runn

这是整个 hvisor移植过程中很隐蔽、很难定位的一个 bug，耗费了整整三天时间才得以解决。最初怀疑是时钟中断注入失败，但在调试过程中发现 Hypervisor 并没有进入任何中断处理函数，也没有接收到任何中断信号。更令人困惑的是，在将定时器频率从 50MHz 提升到 100MHz 后，内核竟然能够完整打印出以下日志：`[    0.000000] arch_timer: cp15 timer(s) running at 50.00MHz (virt).`但依然卡死这这里，随后继续尝试将时钟频率拉高至 1200MHz，发现还能多输出两条原本被卡住的日志，这进一步表明问题并不在于时钟本身，而是其他更底层机制导致了系统卡死。接着我又猜测是因为gicv3初始化失败，hypervisor的mmu没有把time相关寄存器映射成device类型等等，发现都不是导致卡死的原因，在反复查看日志输出的过程中，突然注意到以下内容：：

``` tex
[    0.000000] GICv3: 256 SPIs implemented
[    0.000000] GICv3: 0 Extended SPIs implemented
[    0.000000] GICv3: Distributor has no Range Selector support
[    0.000000] GICv3: 16 PPIs implemented
[    0.000000] GICv3: CPU0: found redistributor 200 region 0:0x00000000308c0000
[    0.000000] ITS [mem 0x30820000-0x3083ffff]
[    0.000000] ITS@0x0000000030820000: allocated 65536 Devices @80480000 (flat, esz 8, psz 64K, shr 0)
[    0.000000] ITS: using cache flushing for cmd queue
[    0.000000] GICv3: using LPI property table @0x0000000080430000
[    0.000000] GIC: using cache flushing for LPI property table
[    0.000000] GICv3: CPU0: using allocated LPI pending table @0x0000000080440000
[    0.000000] arch_timer: cp15 timer(s) runn
```

发现GICv3给 **ITS 子系统自动分配的地址是**地址分别为@80480000、@0x0000000080430000和@0x0000000080440000！！然而我的hvisor.bin的加载地址确是0x80400000（大小约 700KB ≈ 0x000B0000），这些区域**覆盖了 hvisor.bin 占用的内存空间**，导致 Linux 在初始化 GICv3 ITS 时：引发 **Hypervisor 崩溃** 或 **VGIC 初始化失败**；导致中断不触发，`pending_irq()` 永远返回 `None`；`arch_timer` 输出“runni”后卡死，只是中断控制器死锁的**表面现象**。

根据飞腾派提供的 U-Boot 启动参数，Linux 内核镜像通常加载在 `0x90100000`，设备树则加载在 `0x90000000`。因此，为了避免与 Linux 及其子系统的内存分配发生冲突，我们将hvisor的加载地址也调整为 `0x90100000`，确保其不会侵占后续 Linux 系统使用的内存空间。

```rust
//platform.mk修改链接脚本和构建脚本
QEMU_ARGS += -device loader,file="$(hvisor_bin)",addr=0x90100000,force-raw=on
$(hvisor_bin): elf
	@if ! command -v mkimage > /dev/null; then \
		sudo apt update && sudo apt install u-boot-tools; \
	fi && \
	$(OBJCOPY) $(hvisor_elf) --strip-all -O binary $(hvisor_bin).tmp && \
	mkimage -n hvisor_img -A arm64 -O linux -C none -T kernel -a 0x90100000 \
	-e 0x90100000 -d $(hvisor_bin).tmp $(hvisor_bin) && \
	rm -rf $(hvisor_bin).tmp
//更新 U-Boot 启动命令
setenv serverip 192.168.137.1; setenv ipaddr 192.168.137.2; setenv loadaddr 0x90100000; setenv fdt_addr 0x90000000; setenv zone0_kernel_addr 0xa0400000; setenv zone0_fdt_addr 0xa0000000; 
tftp ${loadaddr} ${serverip}:hvisor.bin; tftp ${fdt_addr} ${serverip}:phytium-pi-board-v2.dtb; tftp ${zone0_kernel_addr} ${serverip}:Image; tftp ${zone0_fdt_addr} ${serverip}:linux1.dtb; bootm ${loadaddr} - ${fdt_addr};
```

此次问题的根本原因是 **hvisor 的加载地址与 Linux 内核在初始化 GICv3 ITS 时自动分配的地址发生冲突**，导致 Hypervisor 被覆盖、中断失效、系统卡死。

### 6.解决多个unhandled mmio的错误

在hvisor的运行过程中，系统频繁出现 `unhandled mmio` 错误提示。这类错误通常表明：**某个设备的 MMIO 地址空间被访问了，但 Hypervisor 并未对其进行映射或处理**。这会导致虚拟机中的设备访问失败，引发系统异常。

经过对日志和设备树的深入分析，我们确认这些错误来源于 **飞腾派平台设备树中定义的一些外设地址未在hvisor中配置对应的内存映射区域**。也就是说，当 Linux 内核尝试访问某些硬件寄存器时，hVisor 因为没有为其建立 MMIO 映射而无法正确响应，从而触发 `unhandled mmio` 异常。

解决步骤：

- 通过日志中报错的地址反推是哪个设备被访问；
- 查看该设备在设备树中的 `reg` 地址范围；
- 在 hvisor 的配置文件 `board.rs` 中，将这些地址添加到 `ROOT_ZONE_MEMORY_REGIONS` 列表中，确保其被正确映射；

​	**禁用不必要的外设**

- 对于一些当前并不使用的设备（如 USB、pmdk_dp、dc、pcie、hda、sata、vpu 等），可以直接将其设备节点的 `status` 属性设置为 `"disabled"`，以避免 Linux 内核尝试访问它们；
- 这样可以减少不必要的 MMIO 访问，降低 Hypervisor 负载，同时提升系统的稳定性；

### 7.解决卡死在[ OK ] Listening on Load/Save RF …itch Status /dev/rfkill Watch.

在解决多个 `unhandled mmio` 错误后，Linux 内核终于能够顺利完成初始化并进入用户空间阶段。然而，在后续的启动过程中，系统卡在了 `[ OK ] Listening on Load/Save RF state /dev/rfkill Watch.` 这一行日志，表现为“卡死”现象。经过深入分析，判断这并非系统真正的崩溃或死锁，而是由于串口 `getty` 没有正常工作所导致的“伪卡死”。

在 `multi-user.target` 模式下，系统会默认尝试启动 `serial-getty@ttyAMA1.service`，以便提供串口终端登录功能。然而，如果 `/dev/ttyAMA1` 设备未被正确识别，或者对应的串口驱动未能正常加载，该服务便会进入反复重试状态，造成 systemd 启动流程在此处停滞，用户无法看到登录提示符，从而产生系统“卡住”的错觉(**猜测是因为系统卡在了启动serial-getty@ttyAMA1.service之前的某个服务**)。

考虑到当前环境对完整用户空间服务（如蓝牙、无线网络、音频等）的需求尚不迫切，我们选择通过在内核启动参数中添加 `init=/bin/sh` 的方式，**绕过整个 systemd 初始化流程**，直接进入由 BusyBox 提供的 Shell 环境。这一方式不会加载任何服务、不会管理 socket、也不会尝试启动 `getty`，从而避免因串口控制台问题引发的阻塞，成功实现快速进入命令行界面的目标。最终，系统顺利进入了 Shell 交互环境，验证了内核及基础虚拟化功能的可用性，为后续进一步完善用户空间支持和设备驱动配置提供了稳定的调试基础。

``` rust
chosen {
		stdout-path = "serial1:115200n8";
		bootargs = "clk_ignore_unused console=ttyAMA1,115200 earlycon=pl011,0x2800d000,115200 root=/dev/mmcblk0p1 rw rootwait init=/bin/sh";
	};

```



