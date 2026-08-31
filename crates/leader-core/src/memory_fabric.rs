use crate::memory_map::{MemoryOwner, RAM_BASE, RAM_REGION, ROM_BASE, ROM_REGION, VRAM_BASE, VRAM_REGION};

pub const BYTES_PER_PAGE: usize = 256;
pub const BITS_PER_BYTE: usize = 8;
pub const BYTE_GRID_SIDE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryFabricSpec {
    pub owner: MemoryOwner,
    pub group_id: &'static str,
    pub page_prefix: &'static str,
    pub page_count: usize,
    pub byte_count: usize,
    pub bit_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalMemoryAddress {
    pub owner: MemoryOwner,
    pub absolute: u16,
    pub page: usize,
    pub byte: usize,
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalMemoryByte {
    pub address: PhysicalMemoryAddress,
    pub value: u8,
    pub bits_lsb_first: [bool; BITS_PER_BYTE],
}

#[must_use]
pub const fn memory_fabric_specs() -> [MemoryFabricSpec; 3] {
    [
        MemoryFabricSpec {
            owner: MemoryOwner::Rom,
            group_id: "romsys",
            page_prefix: "romPage",
            page_count: ROM_REGION.len() / BYTES_PER_PAGE,
            byte_count: ROM_REGION.len(),
            bit_count: ROM_REGION.len() * BITS_PER_BYTE,
        },
        MemoryFabricSpec {
            owner: MemoryOwner::Ram,
            group_id: "ramsys",
            page_prefix: "ramPage",
            page_count: RAM_REGION.len() / BYTES_PER_PAGE,
            byte_count: RAM_REGION.len(),
            bit_count: RAM_REGION.len() * BITS_PER_BYTE,
        },
        MemoryFabricSpec {
            owner: MemoryOwner::Vram,
            group_id: "vramsys",
            page_prefix: "vramPage",
            page_count: VRAM_REGION.len() / BYTES_PER_PAGE,
            byte_count: VRAM_REGION.len(),
            bit_count: VRAM_REGION.len() * BITS_PER_BYTE,
        },
    ]
}

#[must_use]
pub const fn total_memory_bytes() -> usize {
    ROM_REGION.len() + RAM_REGION.len() + VRAM_REGION.len()
}

#[must_use]
pub const fn total_memory_bit_cells() -> usize {
    total_memory_bytes() * BITS_PER_BYTE
}

#[must_use]
pub fn resolve_physical_memory_address(address: u16) -> Option<PhysicalMemoryAddress> {
    let (owner, base, length) = if ROM_REGION.contains(address) {
        (MemoryOwner::Rom, ROM_BASE, ROM_REGION.len())
    } else if RAM_REGION.contains(address) {
        (MemoryOwner::Ram, RAM_BASE, RAM_REGION.len())
    } else if VRAM_REGION.contains(address) {
        (MemoryOwner::Vram, VRAM_BASE, VRAM_REGION.len())
    } else {
        return None;
    };

    let offset = usize::from(address - base);
    debug_assert!(offset < length);
    let page = offset / BYTES_PER_PAGE;
    let byte = offset % BYTES_PER_PAGE;
    Some(PhysicalMemoryAddress {
        owner,
        absolute: address,
        page,
        byte,
        row: byte / BYTE_GRID_SIDE,
        column: byte % BYTE_GRID_SIDE,
    })
}

#[must_use]
pub fn resolve_physical_memory_byte(address: u16, value: u8) -> Option<PhysicalMemoryByte> {
    let address = resolve_physical_memory_address(address)?;
    let mut bits = [false; BITS_PER_BYTE];
    let mut bit = 0;
    while bit < BITS_PER_BYTE {
        bits[bit] = value & (1 << bit) != 0;
        bit += 1;
    }
    Some(PhysicalMemoryByte {
        address,
        value,
        bits_lsb_first: bits,
    })
}

#[must_use]
pub fn page_node_id(address: PhysicalMemoryAddress) -> String {
    let prefix = match address.owner {
        MemoryOwner::Rom => "romPage",
        MemoryOwner::Ram => "ramPage",
        MemoryOwner::Vram => "vramPage",
        MemoryOwner::Mmio | MemoryOwner::Unmapped => return String::new(),
    };
    format!("{prefix}{}", address.page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_counts_match_declared_memory_map() {
        let specs = memory_fabric_specs();
        assert_eq!(specs[0].page_count, 32);
        assert_eq!(specs[1].page_count, 96);
        assert_eq!(specs[2].page_count, 8);
        assert_eq!(total_memory_bytes(), 34_816);
        assert_eq!(total_memory_bit_cells(), 278_528);
    }

    #[test]
    fn address_resolves_to_exact_page_byte_row_and_column() {
        let cell = resolve_physical_memory_address(RAM_BASE + 0x2314).unwrap();
        assert_eq!(cell.owner, MemoryOwner::Ram);
        assert_eq!(cell.page, 0x23);
        assert_eq!(cell.byte, 0x14);
        assert_eq!(cell.row, 1);
        assert_eq!(cell.column, 4);
        assert_eq!(page_node_id(cell), "ramPage35");
    }

    #[test]
    fn byte_exposes_all_eight_physical_bit_values() {
        let byte = resolve_physical_memory_byte(VRAM_BASE + 7, 0b1010_0101).unwrap();
        assert_eq!(
            byte.bits_lsb_first,
            [true, false, true, false, false, true, false, true]
        );
    }

    #[test]
    fn mmio_is_not_part_of_memory_bitcell_fabric() {
        assert_eq!(resolve_physical_memory_address(0xA000), None);
    }
}
