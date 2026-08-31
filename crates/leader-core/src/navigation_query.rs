use std::collections::HashSet;

use crate::navigation::{CameraView, NavigationModel};
use crate::topology::{Link, Node, Topology};

impl NavigationModel {
    #[must_use]
    pub fn child_views(&self, view_id: &str) -> Vec<&CameraView> {
        let Some(view) = self.view(view_id) else {
            return Vec::new();
        };
        self.children_of(&view.module_id)
            .into_iter()
            .filter_map(|module| self.view_for_module(&module.id))
            .collect()
    }

    #[must_use]
    pub fn view_path_for_node(&self, node_id: &str) -> Vec<&CameraView> {
        let Some(subsystem) = self.subsystem_for_node(node_id) else {
            return Vec::new();
        };

        let mut path = Vec::with_capacity(3);
        if let Some(root) = self.view(&self.default_view) {
            path.push(root);
        }
        if let Some(view) = self.view_for_module(&subsystem.id) {
            path.push(view);
        }
        if let Some(detail) = self.detail_for_node(node_id) {
            if let Some(view) = self.view_for_module(&detail.id) {
                path.push(view);
            }
        }
        path
    }

    #[must_use]
    pub fn deepest_view_for_node(&self, node_id: &str) -> Option<&CameraView> {
        self.detail_for_node(node_id)
            .and_then(|module| self.view_for_module(&module.id))
            .or_else(|| {
                self.subsystem_for_node(node_id)
                    .and_then(|module| self.view_for_module(&module.id))
            })
    }

    #[must_use]
    pub fn nodes_for_view<'a>(&self, topology: &'a Topology, view_id: &str) -> Vec<&'a Node> {
        let Some(view) = self.view(view_id) else {
            return Vec::new();
        };
        let Some(module) = self.module(&view.module_id) else {
            return Vec::new();
        };
        module
            .node_ids
            .iter()
            .filter_map(|node_id| topology.node(node_id))
            .collect()
    }

    #[must_use]
    pub fn links_for_view<'a>(&self, topology: &'a Topology, view_id: &str) -> Vec<&'a Link> {
        let visible_nodes = self.nodes_for_view(topology, view_id);
        let visible_ids = visible_nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<HashSet<_>>();
        topology
            .links
            .iter()
            .filter(|link| {
                visible_ids.contains(link.from.as_str()) && visible_ids.contains(link.to.as_str())
            })
            .collect()
    }

    #[must_use]
    pub fn node_at_in_view<'a>(
        &self,
        topology: &'a Topology,
        view_id: &str,
        x: f32,
        y: f32,
    ) -> Option<&'a Node> {
        self.nodes_for_view(topology, view_id)
            .into_iter()
            .filter(|node| {
                x >= node.bounds.x
                    && y >= node.bounds.y
                    && x <= node.bounds.x + node.bounds.w
                    && y <= node.bounds.y + node.bounds.h
            })
            .min_by(|left, right| {
                let left_area = left.bounds.w * left.bounds.h;
                let right_area = right.bounds.w * right.bounds.h;
                left_area.total_cmp(&right_area)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_navigation;

    fn ids(views: &[&CameraView]) -> Vec<String> {
        views.iter().map(|view| view.id.clone()).collect()
    }

    #[test]
    fn physical_node_resolves_machine_subsystem_and_deepest_detail() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        assert_eq!(
            ids(&navigation.view_path_for_node("microRom")),
            vec![
                "view-machine".to_owned(),
                "view-decode".to_owned(),
                "view-decode.microcode".to_owned(),
            ]
        );
        assert_eq!(
            ids(&navigation.view_path_for_node("shieldAddr")),
            vec![
                "view-machine".to_owned(),
                "view-io".to_owned(),
                "view-io.shields".to_owned(),
            ]
        );
        assert_eq!(
            navigation
                .deepest_view_for_node("display")
                .map(|view| view.id.as_str()),
            Some("view-gpu.scanout")
        );
    }

    #[test]
    fn node_without_detail_stops_at_its_subsystem() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let node = topology
            .nodes
            .iter()
            .find(|node| navigation.detail_for_node(&node.id).is_none())
            .expect("at least one node without dedicated detail view");
        let path = navigation.view_path_for_node(&node.id);
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].id, "view-machine");
        assert_eq!(path[1].module_id, node.group);
    }

    #[test]
    fn unknown_node_has_no_navigation_target() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        assert!(navigation.view_path_for_node("does-not-exist").is_empty());
        assert!(navigation.deepest_view_for_node("does-not-exist").is_none());
    }

    #[test]
    fn root_view_exposes_every_top_level_subsystem_as_a_child_view() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let children = navigation.child_views(&navigation.default_view);
        assert_eq!(children.len(), topology.groups.len());
        for group in &topology.groups {
            assert!(children.iter().any(|view| view.module_id == group.id));
        }
    }

    #[test]
    fn view_graph_uses_canonical_module_membership() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let nodes = navigation.nodes_for_view(&topology, "view-decode.microcode");
        let node_ids = nodes.iter().map(|node| node.id.as_str()).collect::<HashSet<_>>();
        assert!(node_ids.contains("microAddr"));
        assert!(node_ids.contains("microRom"));
        assert!(!node_ids.contains("shieldAddr"));

        let links = navigation.links_for_view(&topology, "view-decode.microcode");
        assert!(!links.is_empty());
        assert!(links.iter().all(|link| {
            node_ids.contains(link.from.as_str()) && node_ids.contains(link.to.as_str())
        }));
    }

    #[test]
    fn machine_view_contains_the_complete_physical_graph() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        assert_eq!(
            navigation.nodes_for_view(&topology, &navigation.default_view).len(),
            topology.nodes.len()
        );
        assert_eq!(
            navigation.links_for_view(&topology, &navigation.default_view).len(),
            topology.links.len()
        );
    }

    #[test]
    fn hit_testing_is_scoped_to_the_active_canonical_view() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let micro_rom = topology.node("microRom").expect("microRom");
        let x = micro_rom.bounds.x + micro_rom.bounds.w * 0.5;
        let y = micro_rom.bounds.y + micro_rom.bounds.h * 0.5;

        assert_eq!(
            navigation
                .node_at_in_view(&topology, "view-machine", x, y)
                .map(|node| node.id.as_str()),
            Some("microRom")
        );
        assert_eq!(
            navigation
                .node_at_in_view(&topology, "view-decode.microcode", x, y)
                .map(|node| node.id.as_str()),
            Some("microRom")
        );
        assert!(navigation
            .node_at_in_view(&topology, "view-alu", x, y)
            .is_none());
    }
}
