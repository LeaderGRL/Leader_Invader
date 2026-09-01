#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegion {
    pub name: &'static str,
    pub start: u16,
    pub end: u16,
}

impl MemoryRegion {
    #[must_use]
    pub const fn new(name: &'static str, start: u16, end: u16) -> Self {
        Self { name, start, end }
    }

    #[must_use]
    pub const fn contains(self, address: u16) -> bool {
        address >= self.start && address <= self.end
    }

    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end as usize - self.start as usize + 1
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start > self.end
    }
}

pub const ROM_BASE: u16 = 0x0000;
pub const ROM_END: u16 = 0x1FFF;
pub const ROM_REGION: MemoryRegion = MemoryRegion::new("program_rom", ROM_BASE, ROM_END);
pub const ROM_CAPACITY: usize = 0x2000;

pub const RAM_BASE: u16 = 0x2000;
pub const RAM_END: u16 = 0x7FFF;
pub const RAM_REGION: MemoryRegion = MemoryRegion::new("work_ram", RAM_BASE, RAM_END);

pub const ENEMY_SHOT_RAM_BASE: u16 = RAM_BASE + 0x20;
pub const ENEMY_SHOT_RAM_BYTES: u16 = 9;
pub const ENEMY_SHOT_RAM_REGION: MemoryRegion = MemoryRegion::new(
    "enemy_shot_ram",
    ENEMY_SHOT_RAM_BASE,
    ENEMY_SHOT_RAM_BASE + ENEMY_SHOT_RAM_BYTES - 1,
);

pub const SHIELD_RAM_BASE: u16 = RAM_BASE + 0x40;
pub const SHIELD_RAM_BYTES: u16 = 64;
pub const SHIELD_RAM_REGION: MemoryRegion = MemoryRegion::new(
    "shield_ram",
    SHIELD_RAM_BASE,
    SHIELD_RAM_BASE + SHIELD_RAM_BYTES - 1,
);

pub const STACK_BASE: u16 = 0x7F00;
pub const STACK_END: u16 = 0x7FFF;
pub const STACK_REGION: MemoryRegion = MemoryRegion::new("stack", STACK_BASE, STACK_END);

pub const VRAM_BASE: u16 = 0x8000;
pub const VRAM_END: u16 = 0x87FF;
pub const VRAM_REGION: MemoryRegion = MemoryRegion::new("video_ram", VRAM_BASE, VRAM_END);
pub const FRAMEBUFFER_BYTES: usize = 128 * 96 / 8;

pub const MMIO_BASE: u16 = 0xA000;
pub const MMIO_END: u16 = 0xA1FF;
pub const MMIO_REGION: MemoryRegion = MemoryRegion::new("mmio", MMIO_BASE, MMIO_END);

pub const INPUT_PORT: u16 = 0xA000;
pub const SHIFT_DATA: u16 = 0xA010;
pub const SHIFT_OFFSET: u16 = 0xA011;
pub const SHIFT_RESULT: u16 = 0xA012;
pub const DEVICE_CMD: u16 = 0xA100;
pub const DEVICE_STATUS: u16 = 0xA101;
pub const DEVICE_ARG0: u16 = 0xA102;
pub const DEVICE_ARG1: u16 = 0xA103;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmioAccess {
    InputOnly,
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl MmioAccess {
    #[must_use]
    pub const fn allows_read(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }

    #[must_use]
    pub const fn allows_write(self) -> bool {
        matches!(self, Self::WriteOnly | Self::ReadWrite)
    }

    #[must_use]
    pub const fn allows_input(self) -> bool {
        matches!(self, Self::InputOnly)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmioPort {
    pub name: &'static str,
    pub address: u16,
    pub access: MmioAccess,
}

impl MmioPort {
    #[must_use]
    pub const fn new(name: &'static str, address: u16, access: MmioAccess) -> Self {
        Self {
            name,
            address,
            access,
        }
    }
}

pub const MMIO_PORTS: [MmioPort; 8] = [
    MmioPort::new("input", INPUT_PORT, MmioAccess::InputOnly),
    MmioPort::new("shift_data", SHIFT_DATA, MmioAccess::WriteOnly),
    MmioPort::new("shift_offset", SHIFT_OFFSET, MmioAccess::WriteOnly),
    MmioPort::new("shift_result", SHIFT_RESULT, MmioAccess::ReadOnly),
    MmioPort::new("device_cmd", DEVICE_CMD, MmioAccess::WriteOnly),
    MmioPort::new("device_status", DEVICE_STATUS, MmioAccess::ReadWrite),
    MmioPort::new("device_arg0", DEVICE_ARG0, MmioAccess::ReadWrite),
    MmioPort::new("device_arg1", DEVICE_ARG1, MmioAccess::ReadWrite),
];

#[must_use]
pub fn mmio_port(address: u16) -> Option<MmioPort> {
    MMIO_PORTS
        .iter()
        .copied()
        .find(|port| port.address == address)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOwner {
    Rom,
    Ram,
    Vram,
    Mmio,
    Unmapped,
}

#[must_use]
pub const fn owner(address: u16) -> MemoryOwner {
    if ROM_REGION.contains(address) {
        MemoryOwner::Rom
    } else if RAM_REGION.contains(address) {
        MemoryOwner::Ram
    } else if VRAM_REGION.contains(address) {
        MemoryOwner::Vram
    } else if MMIO_REGION.contains(address) {
        MemoryOwner::Mmio
    } else {
        MemoryOwner::Unmapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_regions_are_disjoint_and_keep_existing_addresses() {
        let regions = [ROM_REGION, RAM_REGION, VRAM_REGION, MMIO_REGION];
        for (index, region) in regions.iter().enumerate() {
            assert!(!region.is_empty());
            for other in regions.iter().skip(index + 1) {
                assert!(
                    !region.overlaps(*other),
                    "{} overlaps {}",
                    region.name,
                    other.name
                );
            }
        }
        assert_eq!(ROM_REGION.len(), 0x2000);
        assert_eq!(RAM_REGION.len(), 0x6000);
        assert_eq!(VRAM_REGION.len(), 0x0800);
        assert_eq!(MMIO_REGION.len(), 0x0200);
    }

    #[test]
    fn subregions_fit_inside_work_ram_without_overlap() {
        for region in [ENEMY_SHOT_RAM_REGION, SHIELD_RAM_REGION, STACK_REGION] {
            assert!(RAM_REGION.contains(region.start));
            assert!(RAM_REGION.contains(region.end));
        }
        assert!(!ENEMY_SHOT_RAM_REGION.overlaps(SHIELD_RAM_REGION));
        assert!(!ENEMY_SHOT_RAM_REGION.overlaps(STACK_REGION));
        assert!(!SHIELD_RAM_REGION.overlaps(STACK_REGION));
    }

    #[test]
    fn all_declared_ports_are_unique_and_belong_to_mmio() {
        for (index, port) in MMIO_PORTS.iter().enumerate() {
            assert_eq!(owner(port.address), MemoryOwner::Mmio, "{}", port.name);
            assert!(MMIO_PORTS
                .iter()
                .skip(index + 1)
                .all(|other| other.address != port.address));
            assert_eq!(mmio_port(port.address), Some(*port));
        }
        assert_eq!(mmio_port(MMIO_BASE + 1), None);
    }

    #[test]
    fn canonical_port_directions_are_explicit() {
        assert_eq!(mmio_port(INPUT_PORT).unwrap().access, MmioAccess::InputOnly);
        assert_eq!(mmio_port(SHIFT_DATA).unwrap().access, MmioAccess::WriteOnly);
        assert_eq!(mmio_port(SHIFT_OFFSET).unwrap().access, MmioAccess::WriteOnly);
        assert_eq!(mmio_port(SHIFT_RESULT).unwrap().access, MmioAccess::ReadOnly);
        assert_eq!(mmio_port(DEVICE_CMD).unwrap().access, MmioAccess::WriteOnly);
        assert_eq!(mmio_port(DEVICE_STATUS).unwrap().access, MmioAccess::ReadWrite);
    }

    #[test]
    fn framebuffer_fits_inside_physical_vram() {
        assert!(FRAMEBUFFER_BYTES <= VRAM_REGION.len());
        assert_eq!(FRAMEBUFFER_BYTES, 1536);
    }

    #[test]
    fn ownership_boundaries_are_exact() {
        assert_eq!(owner(0x0000), MemoryOwner::Rom);
        assert_eq!(owner(0x1FFF), MemoryOwner::Rom);
        assert_eq!(owner(0x2000), MemoryOwner::Ram);
        assert_eq!(owner(0x7FFF), MemoryOwner::Ram);
        assert_eq!(owner(0x8000), MemoryOwner::Vram);
        assert_eq!(owner(0x87FF), MemoryOwner::Vram);
        assert_eq!(owner(0x8800), MemoryOwner::Unmapped);
        assert_eq!(owner(0x9FFF), MemoryOwner::Unmapped);
        assert_eq!(owner(0xA000), MemoryOwner::Mmio);
        assert_eq!(owner(0xA1FF), MemoryOwner::Mmio);
        assert_eq!(owner(0xA200), MemoryOwner::Unmapped);
    }
}
