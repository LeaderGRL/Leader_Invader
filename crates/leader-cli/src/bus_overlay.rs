use std::collections::BTreeSet;

use leader_core::{
    memory_owner, mmio_port, BusAddressSource, BusDataSource, BusTransactionKind, MatchTrace,
    MemoryOwner, MmioAccess, Topology, MMIO_PORTS,
};
use leader_svg::RenderConfig;

const MAX_BUS_EVENTS: usize = 150;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 || trace.bus_transactions.is_empty() {
        return svg;
    }
    let overlay = render(topology, trace, config);
    let Some(svg_close) = svg.rfind("</svg>") else {
        return svg;
    };
    let Some(world_close) = svg[..svg_close].rfind("</g>") else {
        return svg;
    };
    svg.insert_str(world_close, &overlay);
    svg
}

fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let indices = sampled_indices(trace);
    let total = config.total();
    let mut out = String::with_capacity(238_000);
    out.push_str("<g id=\"f3-native-bus\">\n");

    for index in indices {
        let event = &trace.bus_transactions[index];
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.016;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.022, total);
        let k3 = norm(moment + 0.145, total);
        let address = event
            .address
            .map_or_else(|| "none".to_owned(), |value| format!("{value:04X}"));
        let memory_owner = event
            .address
            .map_or("none", |address| owner_name(memory_owner(address)));
        let port = event.address.and_then(mmio_port);
        let mmio_port_name = port.map_or("none", |port| port.name);
        let mmio_access = port.map_or("none", |port| access_name(port.access));
        let data = event
            .data
            .map_or_else(|| "none".to_owned(), |value| format!("{value:02X}"));

        out.push_str(&format!(
            "<g opacity=\"0\" data-bus-kind=\"{}\" data-bus-address-source=\"{}\" data-bus-data-source=\"{}\" data-bus-memory-owner=\"{}\" data-bus-mmio-port=\"{}\" data-bus-mmio-access=\"{}\" data-bus-address=\"{}\" data-bus-data=\"{}\" data-bus-control=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
            event.kind.as_str(),
            event.address_source.as_str(),
            event.data_source.as_str(),
            memory_owner,
            mmio_port_name,
            mmio_access,
            address,
            data,
            event.control
        ));

        match event.address_source {
            BusAddressSource::ProgramCounter => {
                glow_node(&mut out, topology, "pcMuxLo", "#f2ae4f");
                glow_node(&mut out, topology, "pcMuxHi", "#f2ae4f");
                glow_node(&mut out, topology, "addrBuf", "#f2ae4f");
            }
            BusAddressSource::Cpu => {
                glow_node(&mut out, topology, "addrBuf", "#f2ae4f");
            }
            BusAddressSource::Dma => {
                glow_node(&mut out, topology, "arb", "#e8e677");
                glow_node(&mut out, topology, "dmaAddr", "#f2ae4f");
            }
            BusAddressSource::None => {}
        }

        match event.data_source {
            BusDataSource::Rom => {
                glow_node(&mut out, topology, "romRowDec", "#ef7caf");
                glow_node(&mut out, topology, "dataBuf", "#4bc8f3");
            }
            BusDataSource::Ram => {
                glow_node(&mut out, topology, "ramPageDec", "#ef7caf");
                glow_node(&mut out, topology, "dataBuf", "#4bc8f3");
            }
            BusDataSource::Vram => {
                glow_node(&mut out, topology, "vramPageDec", "#ef7caf");
                glow_node(&mut out, topology, "dmaData", "#72d4e7");
                glow_node(&mut out, topology, "dataBuf", "#4bc8f3");
            }
            BusDataSource::Cpu => {
                glow_node(&mut out, topology, "writeBus", "#67d9b3");
                glow_node(&mut out, topology, "dataBuf", "#4bc8f3");
            }
            BusDataSource::Device => {
                glow_node(&mut out, topology, "inputLatch", "#ef7caf");
                glow_node(&mut out, topology, "dataBuf", "#4bc8f3");
            }
            BusDataSource::None => {}
        }

        if !matches!(event.kind, BusTransactionKind::Input) {
            glow_node(&mut out, topology, "ctrlBuf", "#ef7caf");
        }
        if matches!(
            event.kind,
            BusTransactionKind::Dma | BusTransactionKind::Scanout
        ) {
            glow_node(&mut out, topology, "arb", "#e8e677");
        }
        if event.kind == BusTransactionKind::Scanout {
            glow_node(&mut out, topology, "scanShift", "#72d4e7");
            glow_node(&mut out, topology, "display", "#72d4e7");
        }

        out.push_str("</g>\n");
    }

    out.push_str("</g>\n");
    out
}

fn sampled_indices(trace: &MatchTrace) -> Vec<usize> {
    let events = &trace.bus_transactions;
    if events.len() <= MAX_BUS_EVENTS {
        return (0..events.len()).collect();
    }

    let mut selected = BTreeSet::new();
    selected.insert(0usize);
    selected.insert(events.len() - 1);

    for owner in [
        MemoryOwner::Rom,
        MemoryOwner::Ram,
        MemoryOwner::Vram,
        MemoryOwner::Mmio,
    ] {
        if let Some(index) = events.iter().position(|event| {
            event
                .address
                .is_some_and(|address| memory_owner(address) == owner)
        }) {
            selected.insert(index);
        }
    }

    for kind in [
        BusTransactionKind::Fetch,
        BusTransactionKind::Read,
        BusTransactionKind::Write,
        BusTransactionKind::Input,
        BusTransactionKind::Dma,
        BusTransactionKind::Scanout,
    ] {
        if let Some(index) = events.iter().position(|event| event.kind == kind) {
            selected.insert(index);
        }
    }

    for port in MMIO_PORTS {
        if let Some(index) = events
            .iter()
            .position(|event| event.address == Some(port.address))
        {
            selected.insert(index);
        }
    }

    let remaining = MAX_BUS_EVENTS.saturating_sub(selected.len());
    if remaining > 0 {
        let stride = events.len().div_ceil(remaining).max(1);
        for index in (0..events.len()).step_by(stride) {
            if selected.len() >= MAX_BUS_EVENTS {
                break;
            }
            selected.insert(index);
        }
    }

    selected.into_iter().take(MAX_BUS_EVENTS).collect()
}

const fn owner_name(owner: MemoryOwner) -> &'static str {
    match owner {
        MemoryOwner::Rom => "rom",
        MemoryOwner::Ram => "ram",
        MemoryOwner::Vram => "vram",
        MemoryOwner::Mmio => "mmio",
        MemoryOwner::Unmapped => "unmapped",
    }
}

const fn access_name(access: MmioAccess) -> &'static str {
    match access {
        MmioAccess::InputOnly => "input_only",
        MmioAccess::ReadOnly => "read_only",
        MmioAccess::WriteOnly => "write_only",
        MmioAccess::ReadWrite => "read_write",
    }
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"8\" fill=\"{}\" fill-opacity=\".18\" stroke=\"{}\" stroke-width=\"9\" filter=\"url(#glow)\"/>",
        b.x - 3.0,
        b.y - 3.0,
        b.w + 6.0,
        b.h + 6.0,
        color,
        color
    ));
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(31)) * 0.0025
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn bus_overlay_exposes_exact_native_transactions_and_mapped_owner() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-bus-overlay", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);

        assert!(baseline.contains("id=\"f3-native-bus\""));
        assert!(baseline.contains("data-bus-kind=\"fetch\""));
        assert!(baseline.contains("data-bus-address-source=\"program_counter\""));
        assert!(baseline.contains("data-bus-data-source=\"rom\""));
        assert!(baseline.contains("data-bus-memory-owner=\"rom\""));
        assert!(baseline.contains("data-bus-memory-owner=\"ram\""));
        assert!(baseline.contains("data-bus-memory-owner=\"mmio\""));
        assert!(baseline.contains("data-bus-mmio-port=\"shift_data\""));
        assert!(baseline.contains("data-bus-mmio-access=\"write_only\""));
        assert!(baseline.contains("data-bus-mmio-port=\"shift_result\""));
        assert!(baseline.contains("data-bus-mmio-access=\"read_only\""));
        assert!(baseline.contains("data-bus-address=\""));
        assert!(baseline.contains("data-bus-data=\""));
        assert!(baseline.contains("data-bus-control=\""));

        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }

    #[test]
    fn bus_sampling_is_bounded_and_preserves_owners_kinds_and_exercised_ports() {
        let trace = Machine::run_match("m3-bus-owner-sampling", 5000);
        let selected = sampled_indices(&trace);
        assert!(selected.len() <= MAX_BUS_EVENTS);

        for owner in [
            MemoryOwner::Rom,
            MemoryOwner::Ram,
            MemoryOwner::Vram,
            MemoryOwner::Mmio,
        ] {
            if trace.bus_transactions.iter().any(|event| {
                event
                    .address
                    .is_some_and(|address| memory_owner(address) == owner)
            }) {
                assert!(selected.iter().any(|index| {
                    trace.bus_transactions[*index]
                        .address
                        .is_some_and(|address| memory_owner(address) == owner)
                }));
            }
        }

        for kind in [
            BusTransactionKind::Fetch,
            BusTransactionKind::Read,
            BusTransactionKind::Write,
            BusTransactionKind::Input,
            BusTransactionKind::Dma,
            BusTransactionKind::Scanout,
        ] {
            if trace.bus_transactions.iter().any(|event| event.kind == kind) {
                assert!(selected
                    .iter()
                    .any(|index| trace.bus_transactions[*index].kind == kind));
            }
        }

        for port in MMIO_PORTS {
            if trace
                .bus_transactions
                .iter()
                .any(|event| event.address == Some(port.address))
            {
                assert!(selected
                    .iter()
                    .any(|index| trace.bus_transactions[*index].address == Some(port.address)));
            }
        }
    }
}
