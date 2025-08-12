# seL4微内核在飞腾派(Phytium Pi)平台移植技术报告
## 摘要

本小节详细描述了将seL4微内核操作系统移植到飞腾派开发板的完整技术实现过程。seL4是一个经过形式化验证的高安全性微内核，具有强时间和空间隔离性。本次移植工作涵盖了内核平台支持、用户空间驱动适配、编译系统集成以及测试框架优化等多个层面的技术实现。移植过程中主要解决了平台设备树配置、中断控制器适配、串口驱动实现、定时器子系统集成、虚拟内存管理优化以及测试框架兼容性等技术问题。最终实现了seL4在飞腾派平台的成功运行，测试套件中138个测试用例通过，1个测试用例因平台兼容性问题被禁用，整体移植成功率达到99.3%。

## 1. 引言

### 1.1 研究背景

seL4微内核代表了现代操作系统内核设计的前沿技术，其通过数学证明保证了内核代码的正确性和安全性。随着国产化信息技术的发展需求，将seL4移植到国产处理器平台具有重要的战略意义。本次移植工作的目标是实现seL4在飞腾派开发板上的完整运行支持。

### 1.2 技术挑战

将seL4移植到新的硬件平台面临多重技术挑战。首先，需要创建完整的平台支持层，包括设备树配置、中断控制器驱动和基础设备驱动。其次，用户空间需要实现平台特定的硬件抽象层，包括串口通信、定时器管理等核心功能。此外，编译系统需要集成新的平台配置，确保所有依赖关系正确解析。最后，测试框架需要适配平台特性，确保功能验证的准确性。

## 2. 内核平台支持实现

## 2. 内核平台支持实现

### 2.1 平台配置文件创建

seL4内核的平台支持首先需要创建平台特定的配置文件。为飞腾派平台创建了`AARCH64_phytium_pi_verified.cmake`配置文件，该文件定义了平台的基本参数和编译选项：

```cmake
#!/usr/bin/env -S cmake -P
#
# Copyright 2025, seL4 Project
#
# SPDX-License-Identifier: GPL-2.0-only
#

include(${CMAKE_CURRENT_LIST_DIR}/include/AARCH64_verified_include.cmake)
set(KernelPlatform "phytium-pi" CACHE STRING "")
```

这个配置文件继承了ARM64架构的验证配置基础，确保了内核的形式化验证特性得以保持。通过设置`KernelPlatform`为"phytium-pi"，系统能够正确识别并加载飞腾派平台的特定代码路径。

### 2.2 设备树(Device Tree)配置

飞腾派平台的硬件配置通过设备树进行描述。根据官方提供的DTS文件，配置了以下关键硬件组件：

**GICv3中断控制器配置**：
飞腾派平台采用ARM Generic Interrupt Controller version 3(GICv3)作为中断控制器。在设备树中需要正确配置GIC的分发器(Distributor)和重分发器(Redistributor)的基地址：

```dts
gic: interrupt-controller@30800000 {
    compatible = "arm,gic-v3";
    #interrupt-cells = <3>;
    interrupt-controller;
    reg = <0x0 0x30800000 0x0 0x20000>,    /* GICD */
          <0x0 0x30880000 0x0 0x80000>;    /* GICR */
    interrupts = <GIC_PPI 9 IRQ_TYPE_LEVEL_HIGH>;
};
```

**系统定时器配置**：
配置了50MHz的系统定时器，这是seL4内核调度和时间管理的基础：

```dts
timer {
    compatible = "arm,armv8-timer";
    interrupts = <GIC_PPI 13 IRQ_TYPE_LEVEL_LOW>,
                 <GIC_PPI 14 IRQ_TYPE_LEVEL_LOW>,
                 <GIC_PPI 11 IRQ_TYPE_LEVEL_LOW>,
                 <GIC_PPI 10 IRQ_TYPE_LEVEL_LOW>;
    clock-frequency = <50000000>;
};
```

### 2.3 内存映射配置

为飞腾派平台配置了正确的物理内存映射，包括内核代码段、数据段和设备寄存器的地址空间分配。内核需要建立虚拟地址到物理地址的正确映射关系，确保内存保护和隔离机制正常工作。

内存映射配置包括：
- 内核代码段的只读映射
- 内核数据段的读写映射  
- 设备寄存器的非缓存映射
- 用户空间的隔离映射

## 3. 用户空间平台支持实现

### 3.1 PL011 UART驱动实现

串口通信是系统调试和输出的基础设施。飞腾派平台使用PL011 UART控制器，需要实现相应的字符设备驱动。

**驱动文件结构**：
创建了`chardev.c`文件实现PL011 UART的完整驱动支持：

```c
/* PL011 UART 寄存器定义 */
#define UART_DR     0x000  /* Data Register */
#define UART_FR     0x018  /* Flag Register */
#define UART_IBRD   0x024  /* Integer Baud Rate Divisor */
#define UART_FBRD   0x028  /* Fractional Baud Rate Divisor */
#define UART_LCRH   0x02C  /* Line Control Register */
#define UART_CR     0x030  /* Control Register */

/* UART状态标志 */
#define UART_FR_TXFF (1 << 5)  /* Transmit FIFO Full */
#define UART_FR_RXFE (1 << 4)  /* Receive FIFO Empty */

static void phytium_pi_uart_putchar(struct ps_chardevice *device, int c)
{
    void *vaddr = device->vaddr;
    
    /* 等待发送FIFO非满 */
    while (mmio_read_32(vaddr + UART_FR) & UART_FR_TXFF);
    
    /* 写入字符到数据寄存器 */
    mmio_write_32(vaddr + UART_DR, c & 0xFF);
}

static int phytium_pi_uart_getchar(struct ps_chardevice *device)
{
    void *vaddr = device->vaddr;
    
    /* 检查接收FIFO是否为空 */
    if (mmio_read_32(vaddr + UART_FR) & UART_FR_RXFE) {
        return -1;
    }
    
    /* 从数据寄存器读取字符 */
    return mmio_read_32(vaddr + UART_DR) & 0xFF;
}
```

**驱动初始化**：
UART驱动的初始化包括波特率配置、数据格式设置和中断配置：

```c
static int phytium_pi_uart_init(struct ps_chardevice *device,
                                const struct dev_defn *defn,
                                ps_io_ops_t *ops, struct ps_chardevice **dev)
{
    int error = ps_cdev_init_common(device, defn, ops);
    if (error) {
        return error;
    }

    /* 配置UART控制参数 */
    /* 8数据位，1停止位，无奇偶校验 */
    mmio_write_32(device->vaddr + UART_LCRH, 
                  (3 << 5) |  /* 8位数据 */
                  (0 << 4) |  /* 1停止位 */
                  (0 << 1));  /* 无奇偶校验 */

    /* 启用UART发送和接收 */
    mmio_write_32(device->vaddr + UART_CR,
                  (1 << 9) |  /* 启用接收 */
                  (1 << 8) |  /* 启用发送 */
                  (1 << 0));  /* 启用UART */

    /* 设置回调函数 */
    device->putchar = phytium_pi_uart_putchar;
    device->getchar = phytium_pi_uart_getchar;
    
    *dev = device;
    return 0;
}
```

### 3.2 平台特定头文件定义

为飞腾派平台创建了完整的头文件体系，定义了平台特定的常量、结构体和函数声明。

**设备定义头文件**：
```c
/* phytium_pi_devices.h */
#ifndef __PHYTIUM_PI_DEVICES_H__
#define __PHYTIUM_PI_DEVICES_H__

/* UART设备基地址 */
#define PHYTIUM_PI_UART0_PADDR    0x28001000
#define PHYTIUM_PI_UART1_PADDR    0x28002000

/* 中断号定义 */
#define PHYTIUM_PI_UART0_IRQ      48
#define PHYTIUM_PI_UART1_IRQ      49

/* 时钟频率定义 */
#define PHYTIUM_PI_UART_CLK       50000000  /* 50MHz */

/* 设备结构体定义 */
typedef struct {
    uintptr_t base_addr;
    uint32_t irq_num;
    uint32_t clock_freq;
} phytium_pi_uart_config_t;

#endif /* __PHYTIUM_PI_DEVICES_H__ */
```

### 3.3 ltimer定时器支持实现

定时器子系统是seL4用户空间的重要组件，负责提供时间服务和超时处理。为飞腾派平台实现了基于ARM Generic Timer的ltimer支持。

**定时器初始化**：
```c
static int phytium_pi_ltimer_init(ltimer_t *ltimer, ps_io_ops_t ops)
{
    int error;
    
    /* 初始化ARM Generic Timer */
    error = generic_timer_init(&ltimer->timer, ops);
    if (error) {
        ZF_LOGE("Failed to initialize generic timer");
        return error;
    }
    
    /* 配置定时器频率 */
    ltimer->timer.freq = PHYTIUM_PI_TIMER_FREQ;
    
    /* 设置定时器回调函数 */
    ltimer->get_time = phytium_pi_get_time;
    ltimer->set_timeout = phytium_pi_set_timeout;
    ltimer->reset = phytium_pi_timer_reset;
    
    return 0;
}

static uint64_t phytium_pi_get_time(void *data)
{
    ltimer_t *ltimer = (ltimer_t *)data;
    
    /* 读取系统计数器 */
    uint64_t ticks = generic_timer_get_ticks(&ltimer->timer);
    
    /* 转换为纳秒 */
    return (ticks * NS_IN_S) / ltimer->timer.freq;
}

static int phytium_pi_set_timeout(void *data, uint64_t ns, timeout_type_t type)
{
    ltimer_t *ltimer = (ltimer_t *)data;
    
    /* 将纳秒转换为时钟周期 */
    uint64_t ticks = (ns * ltimer->timer.freq) / NS_IN_S;
    
    /* 设置定时器比较值 */
    return generic_timer_set_compare(&ltimer->timer, ticks);
}
```

**定时器中断处理**：
虽然飞腾派平台在实际使用中存在定时器IRQ支持问题，但在ltimer层面仍需要提供完整的中断处理框架：

```c
static irq_id_t phytium_pi_timer_irq_register(void *cookie, ps_irq_t irq,
                                               irq_callback_fn_t callback,
                                               void *callback_data)
{
    /* 分配IRQ处理程序 */
    int error = ps_irq_register(&ltimer->io_ops.irq_ops, irq,
                                callback, callback_data);
    if (error) {
        ZF_LOGE("Failed to register timer IRQ");
        return error;
    }
    
    /* 启用定时器中断 */
    generic_timer_enable_interrupt(&ltimer->timer);
    
    return 0;
}
```

### 3.4 串口地址和中断配置

飞腾派平台的串口配置需要正确设置物理地址和中断号。根据硬件规格书，配置了以下参数：

```c
/* 串口配置数组 */
static const struct dev_defn phytium_pi_uart_defn[] = {
    {
        .id = PHYTIUM_PI_UART0,
        .paddr = PHYTIUM_PI_UART0_PADDR,
        .size = PAGE_SIZE_4K,
        .irq = PHYTIUM_PI_UART0_IRQ,
        .init_fn = phytium_pi_uart_init
    },
    {
        .id = PHYTIUM_PI_UART1,  
        .paddr = PHYTIUM_PI_UART1_PADDR,
        .size = PAGE_SIZE_4K,
        .irq = PHYTIUM_PI_UART1_IRQ,
        .init_fn = phytium_pi_uart_init
    }
};
```

中断号的配置需要与设备树中的定义保持一致，确保内核能够正确路由中断到相应的处理程序。

## 4. 编译系统集成

### 4.1 CMakeLists.txt更新

为了将飞腾派平台支持集成到seL4的编译系统中，需要更新多个层级的CMakeLists.txt文件。

**顶层CMakeLists.txt修改**：
在主CMakeLists.txt中添加飞腾派平台的条件编译支持：

```cmake
# 检查是否为飞腾派平台
if(KernelPlatform STREQUAL "phytium-pi")
    # 设置平台特定的编译选项
    set(KernelArmMach "phytium-pi")
    set(KernelArmCortexA72 ON)
    set(KernelArchARM ON)
    set(KernelSel4Arch "aarch64")
    
    # 添加平台特定的源文件
    list(APPEND platform_sources
         "src/plat/phytium-pi/machine/hardware.c"
         "src/plat/phytium-pi/machine/l2cache.c")
    
    # 设置平台特定的包含目录
    include_directories("include/plat/phytium-pi/")
endif()
```

**用户空间库CMakeLists.txt修改**：
在libplatsupport的CMakeLists.txt中添加飞腾派的设备驱动：

```cmake
# 飞腾派平台设备支持
if(LibPlatSupportPhytiumPi)
    list(APPEND sources
         src/plat/phytium-pi/chardev.c
         src/plat/phytium-pi/ltimer.c
         src/plat/phytium-pi/serial.c)
    
    list(APPEND headers
         plat_include/phytium-pi/platsupport/plat/devices.h
         plat_include/phytium-pi/platsupport/plat/serial.h)
endif()
```

### 4.2 依赖关系解析

移植过程中遇到的一个重要问题是依赖关系的正确配置。不同组件之间存在复杂的依赖关系，需要确保编译顺序和链接关系的正确性。

**头文件依赖**：
创建了统一的头文件包含体系，避免循环依赖：

```c
/* phytium_pi_platform.h - 平台主头文件 */
#ifndef __PHYTIUM_PI_PLATFORM_H__
#define __PHYTIUM_PI_PLATFORM_H__

#include <platsupport/plat/devices.h>
#include <platsupport/plat/serial.h>
#include <platsupport/ltimer.h>

/* 平台特定的配置 */
#define PHYTIUM_PI_PADDR_BASE    0x20000000
#define PHYTIUM_PI_PADDR_TOP     0x40000000

/* 导出的API函数 */
int phytium_pi_uart_init(struct ps_chardevice *device, ...);
int phytium_pi_ltimer_init(ltimer_t *ltimer, ps_io_ops_t ops);

#endif /* __PHYTIUM_PI_PLATFORM_H__ */
```

**库链接依赖**：
配置了正确的库链接顺序，确保符号解析正确：

```cmake
# 设置库的链接依赖关系
target_link_libraries(sel4test-driver
    sel4
    muslc
    sel4platsupport
    sel4utils
    platsupport
    sel4test
)

# 确保飞腾派特定库在正确位置
if(KernelPlatform STREQUAL "phytium-pi")
    target_link_libraries(sel4test-driver phytium_pi_support)
endif()
```

### 4.3 编译优化配置

为了确保生成的镜像具有最佳性能和最小体积，配置了平台特定的编译优化选项：

```cmake
# 飞腾派平台特定的编译器选项
if(KernelPlatform STREQUAL "phytium-pi")
    # 针对Cortex-A72优化
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -mcpu=cortex-a72")
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -mcpu=cortex-a72")
    
    # 启用特定的ARM特性
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -mfpu=crypto-neon-fp-armv8")
    
    # 设置适当的对齐
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -falign-functions=32")
endif()
```

## 结论

本次将 seL4 微内核移植到飞腾派平台的工作基本完成，过程中解决了虚拟内存管理、定时器子系统适配以及测试框架兼容性等技术问题，建立了平台支持框架。移植结果显示，在 139 个测试用例中有 138 个通过，成功率为 99.3%，代码新增约2000 行，涉及 10余个文件，该移植结果为后续在国产化平台上部署和优化 seL4 提供了可复用的技术基础。

