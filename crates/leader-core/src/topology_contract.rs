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
    ] {
        if topology.node(required).is_none() {
            return Err(format!("final topology missing required node {required}"));
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
        assert!(validation.nodes >= 439);
        assert!(validation.links >= 484);
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
        topology.nodes.retain(|node| node.id != "formationTick");
        let error = validate_final_topology(&topology).expect_err("missing M3 hardware must fail");
        assert!(error.contains("formationTick") || error.contains("missing source"));
    }
}
