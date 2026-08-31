use crate::trace::PhaseKind;

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
}
