use std::collections::HashSet;

use crate::{control_topology_violations, layout::layout_violations, Topology};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TopologyValidation {
    pub nodes: usize,
    pub links: usize,
}

pub fn validate_final_topology(topology: &Topology) -> Result<TopologyValidation, String> {
    let layout_errors = layout_violations(topology);
    if !layout_errors.is_empty() {
        return Err(format!("final topology layout invalid: {}", layout_errors.join(" | ")));
    }

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
    ] {
        if topology.node(required).is_none() {
            return Err(format!("final F3 topology missing required node {required}"));
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
        assert!(validation.nodes >= 430);
        assert!(validation.links >= 470);
    }

    #[test]
    fn missing_control_consumer_is_detected() {
        let mut topology = build_topology();
        topology.nodes.retain(|node| node.id != "addrLoLatch");
        let error = validate_final_topology(&topology).expect_err("broken topology must fail");
        assert!(error.contains("addrLoLatch") || error.contains("missing target"));
    }
}
