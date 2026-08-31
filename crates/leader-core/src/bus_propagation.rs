use crate::memory_map::{owner, MemoryOwner, RAM_BASE, VRAM_BASE};
use crate::topology::{Link, SignalKind, Topology};
use crate::trace::{BusAddressSource, BusTransactionEvent, BusTransactionKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalBusLinkValue {
    pub link_id: String,
    pub rank: u8,
    pub stage: &'static str,
    pub signal: SignalKind,
    pub value: u16,
    pub width: u8,
}

/// Expands one native bus transaction into dependency-ordered values travelling
/// over the canonical physical bus and memory-page links.
///
/// `rank` is a causal/dependency order for visualization, not an analog delay.
/// Every returned link is looked up in the final topology by physical endpoints
/// and signal kind, so consumers never need to reproduce memory-map or routing
/// semantics outside `leader-core`.
#[must_use]
pub fn physical_bus_link_values(
    topology: &Topology,
    event: BusTransactionEvent,
) -> Vec<PhysicalBusLinkValue> {
    let Some(address) = event.address else {
        return Vec::new();
    };

    let mut values = Vec::new();
    match event.address_source {
        BusAddressSource::ProgramCounter | BusAddressSource::Cpu => {
            for bit in 0..16_u8 {
                push_link(
                    &mut values,
                    topology,
                    &format!("marBit{bit}"),
                    "addrBuf",
                    SignalKind::Address,
                    1,
                    "address_driver",
                    u16::from((address & (1_u16 << bit)) != 0),
                    1,
                );
            }
        }
        BusAddressSource::Dma => {
            push_link(
                &mut values,
                topology,
                "dmaAddr",
                "arb",
                SignalKind::Address,
                1,
                "dma_address_driver",
                address,
                16,
            );
            push_link(
                &mut values,
                topology,
                "arb",
                "addrBuf",
                SignalKind::Address,
                2,
                "address_arbitration",
                address,
                16,
            );
        }
        BusAddressSource::None => {}
    }

    let Some(memory) = memory_target(address) else {
        return values;
    };

    push_link(
        &mut values,
        topology,
        "addrBuf",
        memory.page_decoder,
        SignalKind::Address,
        3,
        "page_decode_address",
        address,
        16,
    );
    push_link(
        &mut values,
        topology,
        "addrBuf",
        memory.byte_decoder,
        SignalKind::Address,
        3,
        "byte_decode_address",
        address,
        16,
    );
    push_link(
        &mut values,
        topology,
        memory.page_decoder,
        &memory.page_node,
        SignalKind::Control,
        4,
        "page_select",
        1,
        1,
    );

    let data = event.data.map(u16::from);
    match event.kind {
        BusTransactionKind::Fetch | BusTransactionKind::Read => {
            if let Some(data) = data {
                push_link(
                    &mut values,
                    topology,
                    &memory.page_node,
                    "dataBuf",
                    SignalKind::Data,
                    5,
                    "memory_read",
                    data,
                    8,
                );
            }
        }
        BusTransactionKind::Write => {
            if let Some(data) = data {
                push_link(
                    &mut values,
                    topology,
                    "dataBuf",
                    &memory.page_node,
                    SignalKind::Data,
                    5,
                    "memory_write",
                    data,
                    8,
                );
            }
        }
        BusTransactionKind::Dma | BusTransactionKind::Scanout => {
            if let Some(data) = data {
                push_link(
                    &mut values,
                    topology,
                    &memory.page_node,
                    "dataBuf",
                    SignalKind::Data,
                    5,
                    "vram_read",
                    data,
                    8,
                );
                push_link(
                    &mut values,
                    topology,
                    "dataBuf",
                    "dmaData",
                    SignalKind::Data,
                    6,
                    "dma_data_latch",
                    data,
                    8,
                );
            }
        }
        BusTransactionKind::Input => {}
    }

    values
}

struct MemoryTarget {
    page_decoder: &'static str,
    byte_decoder: &'static str,
    page_node: String,
}

fn memory_target(address: u16) -> Option<MemoryTarget> {
    match owner(address) {
        MemoryOwner::Rom => Some(MemoryTarget {
            page_decoder: "romRowDec",
            byte_decoder: "romByteDec",
            page_node: format!("romPage{}", address >> 8),
        }),
        MemoryOwner::Ram => Some(MemoryTarget {
            page_decoder: "ramPageDec",
            byte_decoder: "ramByteDec",
            page_node: format!("ramPage{}", (address - RAM_BASE) >> 8),
        }),
        MemoryOwner::Vram => Some(MemoryTarget {
            page_decoder: "vramPageDec",
            byte_decoder: "vramByteDec",
            page_node: format!("vramPage{}", (address - VRAM_BASE) >> 8),
        }),
        MemoryOwner::Mmio | MemoryOwner::Unmapped => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_link(
    values: &mut Vec<PhysicalBusLinkValue>,
    topology: &Topology,
    from: &str,
    to: &str,
    signal: SignalKind,
    rank: u8,
    stage: &'static str,
    value: u16,
    width: u8,
) {
    let Some(link) = find_link(topology, from, to, signal) else {
        return;
    };
    values.push(PhysicalBusLinkValue {
        link_id: link.id.clone(),
        rank,
        stage,
        signal,
        value,
        width,
    });
}

fn find_link(topology: &Topology, from: &str, to: &str, signal: SignalKind) -> Option<&Link> {
    topology
        .links
        .iter()
        .find(|link| link.from == from && link.to == to && link.signal == signal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{BusDataSource, BusTransactionEvent};

    fn event(
        address: u16,
        data: u8,
        address_source: BusAddressSource,
        kind: BusTransactionKind,
    ) -> BusTransactionEvent {
        BusTransactionEvent {
            frame: 7,
            ordinal: 3,
            pc: 0x0123,
            address: Some(address),
            data: Some(data),
            address_source,
            data_source: BusDataSource::Ram,
            kind,
            control: "TEST",
        }
    }

    #[test]
    fn rom_fetch_propagates_address_decode_select_then_data() {
        let topology = crate::build_topology();
        let values = physical_bus_link_values(
            &topology,
            event(0x1234, 0xa5, BusAddressSource::ProgramCounter, BusTransactionKind::Fetch),
        );

        assert!(values.iter().any(|value| {
            value.rank == 3
                && value.stage == "page_decode_address"
                && value.value == 0x1234
        }));
        assert!(values.iter().any(|value| {
            value.rank == 4
                && value.stage == "page_select"
                && topology.links.iter().any(|link| {
                    link.id == value.link_id
                        && link.from == "romRowDec"
                        && link.to == "romPage18"
                })
        }));
        assert!(values.iter().any(|value| {
            value.rank == 5
                && value.stage == "memory_read"
                && value.value == 0xa5
                && value.width == 8
        }));
    }

    #[test]
    fn ram_write_selects_exact_page_and_write_direction() {
        let topology = crate::build_topology();
        let values = physical_bus_link_values(
            &topology,
            event(0x3456, 0x6c, BusAddressSource::Cpu, BusTransactionKind::Write),
        );
        assert!(values.iter().any(|value| {
            value.stage == "page_select"
                && topology.links.iter().any(|link| {
                    link.id == value.link_id
                        && link.from == "ramPageDec"
                        && link.to == "ramPage20"
                })
        }));
        assert!(values.iter().any(|value| {
            value.stage == "memory_write"
                && value.value == 0x6c
                && topology.links.iter().any(|link| {
                    link.id == value.link_id
                        && link.from == "dataBuf"
                        && link.to == "ramPage20"
                })
        }));
    }

    #[test]
    fn dma_propagates_through_arbiter_vram_page_and_data_latch() {
        let topology = crate::build_topology();
        let values = physical_bus_link_values(
            &topology,
            event(0x83fe, 0x5a, BusAddressSource::Dma, BusTransactionKind::Dma),
        );
        assert!(values.iter().any(|value| value.stage == "dma_address_driver" && value.rank == 1));
        assert!(values.iter().any(|value| value.stage == "address_arbitration" && value.rank == 2));
        assert!(values.iter().any(|value| {
            value.stage == "page_select"
                && topology.links.iter().any(|link| {
                    link.id == value.link_id
                        && link.from == "vramPageDec"
                        && link.to == "vramPage3"
                })
        }));
        assert!(values.iter().any(|value| {
            value.stage == "vram_read"
                && topology.links.iter().any(|link| {
                    link.id == value.link_id
                        && link.from == "vramPage3"
                        && link.to == "dataBuf"
                })
        }));
        assert!(values.iter().any(|value| value.stage == "dma_data_latch" && value.rank == 6));
    }

    #[test]
    fn every_reported_bus_propagation_link_exists_in_final_topology() {
        let topology = crate::build_topology();
        for event in [
            event(0x01fe, 0x11, BusAddressSource::ProgramCounter, BusTransactionKind::Fetch),
            event(0x7f20, 0x22, BusAddressSource::Cpu, BusTransactionKind::Read),
            event(0x8123, 0x33, BusAddressSource::Cpu, BusTransactionKind::Write),
            event(0x83fe, 0x44, BusAddressSource::Dma, BusTransactionKind::Dma),
        ] {
            for value in physical_bus_link_values(&topology, event) {
                assert!(
                    topology.links.iter().any(|link| link.id == value.link_id),
                    "reported missing physical link {}",
                    value.link_id
                );
            }
        }
    }
}
