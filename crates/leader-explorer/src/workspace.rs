use std::collections::{BTreeMap, BTreeSet, HashMap};

use leader_core::{build_topology, Rect, Topology};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
struct UserGroup {
    label: String,
    members: BTreeSet<String>,
}

/// Presentation-only workspace state layered above the immutable physical machine.
///
/// This class deliberately owns no CPU/device/memory semantics. Canonical node
/// bounds remain in `leader-core`; workspace positions are stored only as offsets
/// so resetting the editor always recovers the authoritative physical layout.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct WorkspaceLayout {
    topology: Topology,
    offsets: HashMap<String, (f32, f32)>,
    groups: BTreeMap<String, UserGroup>,
    revision: u64,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl WorkspaceLayout {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    #[must_use]
    pub fn new() -> Self {
        Self {
            topology: build_topology(),
            offsets: HashMap::new(),
            groups: BTreeMap::new(),
            revision: 0,
        }
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Sets a presentation offset from the canonical physical node position.
    pub fn set_node_offset(&mut self, node_id: &str, dx: f32, dy: f32) -> bool {
        if self.topology.node(node_id).is_none() || !dx.is_finite() || !dy.is_finite() {
            return false;
        }
        if dx == 0.0 && dy == 0.0 {
            self.offsets.remove(node_id);
        } else {
            self.offsets.insert(node_id.to_owned(), (dx, dy));
        }
        self.bump_revision();
        true
    }

    /// Moves a node to an absolute workspace position while retaining canonical
    /// dimensions and deriving only an offset from the physical layout.
    pub fn move_node_to(&mut self, node_id: &str, x: f32, y: f32) -> bool {
        let Some(node) = self.topology.node(node_id) else {
            return false;
        };
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        self.set_node_offset(node_id, x - node.bounds.x, y - node.bounds.y)
    }

    pub fn reset_node(&mut self, node_id: &str) -> bool {
        if self.topology.node(node_id).is_none() {
            return false;
        }
        let changed = self.offsets.remove(node_id).is_some();
        if changed {
            self.bump_revision();
        }
        changed
    }

    pub fn reset_layout(&mut self) {
        if self.offsets.is_empty() && self.groups.is_empty() {
            return;
        }
        self.offsets.clear();
        self.groups.clear();
        self.bump_revision();
    }

    pub fn create_group(&mut self, group_id: &str, label: &str) -> bool {
        if !valid_group_id(group_id)
            || label.trim().is_empty()
            || self.groups.contains_key(group_id)
            || self.topology.group(group_id).is_some()
        {
            return false;
        }
        self.groups.insert(
            group_id.to_owned(),
            UserGroup {
                label: label.trim().to_owned(),
                members: BTreeSet::new(),
            },
        );
        self.bump_revision();
        true
    }

    pub fn delete_group(&mut self, group_id: &str) -> bool {
        let changed = self.groups.remove(group_id).is_some();
        if changed {
            self.bump_revision();
        }
        changed
    }

    /// A node belongs to at most one user-created group. Physical subsystem
    /// ownership remains untouched and is independent from this editor grouping.
    pub fn add_node_to_group(&mut self, group_id: &str, node_id: &str) -> bool {
        if self.topology.node(node_id).is_none() || !self.groups.contains_key(group_id) {
            return false;
        }
        for group in self.groups.values_mut() {
            group.members.remove(node_id);
        }
        let inserted = self
            .groups
            .get_mut(group_id)
            .is_some_and(|group| group.members.insert(node_id.to_owned()));
        if inserted {
            self.bump_revision();
        }
        inserted
    }

    pub fn remove_node_from_group(&mut self, group_id: &str, node_id: &str) -> bool {
        let changed = self
            .groups
            .get_mut(group_id)
            .is_some_and(|group| group.members.remove(node_id));
        if changed {
            self.bump_revision();
        }
        changed
    }

    /// Applies a presentation delta to every member without changing physical
    /// topology or the machine-owned subsystem grouping.
    pub fn move_group_by(&mut self, group_id: &str, dx: f32, dy: f32) -> bool {
        if !dx.is_finite() || !dy.is_finite() {
            return false;
        }
        let Some(members) = self
            .groups
            .get(group_id)
            .map(|group| group.members.iter().cloned().collect::<Vec<_>>())
        else {
            return false;
        };
        if members.is_empty() {
            return true;
        }
        for node_id in members {
            let (before_x, before_y) = self.offsets.get(&node_id).copied().unwrap_or((0.0, 0.0));
            let next = (before_x + dx, before_y + dy);
            if next == (0.0, 0.0) {
                self.offsets.remove(&node_id);
            } else {
                self.offsets.insert(node_id, next);
            }
        }
        self.bump_revision();
        true
    }

    #[must_use]
    pub fn node_layout_json(&self, node_id: &str) -> String {
        let Some(node) = self.topology.node(node_id) else {
            return "null".to_owned();
        };
        let (dx, dy) = self.offsets.get(node_id).copied().unwrap_or((0.0, 0.0));
        let bounds = translated(node.bounds, dx, dy);
        format!(
            "{{\"id\":\"{}\",\"canonical\":{},\"offset\":{{\"x\":{:.1},\"y\":{:.1}}},\"bounds\":{},\"group\":{}}}",
            json_escape(node_id),
            rect_json(node.bounds),
            dx,
            dy,
            rect_json(bounds),
            self.user_group_json_for_node(node_id)
        )
    }

    #[must_use]
    pub fn snapshot_json(&self) -> String {
        let mut offsets = self.offsets.iter().collect::<Vec<_>>();
        offsets.sort_by(|left, right| left.0.cmp(right.0));
        let offsets = offsets
            .into_iter()
            .map(|(id, (dx, dy))| {
                format!(
                    "{{\"node\":\"{}\",\"dx\":{:.1},\"dy\":{:.1}}}",
                    json_escape(id), dx, dy
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let groups = self
            .groups
            .iter()
            .map(|(id, group)| {
                let members = group
                    .members
                    .iter()
                    .map(|member| format!("\"{}\"", json_escape(member)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"id\":\"{}\",\"label\":\"{}\",\"members\":[{}]}}",
                    json_escape(id),
                    json_escape(&group.label),
                    members
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"revision\":{},\"offsets\":[{}],\"groups\":[{}]}}",
            self.revision, offsets, groups
        )
    }
}

impl WorkspaceLayout {
    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    fn user_group_json_for_node(&self, node_id: &str) -> String {
        self.groups
            .iter()
            .find(|(_, group)| group.members.contains(node_id))
            .map_or_else(
                || "null".to_owned(),
                |(id, _)| format!("\"{}\"", json_escape(id)),
            )
    }
}

fn valid_group_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn translated(rect: Rect, dx: f32, dy: f32) -> Rect {
    Rect::new(rect.x + dx, rect.y + dy, rect.w, rect.h)
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
    fn node_motion_is_an_offset_from_immutable_physical_bounds() {
        let mut workspace = WorkspaceLayout::new();
        let canonical = workspace
            .topology
            .node("microRom")
            .expect("canonical microRom")
            .bounds;
        assert!(workspace.move_node_to("microRom", canonical.x + 40.0, canonical.y - 25.0));
        let json = workspace.node_layout_json("microRom");
        assert!(json.contains("\"offset\":{\"x\":40.0,\"y\":-25.0}"));
        assert_eq!(
            workspace.topology.node("microRom").expect("microRom").bounds,
            canonical,
            "workspace edits must never mutate physical topology"
        );
    }

    #[test]
    fn unknown_nodes_and_invalid_coordinates_are_rejected() {
        let mut workspace = WorkspaceLayout::new();
        assert!(!workspace.move_node_to("missing", 10.0, 10.0));
        assert!(!workspace.set_node_offset("microRom", f32::NAN, 0.0));
        assert!(!workspace.set_node_offset("microRom", 0.0, f32::INFINITY));
        assert_eq!(workspace.revision(), 0);
    }

    #[test]
    fn custom_groups_do_not_replace_physical_subsystems() {
        let mut workspace = WorkspaceLayout::new();
        let physical_group = workspace
            .topology
            .node("microRom")
            .expect("microRom")
            .group
            .clone();
        assert!(workspace.create_group("my-control", "My control cluster"));
        assert!(workspace.add_node_to_group("my-control", "microRom"));
        assert!(workspace.node_layout_json("microRom").contains("\"group\":\"my-control\""));
        assert_eq!(
            workspace.topology.node("microRom").expect("microRom").group,
            physical_group
        );
        assert!(!workspace.create_group("decode", "Cannot shadow physical group"));
    }

    #[test]
    fn assigning_to_another_custom_group_moves_membership_atomically() {
        let mut workspace = WorkspaceLayout::new();
        assert!(workspace.create_group("left", "Left"));
        assert!(workspace.create_group("right", "Right"));
        assert!(workspace.add_node_to_group("left", "flagC"));
        assert!(workspace.add_node_to_group("right", "flagC"));
        let snapshot = workspace.snapshot_json();
        assert!(snapshot.contains("\"id\":\"left\",\"label\":\"Left\",\"members\":[]"));
        assert!(snapshot.contains("\"id\":\"right\",\"label\":\"Right\",\"members\":[\"flagC\"]"));
    }

    #[test]
    fn group_motion_updates_only_presentation_offsets() {
        let mut workspace = WorkspaceLayout::new();
        assert!(workspace.create_group("alu-focus", "ALU focus"));
        assert!(workspace.add_node_to_group("alu-focus", "flagC"));
        assert!(workspace.add_node_to_group("alu-focus", "flagZ"));
        assert!(workspace.move_group_by("alu-focus", 12.0, -8.0));
        assert!(workspace.node_layout_json("flagC").contains("\"x\":12.0,\"y\":-8.0"));
        assert!(workspace.node_layout_json("flagZ").contains("\"x\":12.0,\"y\":-8.0"));
    }

    #[test]
    fn reset_restores_clean_editor_state() {
        let mut workspace = WorkspaceLayout::new();
        assert!(workspace.set_node_offset("flagC", 25.0, 10.0));
        assert!(workspace.create_group("flags", "Flags"));
        assert!(workspace.add_node_to_group("flags", "flagC"));
        workspace.reset_layout();
        let snapshot = workspace.snapshot_json();
        assert!(snapshot.contains("\"offsets\":[]"));
        assert!(snapshot.contains("\"groups\":[]"));
    }
}