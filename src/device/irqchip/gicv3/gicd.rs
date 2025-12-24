// Copyright (c) 2020-2022 Andre Richter <andre.o.richter@gmail.com>

//! GICD Driver - GIC Distributor.
//!
//! # Glossary
//!   - SPI - Shared Peripheral Interrupt.
#![allow(dead_code)]

use core::ptr::write_volatile;
use core::ptr::read_volatile;
use spin::Mutex;

use super::host_gicd_base;

pub static GICD_LOCK: Mutex<()> = Mutex::new(());

pub const GICD_CTLR: usize = 0x0000;
pub const GICD_CTLR_ARE_NS: usize = 1 << 5;
pub const GICD_CTLR_GRP1NS_ENA: usize = 1 << 1;

pub const GICD_TYPER: usize = 0x0004;
pub const GICD_IIDR: usize = 0x0008;
pub const GICD_TYPER2: usize = 0x000c;
pub const GICD_IGROUPR: usize = 0x0080;
pub const GICD_ISENABLER: usize = 0x0100;
pub const GICD_ICENABLER: usize = 0x0180;
pub const GICD_ISPENDR: usize = 0x0200;
pub const GICD_ICPENDR: usize = 0x0280;
pub const GICD_ISACTIVER: usize = 0x0300;
pub const GICD_ICACTIVER: usize = 0x0380;
pub const GICD_IPRIORITYR: usize = 0x0400;
pub const GICD_ITARGETSR: usize = 0x0800;
pub const GICD_ICFGR: usize = 0x0c00;
pub const GICD_NSACR: usize = 0x0e00;
pub const GICD_SGIR: usize = 0x0f00;
pub const GICD_CPENDSGIR: usize = 0x0f10;
pub const GICD_SPENDSGIR: usize = 0x0f20;
pub const GICD_IROUTER: usize = 0x6000;

pub const GICDV3_CIDR0: usize = 0xfff0;
pub const GICDV3_PIDR0: usize = 0xffe0;
pub const GICDV3_PIDR2: usize = 0xffe8;
pub const GICDV3_PIDR4: usize = 0xffd0;

pub fn enable_gic_are_ns() {
    unsafe {
        ((host_gicd_base() + GICD_CTLR) as *mut u32)
            .write_volatile(GICD_CTLR_ARE_NS as u32 | GICD_CTLR_GRP1NS_ENA as u32);
    }
}

pub fn set_ispender(index: usize, value: u32) {
    unsafe {
        write_volatile(
            (host_gicd_base() + GICD_ISPENDR + index * 4) as *mut u32,
            value,
        );
    }
}
// / 设置指定中断的路由配置 (GICD_IROUTER)
// / 该函数操作 64 位的 `GICD_IROUTER` 寄存器，用于控制中断被路由到哪个或哪些 CPU 接口。
// / # 参数
// /
// / - `irq`: 中断号。
// / - `value`: 64 位的路由值。
// /     - 对于直接路由 (Mode=0)：包含目标 CPU 的 MPIDR Affinity 值。
// /     - 对于 1-of-N 路由 (Mode=1)：Bit 31 置 1，表示发送给任意一个候选 CPU。
pub fn set_irouter(irq: usize, value: u64) {
    unsafe {
        // GICD_IROUTER 是 64 位寄存器，每个中断占用 8 字节
        // 基地址偏移 0x6000
        let addr = (host_gicd_base() + GICD_IROUTER + irq * 8) as *mut u64;
        write_volatile(addr, value);
    }
}
// / 设置中断安全分组 (GICD_IGROUPR)
// / 该函数操作 `GICD_IGROUPR` 寄存器，决定中断属于 Group 0 (Secure) 还是 Group 1 (Non-Secure)。
// / 由于该寄存器每位对应一个中断，函数会执行“读-改-写” (Read-Modify-Write) 操作以保护其他位的状态。
// /
// / # 参数
// / - `irq`: 中断号。
// / - `group`: 分组标识。
// /     - `0`: Secure (Group 0)。
// /     - `1`: Non-Secure (Group 1)。
pub fn set_igroup(irq: usize, group: u32) {
    unsafe {
        let reg_idx = irq / 32;
        let bit_pos = irq % 32;
        
        // 计算寄存器地址 (偏移 0x0080)
        // GICD_IGROUPR 是一个 32 位（4 字节）宽的寄存器数组，每个寄存器管理 32 个中断（bit0–bit31）
        let addr = (host_gicd_base() + GICD_IGROUPR + reg_idx * 4) as *const u32;
        let mut val = read_volatile(addr);

        if group != 0 {
            val |= 1 << bit_pos;      // 置 1: 归类为 Non-Secure
        } else {
            val &= !(1 << bit_pos);   // 置 0: 归类为 Secure
        }

        // 转换为 *mut 写回修改后的值
        let write_addr = addr as *mut u32;
        write_volatile(write_addr, val);
    }
}

// / 启用指定中断 (GICD_ISENABLER)
// /
// / 该函数操作 `GICD_ISENABLER` (Interrupt Set-Enable) 寄存器。
// / 向对应位写入 `1` 将启用该中断的转发；写入 `0` 无效（不会禁用中断）。
// /
// / # 参数
// /
// / - `irq`: 中断号。
pub fn set_isenabler(irq: usize) {
    unsafe {
        // GICD_ISENABLER 是 32 位寄存器，写 1 使能
        // 基地址偏移 0x0100
        let addr = (host_gicd_base() + GICD_ISENABLER + (irq / 32) * 4) as *mut u32;
        write_volatile(addr, 1 << (irq % 32));
    }
}

// / 设置指定中断的优先级 (GICD_IPRIORITYR)
// /
// / 该函数操作 `GICD_IPRIORITYR` 寄存器。该寄存器支持字节访问，
// / 因此可以直接写入 8 位优先级值，无需“读-改-写”。
// /
// / # 参数
// /
// / - `irq`: 中断号。
// / - `priority`: 8 位优先级值。
// /     - `0x00`: 最高优先级。
// /     - `0xFF`: 最低优先级。
pub fn set_ipriority(irq: usize, priority: u8) {
    unsafe {
        // GICD_IPRIORITYR 每个中断占 1 字节，基地址偏移 0x0400
        let addr = (host_gicd_base() + GICD_IPRIORITYR + irq) as *mut u8;
        write_volatile(addr, priority);
    }
}

