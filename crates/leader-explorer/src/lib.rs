#![forbid(unsafe_code)]

pub mod playback;
mod viewport;

use leader_core::{
    build_navigation, build_topology, CameraView, ExplorerState, Link, NavigationModel, Node, Rect,
    Topology,
};
use viewport::ViewportState;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Explorer {
    topology: Topology,
    navigation: NavigationModel,
    state: ExplorerState,
    viewport: ViewportState,
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
        let world = Rect::new(0.0, 0.0, topology.width, topology.height);
        let initial = state
            .current_view(&navigation)
            .map_or(world, |view| view.bounds);
        let viewport = ViewportState::new(world, initial);
        Self {
            topology,
            navigation,
            state,
            viewport,
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
        let changed = self.state.focus_node(&self.navigation, node_id);
        if changed {
            self.fit_camera_to_current_view();
        }
        changed
    }

    pub fn focus_at(&mut self, x: f32, y: f32) -> bool {
        let Some(node_id) = self
            .navigation
            .node_at_in_view(&self.topology, self.state.current_view_id(), x, y)
            .map(|node| node.id.clone())
        else {
            return false;
        };
        self.focus_node(&node_id)
    }

    pub fn enter_view(&mut self, view_id: &str) -> bool {
        let changed = self.state.enter_view(&self.navigation, view_id);
        if changed {
            self.fit_camera_to_current_view();
        }
        changed
    }

    pub fn back(&mut self) -> bool {
        let changed = self.state.back(&self.navigation);
        if changed {
            self.fit_camera_to_current_view();
        }
        changed
    }

    pub fn parent(&mut self) -> bool {
        let changed = self.state.parent(&self.navigation);
        if changed {
            self.fit_camera_to_current_view();
        }
        changed
    }

    pub fn home(&mut self) -> bool {
        let changed = self.state.home(&self.navigation);
        if changed {
            self.fit_camera_to_current_view();
        }
        changed
    }

    pub fn pan_camera(&mut self, dx: f32, dy: f32) {
        self.viewport.pan(dx, dy);
    }

    pub fn zoom_camera_at(&mut self, anchor_x: f32, anchor_y: f32, factor: f32) -> bool {
        self.viewport.zoom_at(anchor_x, anchor_y, factor)
    }

    pub fn fit_current_view(&mut self) {
        self.fit_camera_to_current_view();
    }

    #[must_use]
    pub fn camera_json(&self) -> String {
        rect_json(self.viewport.camera())
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

impl Explorer {
    fn fit_camera_to_current_view(&mut self) {
        if let Some(bounds) = self
            .state
            .current_view(&self.navigation)
            .map(|view| view.bounds)
        {
            self.viewport.fit(bounds);
        }
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
        "{{\"id\":\"{}\",\"title\":\"{}\",\"kind\":\"{}\",\"subsystem\":\"{}\",\"bounds\":{}}}",
        json_escape(&node.id),
        json_escape(&node.title),
        json_escape(&node.kind),
        json_escape(&node.group),
        rect_json(node.bounds)
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
        "{{\"id\":\"{}\",\"title\":\"{}\",\"kind\":\"{}\",\"subsystem\":\"{}\",\"targetView\":{},\"viewPath\":[{}],\"bounds\":{}}}",
        json_escape(&node.id),
        json_escape(&node.title),
        json_escape(&node.kind),
        json_escape(&node.group),
        target_json,
        path_json,
        rect_json(node.bounds)
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
        "{{\"id\":\"{}\",\"moduleId\":\"{}\",\"label\":\"{}\",\"parentView\":{},\"density\":\"{}\",\"bounds\":{}}}",
        json_escape(&view.id),
        json_escape(&view.module_id),
        json_escape(&view.label),
        parent,
        view.density.as_str(),
        rect_json(view.bounds)
    )
}

fn rect_json(rect: Rect) -> String {
    format!(
        "{{\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1}}}",
        rect.x, rect.y, rect.w, rect.h
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
        assert!(explorer
            .breadcrumb_json()
            .contains("view-decode.microcode"));
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

    #[test]
    fn navigation_refits_camera_while_manual_pan_and_zoom_remain_local() {
        let mut explorer = Explorer::new();
        let root_camera = explorer.camera_json();
        assert_eq!(
            root_camera,
            "{\"x\":0.0,\"y\":0.0,\"w\":7000.0,\"h\":3720.0}"
        );

        assert!(explorer.focus_node("microRom"));
        let fitted = explorer.camera_json();
        assert_ne!(fitted, root_camera);
        assert_eq!(explorer.current_view_id(), "view-decode.microcode");

        let current_view = explorer
            .state
            .current_view(&explorer.navigation)
            .expect("current view");
        assert_eq!(explorer.viewport.camera(), current_view.bounds);

        let center_x = current_view.bounds.x + current_view.bounds.w * 0.5;
        let center_y = current_view.bounds.y + current_view.bounds.h * 0.5;
        assert!(explorer.zoom_camera_at(center_x, center_y, 2.0));
        assert_ne!(explorer.camera_json(), fitted);
        explorer.pan_camera(10.0, 10.0);
        assert_ne!(explorer.camera_json(), fitted);
        explorer.fit_current_view();
        assert_eq!(explorer.camera_json(), fitted);

        assert!(explorer.back());
        assert_eq!(explorer.camera_json(), root_camera);
    }
}
