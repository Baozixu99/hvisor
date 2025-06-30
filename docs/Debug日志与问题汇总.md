#### 	1.1  适配飞腾派的加载地址

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

#### 	1.2  实现飞腾派平台的 PL011 串口驱动

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

#### 	1.3  解决hvisor启动时卡在 "Using 4 cpu(s) on this system." 及 CPU2/3 未正常初始化问题

在启动hvisor的过程中，系统卡在了输出 `Using 4 cpu(s) on this system.` 这一行，进一步调试发现程序卡死在 `cpu_start` 函数中。经过详细分析，确认问题出在 PSCI（Power State Coordination Interface）调用链中的 `psci::cpu_on` 函数，而根本原因在于未能正确获取到多核 CPU 的唯一标识符 `cpuid`。

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

#### 	1.4  earlycon未启导致Linux初始化日志缺失问题

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

#### 	1.5   GICv3 ITS 地址分配冲突导致系统卡死

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

#### 	1.6  unhandled MMIO 异常的原因与处理方法

在hvisor的运行过程中，系统频繁出现 `unhandled mmio` 错误提示。这类错误通常表明：**某个设备的 MMIO 地址空间被访问了，但 Hypervisor 并未对其进行映射或处理**。这会导致虚拟机中的设备访问失败，引发系统异常。

经过对日志和设备树的深入分析，我们确认这些错误来源于 **飞腾派平台设备树中定义的一些外设地址未在hvisor中配置对应的内存映射区域**。也就是说，当 Linux 内核尝试访问某些硬件寄存器时，hVisor 因为没有为其建立 MMIO 映射而无法正确响应，从而触发 `unhandled mmio` 异常。

解决步骤：

- 通过日志中报错的地址反推是哪个设备被访问；
- 查看该设备在设备树中的 `reg` 地址范围；
- 在 hvisor 的配置文件 `board.rs` 中，将这些地址添加到 `ROOT_ZONE_MEMORY_REGIONS` 列表中，确保其被正确映射；

​	**禁用不必要的外设**

- 对于一些当前并不使用的设备（如 USB、pmdk_dp、dc、pcie、hda、sata、vpu 等），可以直接将其设备节点的 `status` 属性设置为 `"disabled"`，以避免 Linux 内核尝试访问它们；
- 这样可以减少不必要的 MMIO 访问，降低 Hypervisor 负载，同时提升系统的稳定性；

#### 	1.7  serial-getty 服务异常导致系统假死问题

在解决多个 `unhandled mmio` 错误后，Linux 内核终于能够顺利完成初始化并进入用户空间阶段。然而，在后续的启动过程中，系统卡在了 `[ OK ] Listening on Load/Save RF state /dev/rfkill Watch.` 这一行日志，表现为“卡死”现象。经过深入分析，判断这并非系统真正的崩溃或死锁，而是由于串口 `getty` 没有正常工作所导致的“伪卡死”。

在 `multi-user.target` 模式下，系统会默认尝试启动 `serial-getty@ttyAMA1.service`，以便提供串口终端登录功能。然而，如果 `/dev/ttyAMA1` 设备未被正确识别，或者对应的串口驱动未能正常加载，该服务便会进入反复重试状态，造成 systemd 启动流程在此处停滞，用户无法看到登录提示符，从而产生系统“卡住”的错觉(**猜测是因为系统卡在了启动serial-getty@ttyAMA1.service之前的某个服务**)。

考虑到当前环境对完整用户空间服务（如蓝牙、无线网络、音频等）的需求尚不迫切，我们选择通过在内核启动参数中添加 `init=/bin/sh` 的方式，**绕过整个 systemd 初始化流程**，直接进入由 BusyBox 提供的 Shell 环境。这一方式不会加载任何服务、不会管理 socket、也不会尝试启动 `getty`，从而避免因串口控制台问题引发的阻塞，成功实现快速进入命令行界面的目标。最终，系统顺利进入了 Shell 交互环境，验证了内核及基础虚拟化功能的可用性，为后续进一步完善用户空间支持和设备驱动配置提供了稳定的调试基础。

``` rust
chosen {
		stdout-path = "serial1:115200n8";
		bootargs = "clk_ignore_unused console=ttyAMA1,115200 earlycon=pl011,0x2800d000,115200 root=/dev/mmcblk0p1 rw rootwait init=/bin/sh";
	};

```

#### 	1.8  virtio backend is too slow 问题分析

使用 `hvisor` 启动 Non-root Linux时（使用virtio block、virtio serial），日志持续打印如下警告和错误信息：

``` less
[WARN  2] (hvisor::device::virtio_trampoline:108) virtio backend is too slow, please check it!
[ERROR 2] (hvisor::device::virtio_trampoline:112) virtio backend may have some problem, please check it!
```

现象概述：

- **Root Linux** 运行正常，且 virtio 设备初始化成功；
- **Non-root Linux** 中 virtio 设备无法正常工作；
- 中断看似注入成功，但未生效；
- 当禁用 virtio 设备后，Non-root Linux 可以正常启动。

<pre>
<span style="color: blue">[DEBUG 2] (hvisor::device::virtio_trampoline:56) mmio virtio handler,mmio.address: 0x0, mmio.size: 4, mmio.is_write: false, mmio.value: 0xffff80001168dc00,base: 0xa003c00
<span style="color: blue">[DEBUG 2] (hvisor::device::virtio_trampoline:64) dev.base_address: 0x81bd9000, dev.is_enable: true
<span style="color: blue">[DEBUG 2] (hvisor::device::virtio_trampoline:81) src_cpu: 0x2, address: 0xa003c00, size: 4, value: 0xffff80001168dc00, src_zone: 1, is_write: 0, need_interrupt: 0
<span style="color: blue">[DEBUG 2] (hvisor::device::virtio_trampoline:91) old cfg flag: 0x0,cpu2
<span style="color: blue">[DEBUG 2] (hvisor::device::virtio_trampoline:210) push req to hvisor req list, req_rear: 1, req_front: 0, req_list[1]: HvisorDeviceReq { src_cpu: 0, address: 0, size: 0, value: 0, src_zone: 0, is_write: 0, need_interrupt: 0, _padding: 0 }
<span style="color: blue">[DEBUG 2] (hvisor::device::virtio_trampoline:96) need wakeup, sending ipi to wake up virtio device
<span style="color: blue">[DEBUG 2] (hvisor::arch::aarch64::ipi:63) send sgi to cpu_id: 0, mpidr=0x200, sgi_num: 7
<span style="color: blue">[DEBUG 2] (hvisor::arch::aarch64::ipi:74) write sgi sys value = 0x7020001
<span style="color: green">[INFO  3] (hvisor::arch::aarch64::trap:110) arch_handle_exit called 12901 times
<span style="color: blue">[DEBUG 2] (hvisor::arch::aarch64::ipi:75) aff3 = 0x0, aff2 = 0x0, aff1 = 0x20000, irm = 0x0, sgi_id = 0x7000000, target_list = 0x1
<span style="color: greem">[INFO  3] (hvisor::arch::aarch64::trap:111) Current exception reason: 3
<span style="color: blue">[DEBUG 2] (hvisor::event:171) send_event: cpu_id: 0, ipi_int_id: 7, event_id: 3
<span style="color: orange">[WARN  2] (hvisor::device::virtio_trampoline:109) virtio backend is too slow, please check it!
<span style="color: red">[ERROR 2] (hvisor::device::virtio_trampoline:113) virtio backend may have some problem, please check it!
</pre>


**1.Virtio 请求正常触发，backend 中断未响应**

通过日志追踪，virtio 前端设备触发了 MMIO 读请求，并设置了 `need_interrupt = true`，随后 hypervisor 内部发起了 IPI 中断：

``` css
[DEBUG 2] send sgi to cpu_id: 0, mpidr=0x200, sgi_num: 7
[DEBUG 2] (hvisor::arch::aarch64::ipi:74) write sgi sys value = 0x7020001
[DEBUG 2] aff3 = 0x0, aff2 = 0x0, aff1 = 0x20000, irm = 0x0, sgi_id = 0x7000000, target_list = 0x1
```

`0x7020001` 对应 ICC_SGI1R_EL1 的值，表示 SGI 7 发往 Aff0 = 0、Aff1 = 2（MPIDR = 0x200）目标为 Root Linux 的 CPU（CPU0），理论上这会唤醒 Root Linux 的 Virtio 后端处理线程。

![image-20250630133440682](C:\Users\Administrator\AppData\Roaming\Typora\typora-user-images\image-20250630133440682.png)

**2.SGI 中断未在 Root Linux 被处理**.

查看 `gicv3_handle_irq_el1()` 函数发现只触发了虚拟定时器（irq_id==27），但从未触发 `irq_id == SGI_IPI_ID`。说明 SGI 虽然写入，但未能成功注入或处理：

``` rust
if irq_id == SGI_IPI_ID {
    debug!("IPI received, irq_id = {}", irq_id);
    ipi_handled = check_events();
}
```

该分支始终未进入。

**3.系统寄存器设置正确，GICC 接口正常初始化**

系统中每个 CPU 都正确执行了 `gicc_init()` 和 `enable_ipi()`，其中 `DAIF` 中断屏蔽位已正确打开，且 `icc_igrpen1_el1 = 1`，`icc_pmr_el1 = 0xf0`，允许 Group 1 中断进入。Redistributor 的 IGROUPR 和 ISENABLER 也已设置。

**4.注入逻辑路径尚未打通**

在当前实现中，虽然 `write_sysreg!(icc_sgi1r_el1, val)` 成功发送 IPI，但实际 `vgic_inject_irq()` 并**未将 SGI 设置进目标CPU  GICR 的 `SGIR` 寄存器**，或未在 `vgic_pending_table` 中标记为 pending，导致 Root Linux 的 `pending_irq()` 无法枚举出 irq_id == 7，从而 `gicv3_handle_irq_el1()` 无法进入处理分支。

##### GIC Redistributor 地址冲突分析

通过对比发现，`hvisor` 在初始化 VGIC 时，为每个 CPU 分配的 Redistributor 地址如下：

``` less
[INFO  0] (hvisor::device::irqchip::gicv3::vgic:52) registering gicr cpu0 at 0x30880000
[INFO  0] (hvisor::device::irqchip::gicv3::vgic:52) registering gicr cpu1 at 0x308a0000
[INFO  0] (hvisor::device::irqchip::gicv3::vgic:52) registering gicr cpu2 at 0x308c0000
[INFO  0] (hvisor::device::irqchip::gicv3::vgic:52) registering gicr cpu3 at 0x308e0000
```

而 root Linux和non-root linux 启动日志显示，系统扫描 GICR_TYPER 并得到如下映射：：

``` less
 CPU0: found redistributor 200 region 0:0x00000000308c0000 
 CPU1: found redistributor 201 region 0:0x00000000308e0000 
 CPU2: found redistributor 0 region 0:0x0000000030880000
 CPU3: found redistributor 100 region 0:0x00000000308a0000
```

原因分析：Linux 是根据设备树中 `/cpus` 节点的 MPIDR 值（即 reg 字段）确定 affinity，并据此扫描 GICR。

```text
mpidr = (Aff3 << 32) | (Aff2 << 16) | (Aff1 << 8) | Aff0

0x0000000000000200 → Aff1 = 2, Aff0 = 0
0x0000000000000201 → Aff1 = 2, Aff0 = 1
0x0000000000000000 → Aff1 = 0, Aff0 = 0
0x0000000000000100 → Aff1 = 1, Aff0 = 0
```

| 节点名  | reg（MPIDR） | Affinity (Aff1, Aff0) | 说明                |
| ------- | ------------ | --------------------- | ------------------- |
| cpu@0   | 0x200        | (2, 0)                | Root Linux CPU0     |
| cpu@1   | 0x201        | (2, 1)                | Root Linux CPU1     |
| cpu@100 | 0x000        | (0, 0)                | Non-root Linux CPU2 |
| cpu@101 | 0x100        | (1, 0)                | Non-root Linux CPU3 |

由于 Redistributor 地址应按 **MPIDR affinity（Aff1, Aff0）**组合升序排列（而不是逻辑 CPU 编号），所以实际应为：.

```text
gicr_base = 0x30880000

GICR for Aff1=0, Aff0=0  → 0x30880000   (non-root CPU2)
GICR for Aff1=1, Aff0=0  → 0x308a0000   (non-root CPU3)
GICR for Aff1=2, Aff0=0  → 0x308c0000   (root CPU0)
GICR for Aff1=2, Aff0=1  → 0x308e0000   (root CPU1)
```

hvisor 中原始分配逻辑存在问题：

```rust
let gicr_base = arch.gicr_base + cpu * PER_GICR_SIZE;
```

该方式仅按逻辑 CPU 编号依次排列，未考虑 MPIDR 与 affinity 的映射关系，导致 Linux 找到的 Redistributor 地址与 hvisor 注册不一致，**最终可能导致中断注入失败，从而影响 virtio 正常工作。**

##### 修改为基于 MPIDR affinity 的 GICR 地址分配

为适配飞腾派平台，需重新实现GICR 分配逻辑：

``` rust
pub fn host_gicr_base(id: usize) -> usize {
    if cfg!(feature = "mpidr_phytium") {    
        /* phytium:
            GICR for Aff1=0, Aff0=0  → 0x30880000   (non-root CPU2)
            GICR for Aff1=1, Aff0=0  → 0x308a0000   (non-root CPU3)
            GICR for Aff1=2, Aff0=0  → 0x308c0000   (root CPU0)
            GICR for Aff1=2, Aff0=1  → 0x308e0000   (root CPU1) 
        */
        static CPU_MPIDS: [usize; 4] = [0x200, 0x201, 0x000, 0x100]; // 逻辑 CPU0-3
        assert!(id < CPU_MPIDS.len());
        let mpidr = CPU_MPIDS[id];
    
        let aff0 = (mpidr >> 0) & 0xff; 
        let aff1 = (mpidr >> 8) & 0xff; 
        let cpu_interface_number = aff1 | aff0; 
        GIC.get().unwrap().gicr_base + cpu_interface_number * PER_GICR_SIZE
    }else {
        assert!(id < consts::MAX_CPU_NUM);
        GIC.get().unwrap().gicr_base + id * PER_GICR_SIZE
    }
}
```

对应地，在 `vgicv3_mmio_init` 中替换为：

```rust
pub fn vgicv3_mmio_init(&mut self, arch: &HvArchZoneConfig) {
        if arch.gicd_base == 0 || arch.gicr_base == 0 {
            panic!("vgicv3_mmio_init: gicd_base or gicr_base is null");
        }

        self.mmio_region_register(arch.gicd_base, arch.gicd_size, vgicv3_dist_handler, 0);
        self.mmio_region_register(arch.gits_base, arch.gits_size, vgicv3_its_handler, 0);

        for cpu in 0..unsafe { consts::NCPU } {
            let gicr_base = if cfg!(feature = "mpidr_phytium") {
                host_gicr_base(cpu)
            } else {
                arch.gicr_base + cpu * PER_GICR_SIZE
            };
            info!("registering gicr cpu{} at {:#x?}", cpu, gicr_base);
            self.mmio_region_register(gicr_base, PER_GICR_SIZE, vgicv3_redist_handler, cpu);
            
        }
    }
```

报错原因源于 virtio 后端未及时响应，实为中断注入失败引发；深层根因是 GICR 分配逻辑未与 CPU 的 MPIDR 对齐。这样修改后即可保证 VGIC 注册的 GICR 区域与 Linux 启动过程中识别的一致，**确保 virtio 中断能够正确注入并被处理**。