#![forbid(unsafe_code)]

use leader_core::{
    build_navigation, build_topology, CameraView, ExplorerState, Link, NavigationModel, Node,
    Topology,
};

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

    pub fn focus_at(&mut self, x: f32, y: f32) -> bool {
        let Some(node_id) = self
            .navigation
            .node_at_in_view(&self.topology, self.state.current_view_id(), x, y)
            .map(|node| node.id.clone())
        else {
            return false;
        };
        self.state.focus_node(&self.navigation, &node_id)
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
    pub fn topology_json(&self) -> String {
        graph_json(
            self.topology.width,
            self.topology.height,
            self.topology.nodes.iter().collect(),
            self.topology.links.iter().collect(),
        )
    }

    #[must_use]
    pub fn current_view_graph_json(&self) -> String {
        let view_id = self.state.current_view_id();
        graph_json(
            self.topology.width,
            self.topology.height,
            self.navigation.nodes_for_view(&self.topology, view_id),
            self.navigation.links_for_view(&self.topology, view_id),
        )
    }

    #[must_use]
    pub fn node_at_json(&self, x: f32, y: f32) -> String {
        self.navigation
            .node_at_in_view(&self.topology, self.state.current_view_id(), x, y)
            .map(|node| node_json(node, &self.navigation))
            .unwrap_or_else(|| "null".to_owned())
    }

    #[must_use]
    pub fn node_json(&self, node_id: &str) -> String {
        self.topology
            .node(node_id)
            .map(|node| node_json(node, &self.navigation))
            .unwrap_or_else(|| "null".to_owned())
    }
}

fn graph_json(width: f32, height: f32, nodes: Vec<&Node>, links: Vec<&Link>) -> String {
    let nodes = nodes
        .iter()
        .map(|node| simple_node_json(node))
        .collect::<Vec<_>>()
        .join(",");
    let links = links
        .iter()
        .map(|link| link_json(link))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"width\":{width:.1},\"height\":{height:.1},\"nodes\":[{nodes}],\"links\":[{links}]}}"
    )
}

fn simple_node_json(node: &Node) -> String {
    format!(
        "{{\"id\":\"{}\",\"title\":\"{}\",\"kind\":\"{}\",\"subsystem\":\"{}\",\"bounds\":{{\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1}}}}}",
        json_escape(&node.id),
        json_escape(&node.title),
        json_escape(&node.kind),
        json_escape(&node.group),
        node.bounds.x,
        node.bounds.y,
        node.bounds.w,
        node.bounds.h
    )
}

fn node_json(node: &Node, navigation: &NavigationModel) -> String {
    let target = navigation.deepest_view_for_node(&node.id);
    let path = navigation.view_path_for_node(&node.id);
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

fn link_json(link: &Link) -> String {
    format!(
        "{{\"id\":\"{}\",\"from\":\"{}\",\"to\":\"{}\",\"signal\":\"{}\",\"label\":\"{}\"}}",
        json_escape(&link.id),
        json_escape(&link.from),
        json_escape(&link.to),
        link.signal.css_class(),
        json_escape(&link.label)
    )
}

fn views_json(views: &[&CameraView]) -> String {
    format!(
        "[{}]",
        views
            .iter()
            .map(|view| view_json(view))
            .collect::<Vec<_>>()
            .join(",")
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

    #[test]
    fn adapter_exposes_complete_and_view_scoped_physical_graphs() {
        let mut explorer = Explorer::new();
        let topology = explorer.topology_json();
        assert!(topology.contains("\"id\":\"microRom\""));
        assert!(topology.contains("\"id\":\"shieldAddr\""));
        assert!(topology.contains("\"signal\":\"control\""));

        assert!(explorer.focus_node("microRom"));
        let detail = explorer.current_view_graph_json();
        assert!(detail.contains("\"id\":\"microRom\""));
        assert!(detail.contains("\"id\":\"microAddr\""));
        assert!(!detail.contains("\"id\":\"shieldAddr\""));
    }

    #[test]
    fn adapter_hit_testing_and_focus_use_real_node_bounds() {
        let mut explorer = Explorer::new();
        let micro_rom = explorer.topology.node("microRom").expect("microRom");
        let x = micro_rom.bounds.x + micro_rom.bounds.w * 0.5;
        let y = micro_rom.bounds.y + micro_rom.bounds.h * 0.5;

        let hit = explorer.node_at_json(x, y);
        assert!(hit.contains("\"id\":\"microRom\""));
        assert!(explorer.focus_at(x, y));
        assert_eq!(explorer.current_view_id(), "view-decode.microcode");
        assert!(!explorer.focus_at(-10.0, -10.0));
    }
}
