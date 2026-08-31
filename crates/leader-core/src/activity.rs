use std::collections::HashSet;

use crate::isa::Reg;
use crate::topology::{SignalKind, Topology};
use crate::trace::PhaseKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalActivityLink {
    pub id: String,
    pub signal: SignalKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalBitChange {
    pub node_id: String,
    pub before: bool,
    pub after: bool,
}

/// Returns the physical node ids that participate in a native trace phase.
///
/// This mapping belongs to the core because it describes the physical machine,
/// not a particular renderer. Frontends may choose how to present these ids,
/// but they must not independently reconstruct machine activity semantics.
#[must_use]
pub fn physical_activity_nodes(phase: PhaseKind, address: Option<u16>) -> Vec<String> {
    let mut ids = match phase {
        PhaseKind::Fetch => vec![
            "clock", "clkGate", "phase0", "pcMuxLo", "pcMuxHi", "addrBuf",
        ],
        PhaseKind::Decode => vec![
            "opHi", "opLo", "decA", "decB", "microAddr", "microRom",
        ],
        PhaseKind::Input => vec!["kbd", "inputLatch", "dataBuf"],
        PhaseKind::MemoryRead => vec!["addrBuf", "dataBuf"],
        PhaseKind::Alu => vec![
            "readMuxA",
            "readMuxB",
            "aluSel",
            "writeBus",
            "flagZ",
            "flagC",
            "flagN",
        ],
        PhaseKind::MemoryWrite => vec!["writeBus", "dataBuf", "ctrlBuf"],
        PhaseKind::Dma => vec![
            "arb",
            "dmaAddr",
            "dmaData",
            "dataBuf",
            "vramPageDec",
            "vramPage0",
        ],
        PhaseKind::Scanout => vec![
            "spriteRom",
            "xCounter",
            "yCounter",
            "pixelMux",
            "scanShift",
            "hsync",
            "vsync",
            "display",
        ],
        PhaseKind::VBlank => vec!["vsync", "timer", "irqAnd", "irqLatch", "microAddr"],
    }
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();

    if phase == PhaseKind::Alu {
        for bit in 0..8 {
            for prefix in ["xorA", "xorB", "andA", "andB", "orC", "muxR"] {
                ids.push(format!("{prefix}{bit}"));
            }
        }
    }

    if let Some(address) = address {
        match address {
            0x0000..=0x1fff => {
                ids.push("romRowDec".to_owned());
                ids.push(format!("romPage{}", address >> 8));
            }
            0x2000..=0x7fff => {
                ids.push("ramPageDec".to_owned());
                ids.push(format!("ramPage{}", ((address - 0x2000) >> 8).min(95)));
            }
            0x8000..=0x87ff => {
                ids.push("vramPageDec".to_owned());
                ids.push(format!("vramPage{}", ((address - 0x8000) >> 8).min(7)));
            }
            _ => {}
        }
    }

    ids
}

/// Returns the canonical physical links that connect participating activity nodes.
///
/// This is a conservative activity subgraph: a link is active only when both of
/// its physical endpoint nodes participate in the same native activity snapshot.
/// More precise electrical stage timing can refine this contract later without
/// requiring frontend-side topology inference.
#[must_use]
pub fn physical_activity_links(
    topology: &Topology,
    phase: PhaseKind,
    address: Option<u16>,
) -> Vec<PhysicalActivityLink> {
    let nodes = physical_activity_nodes(phase, address);
    let active = nodes.iter().map(String::as_str).collect::<HashSet<_>>();

    topology
        .links
        .iter()
        .filter(|link| {
            active.contains(link.from.as_str()) && active.contains(link.to.as_str())
        })
        .map(|link| PhysicalActivityLink {
            id: link.id.clone(),
            signal: link.signal,
        })
        .collect()
}

#[must_use]
pub fn physical_register_bit_changes(reg: Reg, before: u8, after: u8) -> Vec<PhysicalBitChange> {
    physical_byte_bit_changes(&format!("reg{}", reg.name()), before, after)
}

#[must_use]
pub fn physical_pc_bit_changes(before: u16, after: u16) -> Vec<PhysicalBitChange> {
    physical_word_bit_changes("pcBit", before, after)
}

#[must_use]
pub fn physical_sp_bit_changes(before: u16, after: u16) -> Vec<PhysicalBitChange> {
    physical_word_bit_changes("spBit", before, after)
}

#[must_use]
pub fn physical_flag_bit_changes(before: u8, after: u8) -> Vec<PhysicalBitChange> {
    const FLAGS: [(&str, u8); 3] = [("flagZ", 0), ("flagC", 1), ("flagN", 2)];
    FLAGS
        .into_iter()
        .filter_map(|(node_id, bit)| {
            let before_bit = before & (1 << bit) != 0;
            let after_bit = after & (1 << bit) != 0;
            (before_bit != after_bit).then(|| PhysicalBitChange {
                node_id: node_id.to_owned(),
                before: before_bit,
                after: after_bit,
            })
        })
        .collect()
}

fn physical_byte_bit_changes(prefix: &str, before: u8, after: u8) -> Vec<PhysicalBitChange> {
    (0..8)
        .filter_map(|bit| {
            let before_bit = before & (1 << bit) != 0;
            let after_bit = after & (1 << bit) != 0;
            (before_bit != after_bit).then(|| PhysicalBitChange {
                node_id: format!("{prefix}{bit}"),
                before: before_bit,
                after: after_bit,
            })
        })
        .collect()
}

fn physical_word_bit_changes(prefix: &str, before: u16, after: u16) -> Vec<PhysicalBitChange> {
    (0..16)
        .filter_map(|bit| {
            let before_bit = before & (1 << bit) != 0;
            let after_bit = after & (1 << bit) != 0;
            (before_bit != after_bit).then(|| PhysicalBitChange {
                node_id: format!("{prefix}{bit}"),
                before: before_bit,
                after: after_bit,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alu_activity_expands_to_all_eight_physical_slices() {
        let ids = physical_activity_nodes(PhaseKind::Alu, None);
        for bit in 0..8 {
            for prefix in ["xorA", "xorB", "andA", "andB", "orC", "muxR"] {
                assert!(ids.contains(&format!("{prefix}{bit}")));
            }
        }
        assert!(ids.contains(&"flagC".to_owned()));
    }

    #[test]
    fn memory_activity_selects_the_exact_canonical_page() {
        let rom = physical_activity_nodes(PhaseKind::MemoryRead, Some(0x1234));
        assert!(rom.contains(&"romPage18".to_owned()));

        let ram = physical_activity_nodes(PhaseKind::MemoryWrite, Some(0x3456));
        assert!(ram.contains(&"ramPage20".to_owned()));

        let vram = physical_activity_nodes(PhaseKind::Dma, Some(0x83fe));
        assert!(vram.contains(&"vramPage3".to_owned()));
    }

    #[test]
    fn activity_links_are_derived_from_the_canonical_topology() {
        let topology = crate::build_topology();
        let links = physical_activity_links(&topology, PhaseKind::Scanout, None);
        assert!(links.iter().any(|link| link.id == "g-spriteRom-pixelMux"));
        assert!(links.iter().any(|link| link.id == "g-scanShift-display"));
        assert!(links.iter().all(|link| {
            topology.links.iter().any(|candidate| {
                candidate.id == link.id && candidate.signal == link.signal
            })
        }));
    }

    #[test]
    fn architectural_register_changes_map_to_final_aligned_register_nodes() {
        let changes = physical_register_bit_changes(Reg::C, 0b0000_0001, 0b0000_0101);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].node_id, "regC2");
        assert!(!changes[0].before);
        assert!(changes[0].after);
    }

    #[test]
    fn pc_sp_and_flags_changes_identify_only_flipped_physical_bits() {
        let pc = physical_pc_bit_changes(0x00ff, 0x0100);
        assert_eq!(pc.len(), 9);
        assert!(pc.iter().any(|change| change.node_id == "pcBit8" && change.after));

        let sp = physical_sp_bit_changes(0x7fff, 0x7ffe);
        assert_eq!(sp.len(), 1);
        assert_eq!(sp[0].node_id, "spBit0");

        let flags = physical_flag_bit_changes(0b001, 0b110);
        assert_eq!(flags.len(), 3);
        assert!(flags.iter().any(|change| change.node_id == "flagN" && change.after));
    }
}
