// Copyright (c) 2025 Syswonder
// hvisor is licensed under Mulan PSL v2.
// You can use this software according to the terms and conditions of the Mulan PSL v2.
// You may obtain a copy of Mulan PSL v2 at:
//     http://license.coscl.org.cn/MulanPSL2
// THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY KIND, EITHER
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT, MERCHANTABILITY OR
// FIT FOR A PARTICULAR PURPOSE.
// See the Mulan PSL v2 for more details.
//
// Syswonder Website:
//      https://www.syswonder.org
//
// Authors:
//
use crate::{arch::zone::HvArchZoneConfig, config::*};

pub const BOARD_NAME: &str = "phytium-pi";

pub const BOARD_NCPUS: usize = 4;

pub const ROOT_ZONE_DTB_ADDR: u64 = 0xa0000000;
pub const ROOT_ZONE_KERNEL_ADDR: u64 = 0xa0400000;
pub const ROOT_ZONE_ENTRY: u64 = 0xa0400000;
pub const ROOT_ZONE_CPUS: u64 = (1 << 1) | (1 << 0);

pub const ROOT_ZONE_NAME: &str = "root-linux";

pub const ROOT_ZONE_MEMORY_REGIONS: [HvConfigMemoryRegion; 12] = [
    // ram
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x80000000,
        virtual_start: 0x80000000,
        size: 0x80000000,
    }, 
    // soc@0
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x28000000,  
        virtual_start: 0x28000000,
        size: 0x00100000,           
    },
    //iommu
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x30000000,  
        virtual_start: 0x30000000,
        size: 0x800000,           
    },
    // ethernet0
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x3200c000,  
        virtual_start: 0x3200c000,
        size: 0x00002000,            // 8KB
    },
    // USB
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x31800000,  // usb2@31800000
        virtual_start: 0x31800000,
        size: 0x00080000,            // 512KB
    },
    // USB2 @32800000 - Host Mode
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x32800000,
        virtual_start: 0x32800000,
        size: 0x00040000, // 256KB
    },
      // USB2 @32840000 - Host Mode
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x32840000,
        virtual_start: 0x32840000,
        size: 0x00040000, // 256KB
    },
    // USB3 @31a08000 - XHCI Controller
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x31a08000,
        virtual_start: 0x31a08000,
        size: 0x00018000, // 96KB
    },

    // USB3 @31a28000 - XHCI Controller
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x31a28000,
        virtual_start: 0x31a28000,
        size: 0x00018000, // 96KB
    },
    // sata
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x32014000,
        virtual_start: 0x32014000,
        size: 0x1000, // 96KB
    },    
    //mailbox
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x32a00000,  
        virtual_start: 0x32a00000,
        size: 0x1000,           
    },
    //sram
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0x32a10000,  
        virtual_start: 0x32a10000,
        size: 0x2000,           
    }
];
//46-usb2,54-mailbox 64-hvisor,75-sata,87-net,104、105-mmc,116-uart,133、138-i2c,191-spi,
pub const ROOT_ZONE_IRQS: [u32; 13] = [46,54, 64, 75, 76, 78, 87, 104, 105, 116, 133, 138, 191];

pub const ROOT_ARCH_ZONE_CONFIG: HvArchZoneConfig = HvArchZoneConfig {
    gicd_base: 0x30800000,  
    gicd_size: 0x20000,       // 128KB
    gicr_base: 0x30880000,    
    gicr_size: 0x80000,       // 
    gicc_base: 0x0,   
    gicc_size: 0x0,       // 64KB
    gicc_offset: 0x0,         
    gich_base: 0x0,    
    gich_size: 0x0,       // 64KB
    gicv_base: 0x0,    
    gicv_size: 0x0,       // 64KB
    gits_base: 0x0,             
    gits_size: 0x0,
};

pub const ROOT_ZONE_IVC_CONFIG: [HvIvcConfig; 0] = [];
