use crate::navigation::{CameraView, NavigationModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerState {
    current_view: String,
    history: Vec<String>,
}

impl ExplorerState {
    #[must_use]
    pub fn new(navigation: &NavigationModel) -> Self {
        Self {
            current_view: navigation.default_view.clone(),
            history: Vec::new(),
        }
    }

    #[must_use]
    pub fn current_view_id(&self) -> &str {
        &self.current_view
    }

    #[must_use]
    pub fn current_view<'a>(&self, navigation: &'a NavigationModel) -> Option<&'a CameraView> {
        navigation.view(&self.current_view)
    }

    #[must_use]
    pub fn history(&self) -> &[String] {
        &self.history
    }

    #[must_use]
    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    #[must_use]
    pub fn breadcrumb<'a>(&self, navigation: &'a NavigationModel) -> Vec<&'a CameraView> {
        navigation.lineage_for_view(&self.current_view)
    }

    #[must_use]
    pub fn available_children<'a>(&self, navigation: &'a NavigationModel) -> Vec<&'a CameraView> {
        navigation.child_views(&self.current_view)
    }

    pub fn enter_node(&mut self, navigation: &NavigationModel, node_id: &str) -> bool {
        let Some(target) = navigation.deepest_view_for_node(node_id) else {
            return false;
        };
        self.enter_view(navigation, &target.id)
    }

    pub fn enter_view(&mut self, navigation: &NavigationModel, view_id: &str) -> bool {
        if view_id == self.current_view || navigation.view(view_id).is_none() {
            return false;
        }
        if !is_direct_navigation_edge(navigation, &self.current_view, view_id) {
            return false;
        }
        self.history.push(self.current_view.clone());
        self.current_view = view_id.to_owned();
        true
    }

    pub fn focus_node(&mut self, navigation: &NavigationModel, node_id: &str) -> bool {
        let path = navigation.view_path_for_node(node_id);
        let Some(target) = path.last() else {
            return false;
        };
        if target.id == self.current_view {
            return false;
        }
        self.history.push(self.current_view.clone());
        self.current_view = target.id.clone();
        true
    }

    pub fn back(&mut self, navigation: &NavigationModel) -> bool {
        while let Some(previous) = self.history.pop() {
            if navigation.view(&previous).is_some() {
                self.current_view = previous;
                return true;
            }
        }
        false
    }

    pub fn parent(&mut self, navigation: &NavigationModel) -> bool {
        let Some(parent) = self
            .current_view(navigation)
            .and_then(|view| view.parent_view.clone())
        else {
            return false;
        };
        self.history.push(self.current_view.clone());
        self.current_view = parent;
        true
    }

    pub fn home(&mut self, navigation: &NavigationModel) -> bool {
        if self.current_view == navigation.default_view {
            return false;
        }
        self.history.push(self.current_view.clone());
        self.current_view = navigation.default_view.clone();
        true
    }
}

fn is_direct_navigation_edge(navigation: &NavigationModel, from: &str, to: &str) -> bool {
    let Some(target) = navigation.view(to) else {
        return false;
    };
    target.parent_view.as_deref() == Some(from)
        || navigation
            .view(from)
            .and_then(|view| view.parent_view.as_deref())
            == Some(to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_navigation;

    #[test]
    fn click_navigation_enters_node_detail_and_back_restores_previous_view() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let mut state = ExplorerState::new(&navigation);

        assert!(state.focus_node(&navigation, "microRom"));
        assert_eq!(state.current_view_id(), "view-decode.microcode");
        assert_eq!(
            state
                .breadcrumb(&navigation)
                .iter()
                .map(|view| view.id.as_str())
                .collect::<Vec<_>>(),
            vec!["view-machine", "view-decode", "view-decode.microcode"]
        );
        assert!(state.back(&navigation));
        assert_eq!(state.current_view_id(), "view-machine");
    }

    #[test]
    fn direct_enter_enforces_parent_child_navigation_edges() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let mut state = ExplorerState::new(&navigation);

        assert!(!state.enter_view(&navigation, "view-decode.microcode"));
        assert!(state.enter_view(&navigation, "view-decode"));
        assert!(state.enter_view(&navigation, "view-decode.microcode"));
        assert!(!state.enter_view(&navigation, "view-alu.ripple"));
        assert!(state.parent(&navigation));
        assert_eq!(state.current_view_id(), "view-decode");
    }

    #[test]
    fn node_enter_uses_deepest_view_when_it_is_a_direct_child() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let mut state = ExplorerState::new(&navigation);

        assert!(state.enter_view(&navigation, "view-gpu"));
        assert!(state.enter_node(&navigation, "display"));
        assert_eq!(state.current_view_id(), "view-gpu.scanout");
    }

    #[test]
    fn home_parent_and_children_are_consistent() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let mut state = ExplorerState::new(&navigation);

        let root_children = state.available_children(&navigation);
        assert_eq!(root_children.len(), topology.groups.len());

        assert!(state.enter_view(&navigation, "view-io"));
        let io_children = state.available_children(&navigation);
        assert!(io_children.iter().any(|view| view.id == "view-io.shields"));
        assert!(state.home(&navigation));
        assert_eq!(state.current_view_id(), navigation.default_view);
        assert!(state.back(&navigation));
        assert_eq!(state.current_view_id(), "view-io");
    }

    #[test]
    fn unknown_node_or_view_does_not_mutate_state() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let mut state = ExplorerState::new(&navigation);
        let before = state.clone();

        assert!(!state.focus_node(&navigation, "missing"));
        assert!(!state.enter_view(&navigation, "view-missing"));
        assert_eq!(state, before);
    }
}
