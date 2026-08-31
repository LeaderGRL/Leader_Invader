use crate::navigation::{CameraView, NavigationModel};

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
}
