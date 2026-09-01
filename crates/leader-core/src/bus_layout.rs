use crate::topology::{Link, SignalKind, Topology};

/// Completes the visible system-bus and memory-page wiring.
///
/// Historical README topology intentionally sampled many repeated page wires to
/// keep the static SVG smaller. The live physical graph must be inspectable
/// without that sampling, so this pass materializes every canonical connection.
pub fn inject_system_bus_wiring(topology: &mut Topology) {
    for bit in 0..16 {
        add_path(
            topology,
            &format!("bus-mar-addr-{bit}"),
            &format!("marBit{bit}"),
            "addrBuf",
            SignalKind::Address,
            "A",
        );
    }

    for bit in 0..8 {
        add_path(
            topology,
            &format!("bus-data-mdr-{bit}"),
            "dataBuf",
            &format!("mdrBit{bit}"),
            SignalKind::Data,
            "D",
        );
    }

    add_path(
        topology,
        "bus-write-data",
        "writeBus",
        "dataBuf",
        SignalKind::Data,
        "CPU W",
    );
    add_path(
        topology,
        "bus-arb-address",
        "arb",
        "addrBuf",
        SignalKind::Address,
        "OWNER",
    );

    for decoder in [
        "romRowDec",
        "romByteDec",
        "ramPageDec",
        "ramByteDec",
        "vramPageDec",
        "vramByteDec",
    ] {
        add_path(
            topology,
            &format!("bus-address-{decoder}"),
            "addrBuf",
            decoder,
            SignalKind::Address,
            "A",
        );
    }

    for page in 0..32 {
        let node = format!("romPage{page}");
        add_path(
            topology,
            &format!("rom-cs-full-{page}"),
            "romRowDec",
            &node,
            SignalKind::Control,
            "CS",
        );
        add_path(
            topology,
            &format!("rom-data-full-{page}"),
            &node,
            "dataBuf",
            SignalKind::Data,
            "D",
        );
    }

    for page in 0..96 {
        let node = format!("ramPage{page}");
        add_path(
            topology,
            &format!("ram-cs-full-{page}"),
            "ramPageDec",
            &node,
            SignalKind::Control,
            "CS",
        );
        add_path(
            topology,
            &format!("ram-read-full-{page}"),
            &node,
            "dataBuf",
            SignalKind::Data,
            "READ",
        );
        add_path(
            topology,
            &format!("ram-write-full-{page}"),
            "dataBuf",
            &node,
            SignalKind::Data,
            "WRITE",
        );
    }

    for page in 0..8 {
        let node = format!("vramPage{page}");
        add_path(
            topology,
            &format!("vram-cs-full-{page}"),
            "vramPageDec",
            &node,
            SignalKind::Control,
            "CS",
        );
        add_path(
            topology,
            &format!("vram-read-full-{page}"),
            &node,
            "dataBuf",
            SignalKind::Data,
            "READ",
        );
        add_path(
            topology,
            &format!("vram-write-full-{page}"),
            "dataBuf",
            &node,
            SignalKind::Data,
            "WRITE",
        );
    }

    add_path(
        topology,
        "stack-address-full",
        "addrBuf",
        "stackRam",
        SignalKind::Address,
        "A",
    );
    add_path(
        topology,
        "stack-read-full",
        "stackRam",
        "dataBuf",
        SignalKind::Data,
        "READ",
    );
    add_path(
        topology,
        "stack-write-full",
        "dataBuf",
        "stackRam",
        SignalKind::Data,
        "WRITE",
    );
}

fn add_path(
    topology: &mut Topology,
    id: &str,
    from: &str,
    to: &str,
    signal: SignalKind,
    label: &str,
) {
    if topology
        .links
        .iter()
        .any(|link| link.from == from && link.to == to && link.signal == signal)
    {
        return;
    }
    debug_assert!(topology.node(from).is_some(), "missing bus source node {from}");
    debug_assert!(topology.node(to).is_some(), "missing bus target node {to}");
    topology.links.push(Link {
        id: id.to_owned(),
        from: from.to_owned(),
        to: to.to_owned(),
        signal,
        label: label.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_memory_page_has_complete_read_path() {
        let mut topology = crate::topology::build_topology();
        inject_system_bus_wiring(&mut topology);

        for page in 0..32 {
            let node = format!("romPage{page}");
            assert!(has_path(&topology, "romRowDec", &node, SignalKind::Control));
            assert!(has_path(&topology, &node, "dataBuf", SignalKind::Data));
        }
        for page in 0..96 {
            let node = format!("ramPage{page}");
            assert!(has_path(&topology, "ramPageDec", &node, SignalKind::Control));
            assert!(has_path(&topology, &node, "dataBuf", SignalKind::Data));
            assert!(has_path(&topology, "dataBuf", &node, SignalKind::Data));
        }
        for page in 0..8 {
            let node = format!("vramPage{page}");
            assert!(has_path(&topology, "vramPageDec", &node, SignalKind::Control));
            assert!(has_path(&topology, &node, "dataBuf", SignalKind::Data));
            assert!(has_path(&topology, "dataBuf", &node, SignalKind::Data));
        }
    }

    #[test]
    fn mar_and_mdr_are_physically_connected_to_system_buffers() {
        let mut topology = crate::topology::build_topology();
        inject_system_bus_wiring(&mut topology);
        for bit in 0..16 {
            assert!(has_path(
                &topology,
                &format!("marBit{bit}"),
                "addrBuf",
                SignalKind::Address
            ));
        }
        for bit in 0..8 {
            assert!(has_path(
                &topology,
                "dataBuf",
                &format!("mdrBit{bit}"),
                SignalKind::Data
            ));
        }
    }

    fn has_path(topology: &Topology, from: &str, to: &str, signal: SignalKind) -> bool {
        topology
            .links
            .iter()
            .any(|link| link.from == from && link.to == to && link.signal == signal)
    }
}
