#![forbid(unsafe_code)]

use leader_core::{build_navigation, build_topology, CameraView, ExplorerState, NavigationModel, Topology};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Explorer {
    topology: Topology,
    navigation: NavigationModel,
    state: ExplorerState,
}

impl Default for Explorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Explorer {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    #[must_use]
    pub fn new() -> Self {
        let topology = build_topology();
        let navigation = build_navigation(&topology);
        let state = ExplorerState::new(&navigation);
        Self {
            topology,
            navigation,
            state,
        }
    }

    #[must_use]
    pub fn current_view_id(&self) -> String {
        self.state.current_view_id().to_owned()
    }

    #[must_use]
    pub fn can_go_back(&self) -> bool {
        self.state.can_go_back()
    }

    pub fn focus_node(&mut self, node_id: &str) -> bool {
        self.state.focus_node(&self.navigation, node_id)
    }

    pub fn enter_view(&mut self, view_id: &str) -> bool {
        self.state.enter_view(&self.navigation, view_id)
    }

    pub fn back(&mut self) -> bool {
        self.state.back(&self.navigation)
    }

    pub fn parent(&mut self) -> bool {
        self.state.parent(&self.navigation)
    }

    pub fn home(&mut self) -> bool {
        self.state.home(&self.navigation)
    }

    #[must_use]
    pub fn current_view_json(&self) -> String {
        self.state
            .current_view(&self.navigation)
            .map(view_json)
            .unwrap_or_else(|| "null".to_owned())
    }

    #[must_use]
    pub fn breadcrumb_json(&self) -> String {
        views_json(&self.state.breadcrumb(&self.navigation))
    }

    #[must_use]
    pub fn child_views_json(&self) -> String {
        views_json(&self.state.available_children(&self.navigation))
    }

    #[must_use]
    pub fn node_json(&self, node_id: &str) -> String {
        let Some(node) = self.topology.node(node_id) else {
            return "null".to_owned();
        };
        let target = self.navigation.deepest_view_for_node(node_id);
        let path = self.navigation.view_path_for_node(node_id);
        let path_json = path
            .iter()
            .map(|view| format!("\"{}\"", json_escape(&view.id)))
            .collect::<Vec<_>>()
            .join(",");
        let target_json = target.map_or_else(
            || "null".to_owned(),
            |view| format!("\"{}\"", json_escape(&view.id)),
        );
        format!(
            "{{\"id\":\"{}\",\"title\":\"{}\",\"kind\":\"{}\",\"subsystem\":\"{}\",\"targetView\":{},\"viewPath\":[{}],\"bounds\":{{\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1}}}}}",
            json_escape(&node.id),
            json_escape(&node.title),
            json_escape(&node.kind),
            json_escape(&node.group),
            target_json,
            path_json,
            node.bounds.x,
            node.bounds.y,
            node.bounds.w,
            node.bounds.h
        )
    }
}

fn views_json(views: &[&CameraView]) -> String {
    format!(
        "[{}]",
        views.iter().map(|view| view_json(view)).collect::<Vec<_>>().join(",")
    )
}

fn view_json(view: &CameraView) -> String {
    let parent = view.parent_view.as_ref().map_or_else(
        || "null".to_owned(),
        |parent| format!("\"{}\"", json_escape(parent)),
    );
    format!(
        "{{\"id\":\"{}\",\"moduleId\":\"{}\",\"label\":\"{}\",\"parentView\":{},\"density\":\"{}\",\"bounds\":{{\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1}}}}}",
        json_escape(&view.id),
        json_escape(&view.module_id),
        json_escape(&view.label),
        parent,
        view.density.as_str(),
        view.bounds.x,
        view.bounds.y,
        view.bounds.w,
        view.bounds.h
    )
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_starts_at_machine_and_navigates_to_real_detail() {
        let mut explorer = Explorer::new();
        assert_eq!(explorer.current_view_id(), "view-machine");
        assert!(explorer.focus_node("microRom"));
        assert_eq!(explorer.current_view_id(), "view-decode.microcode");
        assert!(explorer.breadcrumb_json().contains("view-decode.microcode"));
        assert!(explorer.back());
        assert_eq!(explorer.current_view_id(), "view-machine");
    }

    #[test]
    fn adapter_exposes_real_node_metadata_and_target_path() {
        let explorer = Explorer::new();
        let node = explorer.node_json("shieldAddr");
        assert!(node.contains("\"subsystem\":\"io\""));
        assert!(node.contains("\"targetView\":\"view-io.shields\""));
        assert!(node.contains("view-machine"));
        assert!(node.contains("view-io.shields"));
        assert_eq!(explorer.node_json("missing"), "null");
    }

    #[test]
    fn direct_view_navigation_preserves_hierarchy_edges() {
        let mut explorer = Explorer::new();
        assert!(!explorer.enter_view("view-alu.ripple"));
        assert!(explorer.enter_view("view-alu"));
        assert!(explorer.enter_view("view-alu.ripple"));
        assert!(explorer.parent());
        assert_eq!(explorer.current_view_id(), "view-alu");
        assert!(explorer.home());
        assert_eq!(explorer.current_view_id(), "view-machine");
    }

    #[test]
    fn current_and_child_view_json_are_frontend_ready() {
        let explorer = Explorer::new();
        let current = explorer.current_view_json();
        assert!(current.contains("\"id\":\"view-machine\""));
        assert!(current.contains("\"bounds\""));
        let children = explorer.child_views_json();
        assert!(children.contains("view-decode"));
        assert!(children.contains("view-gpu"));
    }
}
