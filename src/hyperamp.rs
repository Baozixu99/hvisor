// Copyright (c) 2023 Beihang University, Huawei Technologies Co.,Ltd. All rights reserved.
// hvisor is licensed under Mulan PSL v2.
// You can use this software according to the terms and conditions of the Mulan PSL v2.
// You may obtain a copy of Mulan PSL v2 at:
//          http://license.coscl.org.cn/MulanPSL2
// THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY KIND,
// EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
// MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.
// See the Mulan PSL v2 for more details.

//! HyperAMP MMIO 中断优化模块
//!
//! 功能：
//! - 捕获 Root Linux 对 MMIO 控制区域的写操作
//! - 读取 target_zone_id 和 service_id
//! - 直接注入中断到目标 Zone
//!
//! 性能优化：
//! - 原方案：ioctl → copy_from_user → hvisor_call → send_event → inject_irq (~48.7ms)
//! - 新方案：MMIO write → trap → read control → inject_irq (~5-10μs)
//! - 性能提升：约 5000 倍

use crate::device::irqchip::set_ispender;
use crate::error::HvResult;
use crate::memory::MMIOAccess;

/// HyperAMP MMIO 控制区域偏移定义
const HYPERAMP_CTRL_IPI_TRIGGER_OFFSET: usize = 0x00;  // ipi_trigger 字段
const HYPERAMP_CTRL_TARGET_ZONE_OFFSET: usize = 0x04;  // target_zone_id 字段
const HYPERAMP_CTRL_SERVICE_ID_OFFSET: usize = 0x08;   // service_id 字段

/// HyperAMP MMIO 处理函数
///
/// 当 Root Linux 写入 ipi_trigger 字段时触发此函数
///
/// # 参数
/// - `mmio`: MMIO 访问信息
/// - `base`: MMIO 区域基地址（0xde410000）
///
/// # 返回值
/// - `Ok(())`: 处理成功
/// - `Err(HvError)`: 处理失败
pub fn mmio_hyperamp_handler(mmio: &mut MMIOAccess, base: usize) -> HvResult {
    let is_write = mmio.is_write;
    let offset = mmio.address;
    
    // 处理读操作：返回固定值（预热读取不需要真实数据）
    if !is_write {
        // 读取任何字段都返回 0（避免卡住）
        mmio.value = 0;
        return Ok(());
    }
    
    // 只处理写入 ipi_trigger 字段的操作
    if offset != HYPERAMP_CTRL_IPI_TRIGGER_OFFSET {
        // 写入非 ipi_trigger 字段：忽略
        warn!(
            "HyperAMP MMIO: non-trigger write [offset={:#x}, value={:#x}]",
            offset, mmio.value
        );
        return Ok(());
    }
    
    // 从 mmio.value 中解包参数（单次写入传递所有信息）
    // 格式：高 16 位为 target_zone_id，低 16 位为 service_id
    let packed_value = mmio.value as u32;
    
    // 预热写入检测：值为 0 时只建立页表，不注入中断
    if packed_value == 0 {
        // 预热成功，跳过中断注入
        debug!("HyperAMP MMIO: warmup write detected [value=0], skipping interrupt injection");
        return Ok(());
    }
    
    let target_zone_id = (packed_value >> 16) & 0xFFFF;
    let service_id = packed_value & 0xFFFF;
    
    // 串口输出非常慢（~100μs/字符），一行日志约 10-12ms
    // 生产环境应该禁用日志，调试时可临时启用
    debug!(
        "[HyperAMP MMIO]: trigger interrupt [packed={:#x}, zone={}, service={}, base={:#x}]",
        packed_value, target_zone_id, service_id, base
    );
    
    // 根据 target_zone_id 和 service_id 查找中断号
    let irq_num = get_irq_by_service(target_zone_id, service_id);
    
    // 直接注入中断（类似 IVC 的实现）
    set_ispender(irq_num / 32, 1 << (irq_num % 32));
    

    debug!("[HyperAMP MMIO]: interrupt injected [irq={}]", irq_num);
    
    Ok(())
}

/// 根据 zone_id 和 service_id 查找对应的中断号
///
/// # 参数
/// - `target_zone_id`: 目标 Zone ID
/// - `service_id`: 服务 ID（当前未使用，保留用于未来扩展）
///
/// # 返回值
/// - 对应的中断号
///
/// # 注意
/// 当前使用硬编码映射，未来应该从配置文件读取
fn get_irq_by_service(target_zone_id: u32, _service_id: u32) -> usize {
    // TODO: 从配置中读取 zone_id + service_id → irq 的映射
    // 当前使用硬编码（临时方案）
    match target_zone_id {
        1 => 74,  // Zone 1 (NPUcore) 使用 IRQ 74 (SWI1)
        _ => {
            warn!(
                "HyperAMP MMIO: no IRQ mapping for zone {}, using default IRQ 74",
                target_zone_id
            );
            74  // 默认返回 IRQ 74
        }
    }
}

// 注意：不需要为 Zone 添加 hyperamp_ctrl_pa() 方法
// 因为 MMIO handler 直接接收 base 参数（0xde410000）
// 这个 base 参数由 zone.rs 中的 mmio_region_register() 提供
