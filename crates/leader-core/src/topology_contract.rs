use std::collections::HashSet;

use crate::{control_topology_violations, Topology};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TopologyValidation {
    pub nodes: usize,
    pub links: usize,
}

pub fn validate_final_topology(topology: &Topology) -> Result<TopologyValidation, String> {
    let control_errors = control_topology_violations(topology);
    if !control_errors.is_empty() {
        return Err(format!(
            "physical control topology invalid: {}",
            control_errors.join(" | ")
        ));
    }

    let mut node_ids = HashSet::with_capacity(topology.nodes.len());
    for node in &topology.nodes {
        if !node_ids.insert(node.id.as_str()) {
            return Err(format!("duplicate topology node id {}", node.id));
        }
        let Some(group) = topology.group(&node.group) else {
            return Err(format!("node {} references missing group {}", node.id, node.group));
        };
        let inside = node.bounds.x >= group.bounds.x
            && node.bounds.y >= group.bounds.y
            && node.bounds.x + node.bounds.w <= group.bounds.x + group.bounds.w
            && node.bounds.y + node.bounds.h <= group.bounds.y + group.bounds.h;
        if !inside {
            return Err(format!(
                "node {} escapes group {}: node=({:.0},{:.0},{:.0},{:.0}) group=({:.0},{:.0},{:.0},{:.0})",
                node.id,
                node.group,
                node.bounds.x,
                node.bounds.y,
                node.bounds.w,
                node.bounds.h,
                group.bounds.x,
                group.bounds.y,
                group.bounds.w,
                group.bounds.h
            ));
        }
    }

    let mut link_ids = HashSet::with_capacity(topology.links.len());
    for link in &topology.links {
        if !link_ids.insert(link.id.as_str()) {
            return Err(format!("duplicate topology link id {}", link.id));
        }
        if topology.node(&link.from).is_none() {
            return Err(format!("link {} references missing source {}", link.id, link.from));
        }
        if topology.node(&link.to).is_none() {
            return Err(format!("link {} references missing target {}", link.id, link.to));
        }
    }

    for required in [
        "microRom", "ctrlMarLoad", "addrLoLatch", "addrHiLatch", "conditionLatch",
        "pcSelectLatch", "regSelectLatch", "returnDataMux", "stackRam", "dmaAddr",
        "dmaData", "xCounter", "yCounter", "pixelMux", "scanShift", "hsync", "vsync",
        "vblankLatch", "vblankWaitGate", "display", "shiftHi", "shiftLo", "shiftOffset",
        "shiftMux", "shiftOut", "formationAlive", "formationDivider", "formationCounter",
        "formationTick", "enemyShotAlloc", "enemyShotCooldown", "enemyShot0X",
        "enemyShot0Y", "enemyShot0Active", "enemyShot1X", "enemyShot1Y",
        "enemyShot1Active", "enemyShot2X", "enemyShot2Y", "enemyShot2Active",
        "shieldAddr", "shieldMask", "shieldWriteEnable", "shieldRam0", "shieldRam1",
        "shieldRam2", "shieldRam3", "shieldVideoMux",
    ] {
        if topology.node(required).is_none() {
            return Err(format!("final topology missing required node {required}"));
        }
    }

    for (from, to) in [
        ("dmaAddr", "arb"),
        ("dataBuf", "dmaData"),
        ("pixelMux", "scanShift"),
        ("scanShift", "display"),
        ("xCounter", "hsync"),
        ("yCounter", "vsync"),
        ("hsync", "display"),
        ("vsync", "display"),
        ("vsync", "vblankLatch"),
        ("reset", "vblankLatch"),
        ("vblankLatch", "vblankWaitGate"),
        ("ctrlWait", "vblankWaitGate"),
        ("vblankWaitGate", "irqLatch"),
    ] {
        require_path(topology, from, to, "video/control")?;
    }

    for bit in 0..8 {
        let rhs_xor = format!("rhsXor{bit}");
        let xor_a = format!("xorA{bit}");
        let xor_b = format!("xorB{bit}");
        let and_a = format!("andA{bit}");
        let and_b = format!("andB{bit}");
        let or_c = format!("orC{bit}");
        let pass_r = format!("passR{bit}");
        let logic_and = format!("logicAnd{bit}");
        let logic_or = format!("logicOr{bit}");
        let mux_r = format!("muxR{bit}");

        for required in [&rhs_xor, &pass_r, &logic_and, &logic_or] {
            if topology.node(required).is_none() {
                return Err(format!("final topology missing required ALU function node {required}"));
            }
        }

        for (from, to, family) in [
            ("readMuxA", xor_a.as_str(), "ALU adder A"),
            ("readMuxB", rhs_xor.as_str(), "ALU RHS input"),
            ("aluSel", rhs_xor.as_str(), "ALU subtraction control"),
            (rhs_xor.as_str(), xor_a.as_str(), "ALU effective RHS sum"),
            ("readMuxA", and_a.as_str(), "ALU generate A"),
            (rhs_xor.as_str(), and_a.as_str(), "ALU effective RHS generate"),
            (xor_a.as_str(), xor_b.as_str(), "ALU sum"),
            (xor_a.as_str(), and_b.as_str(), "ALU propagate"),
            (and_a.as_str(), or_c.as_str(), "ALU carry generate"),
            (and_b.as_str(), or_c.as_str(), "ALU carry propagate"),
            ("readMuxA", pass_r.as_str(), "ALU pass"),
            ("readMuxA", logic_and.as_str(), "ALU logical AND A"),
            ("readMuxB", logic_and.as_str(), "ALU logical AND B"),
            ("readMuxA", logic_or.as_str(), "ALU logical OR A"),
            ("readMuxB", logic_or.as_str(), "ALU logical OR B"),
            (pass_r.as_str(), mux_r.as_str(), "ALU PASS result"),
            (logic_and.as_str(), mux_r.as_str(), "ALU AND result"),
            (logic_or.as_str(), mux_r.as_str(), "ALU OR result"),
            (xor_a.as_str(), mux_r.as_str(), "ALU XOR result"),
            (xor_b.as_str(), mux_r.as_str(), "ALU arithmetic result"),
            ("aluSel", mux_r.as_str(), "ALU selection"),
            (mux_r.as_str(), "writeBus", "ALU writeback"),
        ] {
            require_path(topology, from, to, family)?;
        }
        if bit > 0 {
            let previous_carry = format!("orC{}", bit - 1);
            require_path(topology, &previous_carry, &xor_b, "ALU carry")?;
            require_path(topology, &previous_carry, &and_b, "ALU carry")?;
        }
    }

    for page in 0..32 {
        let node = format!("romPage{page}");
        require_path(topology, "romRowDec", &node, "ROM select")?;
        require_path(topology, &node, "dataBuf", "ROM read")?;
    }
    for page in 0..96 {
        let node = format!("ramPage{page}");
        require_path(topology, "ramPageDec", &node, "RAM select")?;
        require_path(topology, &node, "dataBuf", "RAM read")?;
        require_path(topology, "dataBuf", &node, "RAM write")?;
    }
    for page in 0..8 {
        let node = format!("vramPage{page}");
        require_path(topology, "vramPageDec", &node, "VRAM select")?;
        require_path(topology, &node, "dataBuf", "VRAM read")?;
        require_path(topology, "dataBuf", &node, "VRAM write")?;
    }

    Ok(TopologyValidation {
        nodes: topology.nodes.len(),
        links: topology.links.len(),
    })
}

fn require_path(topology: &Topology, from: &str, to: &str, family: &str) -> Result<(), String> {
    if topology
        .links
        .iter()
        .any(|link| link.from == from && link.to == to)
    {
        Ok(())
    } else {
        Err(format!("final topology missing required {family} path {from} -> {to}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_topology;

    #[test]
    fn final_runtime_topology_is_closed_unique_and_inside_groups() {
        let topology = build_topology();
        let validation = validate_final_topology(&topology).expect("valid final topology");
        assert!(validation.nodes >= 498);
        assert!(validation.links >= 880);
    }

    #[test]
    fn missing_control_consumer_is_detected() {
        let mut topology = build_topology();
        topology.nodes.retain(|node| node.id != "addrLoLatch");
        let error = validate_final_topology(&topology).expect_err("broken topology must fail");
        assert!(error.contains("addrLoLatch") || error.contains("missing target"));
    }

    #[test]
    fn missing_m3_peripheral_is_detected() {
        let mut topology = build_topology();
        topology.nodes.retain(|node| node.id != "shieldRam3");
        let error = validate_final_topology(&topology).expect_err("missing M3 hardware must fail");
        assert!(error.contains("shieldRam3") || error.contains("missing target"));
    }

    #[test]
    fn missing_video_pipeline_hardware_is_detected() {
        let mut topology = build_topology();
        topology.nodes.retain(|node| node.id != "scanShift");
        let error = validate_final_topology(&topology).expect_err("missing video hardware must fail");
        assert!(error.contains("scanShift") || error.contains("missing target"));
    }

    #[test]
    fn missing_video_pipeline_link_is_detected() {
        let mut topology = build_topology();
        topology
            .links
            .retain(|link| !(link.from == "scanShift" && link.to == "display"));
        let error = validate_final_topology(&topology).expect_err("missing video path must fail");
        assert!(error.contains("scanShift -> display"));
    }

    #[test]
    fn missing_vblank_wait_gate_path_is_detected() {
        let mut topology = build_topology();
        topology
            .links
            .retain(|link| !(link.from == "ctrlWait" && link.to == "vblankWaitGate"));
        let error = validate_final_topology(&topology).expect_err("missing VBlank wait gate must fail");
        assert!(error.contains("ctrlWait -> vblankWaitGate"));
    }

    #[test]
    fn missing_alu_internal_link_is_detected() {
        let mut topology = build_topology();
        topology
            .links
            .retain(|link| !(link.from == "andB3" && link.to == "orC3"));
        let error = validate_final_topology(&topology).expect_err("missing ALU path must fail");
        assert!(error.contains("andB3 -> orC3"));
    }

    #[test]
    fn missing_subtraction_conditioning_is_detected() {
        let mut topology = build_topology();
        topology
            .links
            .retain(|link| !(link.from == "aluSel" && link.to == "rhsXor4"));
        let error = validate_final_topology(&topology).expect_err("missing SUB condition path must fail");
        assert!(error.contains("aluSel -> rhsXor4"));
    }

    #[test]
    fn missing_logical_result_path_is_detected() {
        let mut topology = build_topology();
        topology
            .links
            .retain(|link| !(link.from == "logicOr5" && link.to == "muxR5"));
        let error = validate_final_topology(&topology).expect_err("missing logical path must fail");
        assert!(error.contains("logicOr5 -> muxR5"));
    }

    #[test]
    fn missing_memory_page_link_is_detected() {
        let mut topology = build_topology();
        topology
            .links
            .retain(|link| !(link.from == "ramPage37" && link.to == "dataBuf"));
        let error = validate_final_topology(&topology).expect_err("missing RAM path must fail");
        assert!(error.contains("ramPage37 -> dataBuf"));
    }
}
