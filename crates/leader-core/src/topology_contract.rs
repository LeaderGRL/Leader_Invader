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
        "microRom",
        "ctrlMarLoad",
        "addrLoLatch",
        "addrHiLatch",
        "conditionLatch",
        "pcSelectLatch",
        "regSelectLatch",
        "returnDataMux",
        "stackRam",
        "dmaAddr",
        "dmaData",
        "xCounter",
        "yCounter",
        "pixelMux",
        "scanShift",
        "hsync",
        "vsync",
        "display",
        "shiftHi",
        "shiftLo",
        "shiftOffset",
        "shiftMux",
        "shiftOut",
        "formationAlive",
        "formationDivider",
        "formationCounter",
        "formationTick",
        "enemyShotAlloc",
        "enemyShotCooldown",
        "enemyShot0X",
        "enemyShot0Y",
        "enemyShot0Active",
        "enemyShot1X",
        "enemyShot1Y",
        "enemyShot1Active",
        "enemyShot2X",
        "enemyShot2Y",
        "enemyShot2Active",
        "shieldAddr",
        "shieldMask",
        "shieldWriteEnable",
        "shieldRam0",
        "shieldRam1",
        "shieldRam2",
        "shieldRam3",
        "shieldVideoMux",
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
    ] {
        if !topology
            .links
            .iter()
            .any(|link| link.from == from && link.to == to)
        {
            return Err(format!(
                "final topology missing required video path {from} -> {to}"
            ));
        }
    }

    Ok(TopologyValidation {
        nodes: topology.nodes.len(),
        links: topology.links.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_topology;

    #[test]
    fn final_runtime_topology_is_closed_unique_and_inside_groups() {
        let topology = build_topology();
        let validation = validate_final_topology(&topology).expect("valid final topology");
        assert!(validation.nodes >= 458);
        assert!(validation.links >= 520);
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
}
