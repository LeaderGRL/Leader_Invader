use std::collections::HashSet;

use crate::topology::{Rect, Topology};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationLevel {
    Machine,
    Subsystem,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailDensity {
    Overview,
    Native,
    BitExact,
}

impl DetailDensity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Native => "native",
            Self::BitExact => "bit_exact",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub id: String,
    pub label: String,
    pub parent: Option<String>,
    pub bounds: Rect,
    pub node_ids: Vec<String>,
    pub child_modules: Vec<String>,
    pub assembly_rank: usize,
    pub level: NavigationLevel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraView {
    pub id: String,
    pub module_id: String,
    pub label: String,
    pub bounds: Rect,
    pub parent_view: Option<String>,
    pub level: NavigationLevel,
    pub density: DetailDensity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationModel {
    pub default_view: String,
    pub modules: Vec<Module>,
    pub views: Vec<CameraView>,
}

impl NavigationModel {
    #[must_use]
    pub fn module(&self, id: &str) -> Option<&Module> {
        self.modules.iter().find(|module| module.id == id)
    }

    #[must_use]
    pub fn view(&self, id: &str) -> Option<&CameraView> {
        self.views.iter().find(|view| view.id == id)
    }

    #[must_use]
    pub fn view_for_module(&self, module_id: &str) -> Option<&CameraView> {
        self.views.iter().find(|view| view.module_id == module_id)
    }

    #[must_use]
    pub fn children_of(&self, module_id: &str) -> Vec<&Module> {
        self.module(module_id)
            .map(|module| {
                module
                    .child_modules
                    .iter()
                    .filter_map(|child| self.module(child))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn lineage_for_view(&self, view_id: &str) -> Vec<&CameraView> {
        let mut lineage = Vec::new();
        let mut current = self.view(view_id);
        while let Some(view) = current {
            lineage.push(view);
            current = view.parent_view.as_deref().and_then(|parent| self.view(parent));
        }
        lineage.reverse();
        lineage
    }
}

#[must_use]
pub fn build_navigation(topology: &Topology) -> NavigationModel {
    let root_id = "machine".to_owned();
    let mut modules = vec![Module {
        id: root_id.clone(),
        label: "LEADER MACHINE".to_owned(),
        parent: None,
        bounds: Rect::new(0.0, 0.0, topology.width, topology.height),
        node_ids: topology.nodes.iter().map(|node| node.id.clone()).collect(),
        child_modules: Vec::new(),
        assembly_rank: 0,
        level: NavigationLevel::Machine,
    }];

    let mut groups = topology.groups.iter().collect::<Vec<_>>();
    groups.sort_by_key(|group| group.assembly_rank);
    for group in groups {
        let mut node_ids = topology
            .nodes
            .iter()
            .filter(|node| node.group == group.id)
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        node_ids.sort();
        modules[0].child_modules.push(group.id.clone());
        modules.push(Module {
            id: group.id.clone(),
            label: group.label.clone(),
            parent: Some(root_id.clone()),
            bounds: group.bounds,
            node_ids,
            child_modules: Vec::new(),
            assembly_rank: group.assembly_rank,
            level: NavigationLevel::Subsystem,
        });
    }

    push_detail_module(topology, &mut modules, "clk.phases", "CLOCK / PHASE CHAIN", "clk", |id| {
        matches!(id, "clock" | "reset" | "clkGate") || id.starts_with("phase")
    });
    push_detail_module(topology, &mut modules, "pc.fetch", "PC / MAR FETCH PATH", "pc", |id| {
        id.starts_with("pcBit")
            || id.starts_with("marBit")
            || id.starts_with("pcMux")
            || id.starts_with("pcInc")
            || id == "pcCarry"
    });
    push_detail_module(
        topology,
        &mut modules,
        "decode.instruction",
        "IR / OPCODE DECODE",
        "decode",
        |id| {
            id.starts_with("mdrBit")
                || id.starts_with("irBit")
                || matches!(id, "opHi" | "opLo" | "decA" | "decB")
                || id.starts_with("decA")
                || id.starts_with("decB")
        },
    );
    push_detail_module(
        topology,
        &mut modules,
        "decode.microcode",
        "MICROCODE / CONTROL ROM",
        "decode",
        |id| {
            id == "microAddr"
                || id == "microRom"
                || id.starts_with("microAddrBit")
                || id.starts_with("ctrl")
        },
    );
    push_detail_module(
        topology,
        &mut modules,
        "regs.readwrite",
        "REGISTER READ / WRITEBACK",
        "regs",
        |id| id.starts_with("reg") || id.starts_with("readMux") || matches!(id, "writeDec" | "writeBus"),
    );
    push_detail_module(topology, &mut modules, "alu.ripple", "RIPPLE ALU / FLAGS", "alu", |id| {
        id == "aluSel"
            || id.starts_with("xor")
            || id.starts_with("and")
            || id.starts_with("orC")
            || id.starts_with("muxR")
            || id.starts_with("flag")
    });
    push_detail_module(
        topology,
        &mut modules,
        "romsys.decode",
        "ROM ADDRESS DECODE",
        "romsys",
        |id| matches!(id, "romRowDec" | "romByteDec"),
    );
    push_detail_module(topology, &mut modules, "romsys.pages", "ROM PAGE ARRAY", "romsys", |id| {
        id.starts_with("romPage")
    });
    push_detail_module(
        topology,
        &mut modules,
        "ramsys.decode",
        "RAM ADDRESS DECODE",
        "ramsys",
        |id| matches!(id, "ramPageDec" | "ramByteDec"),
    );
    push_detail_module(topology, &mut modules, "ramsys.pages", "RAM PAGE ARRAY", "ramsys", |id| {
        id.starts_with("ramPage")
    });
    push_detail_module(
        topology,
        &mut modules,
        "bus.arbitration",
        "SYSTEM BUS ARBITRATION",
        "bus",
        |id| matches!(id, "addrBuf" | "dataBuf" | "ctrlBuf" | "arb"),
    );
    push_detail_module(topology, &mut modules, "bus.stack", "STACK / SP RIPPLE PATH", "bus", |id| {
        id.starts_with("spBit") || matches!(id, "spDec" | "spBorrow" | "spInc" | "stackRam")
    });
    push_detail_module(
        topology,
        &mut modules,
        "vramsys.decode",
        "VRAM ADDRESS DECODE",
        "vramsys",
        |id| matches!(id, "vramPageDec" | "vramByteDec"),
    );
    push_detail_module(topology, &mut modules, "vramsys.pages", "VRAM PAGE ARRAY", "vramsys", |id| {
        id.starts_with("vramPage")
    });
    push_detail_module(topology, &mut modules, "io.input_irq", "INPUT / TIMER / IRQ", "io", |id| {
        matches!(id, "kbd" | "inputLatch" | "timer" | "irqAnd" | "irqLatch")
    });
    push_detail_module(topology, &mut modules, "io.shift_register", "16-BIT SHIFT REGISTER", "io", |id| {
        id.starts_with("shift")
    });
    push_detail_module(topology, &mut modules, "io.formation", "FORMATION CADENCE", "io", |id| {
        id.starts_with("formation")
    });
    push_detail_module(topology, &mut modules, "io.enemy_shots", "ENEMY SHOT BANK", "io", |id| {
        id.starts_with("enemyShot")
    });
    push_detail_module(topology, &mut modules, "io.shields", "BIT-ADDRESSED SHIELDS", "io", |id| {
        id.starts_with("shield")
    });
    push_detail_module(topology, &mut modules, "gpu.dma", "VIDEO DMA", "gpu", |id| {
        matches!(id, "dmaAddr" | "dmaData")
    });
    push_detail_module(topology, &mut modules, "gpu.scanout", "PIXEL SCANOUT", "gpu", |id| {
        matches!(id, "spriteRom" | "pixelMux" | "scanShift" | "display")
    });
    push_detail_module(topology, &mut modules, "gpu.timing", "VIDEO TIMING / VBLANK", "gpu", |id| {
        matches!(
            id,
            "xCounter" | "yCounter" | "hsync" | "vsync" | "vblankLatch" | "vblankWaitGate"
        )
    });

    let views = modules
        .iter()
        .map(|module| {
            let (padding, density) = match module.level {
                NavigationLevel::Machine => (0.0, DetailDensity::Overview),
                NavigationLevel::Subsystem => (44.0, DetailDensity::Native),
                NavigationLevel::Detail => (24.0, DetailDensity::BitExact),
            };
            CameraView {
                id: view_id(&module.id),
                module_id: module.id.clone(),
                label: module.label.clone(),
                bounds: module.bounds.padded(padding),
                parent_view: module.parent.as_deref().map(view_id),
                level: module.level,
                density,
            }
        })
        .collect();

    NavigationModel {
        default_view: view_id("machine"),
        modules,
        views,
    }
}

fn push_detail_module<F>(
    topology: &Topology,
    modules: &mut Vec<Module>,
    id: &str,
    label: &str,
    parent: &str,
    include: F,
) where
    F: Fn(&str) -> bool,
{
    let mut nodes = topology
        .nodes
        .iter()
        .filter(|node| node.group == parent && include(&node.id))
        .collect::<Vec<_>>();
    if nodes.is_empty() {
        return;
    }
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let bounds = nodes
        .iter()
        .skip(1)
        .fold(nodes[0].bounds, |bounds, node| union(bounds, node.bounds));
    let node_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
    let Some(parent_index) = modules.iter().position(|module| module.id == parent) else {
        return;
    };
    let assembly_rank = modules[parent_index].assembly_rank;
    modules[parent_index].child_modules.push(id.to_owned());
    modules.push(Module {
        id: id.to_owned(),
        label: label.to_owned(),
        parent: Some(parent.to_owned()),
        bounds,
        node_ids,
        child_modules: Vec::new(),
        assembly_rank,
        level: NavigationLevel::Detail,
    });
}

fn union(left: Rect, right: Rect) -> Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = (left.x + left.w).max(right.x + right.w);
    let bottom = (left.y + left.h).max(right.y + right.h);
    Rect::new(x, y, right_edge - x, bottom - y)
}

fn view_id(module_id: &str) -> String {
    format!("view-{module_id}")
}

#[must_use]
pub fn navigation_violations(topology: &Topology, navigation: &NavigationModel) -> Vec<String> {
    let mut errors = Vec::new();
    let mut module_ids = HashSet::new();
    let mut view_ids = HashSet::new();

    if navigation.view(&navigation.default_view).is_none() {
        errors.push(format!("missing default view {}", navigation.default_view));
    }

    for module in &navigation.modules {
        if !module_ids.insert(module.id.as_str()) {
            errors.push(format!("duplicate module {}", module.id));
        }
        if let Some(parent) = &module.parent {
            if navigation.module(parent).is_none() {
                errors.push(format!("module {} references missing parent {parent}", module.id));
            }
        }
        for node_id in &module.node_ids {
            let Some(node) = topology.node(node_id) else {
                errors.push(format!("module {} references missing node {node_id}", module.id));
                continue;
            };
            if !contains(module.bounds, node.bounds) {
                errors.push(format!("module {} does not contain node {node_id}", module.id));
            }
        }
        for child_id in &module.child_modules {
            let Some(child) = navigation.module(child_id) else {
                errors.push(format!("module {} references missing child {child_id}", module.id));
                continue;
            };
            if child.parent.as_deref() != Some(module.id.as_str()) {
                errors.push(format!("module {child_id} is not parented by {}", module.id));
            }
        }
    }

    for view in &navigation.views {
        if !view_ids.insert(view.id.as_str()) {
            errors.push(format!("duplicate view {}", view.id));
        }
        if navigation.module(&view.module_id).is_none() {
            errors.push(format!("view {} references missing module {}", view.id, view.module_id));
        }
        if let Some(parent_view) = &view.parent_view {
            if navigation.view(parent_view).is_none() {
                errors.push(format!("view {} references missing parent view {parent_view}", view.id));
            }
        }
    }

    errors
}

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.w <= outer.x + outer.w
        && inner.y + inner.h <= outer.y + outer.h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_covers_every_top_level_subsystem_and_node() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let root = navigation.module("machine").expect("machine root");
        assert_eq!(root.child_modules.len(), topology.groups.len());
        for group in &topology.groups {
            let module = navigation.module(&group.id).expect("subsystem module");
            assert_eq!(module.parent.as_deref(), Some("machine"));
            for node in topology.nodes.iter().filter(|node| node.group == group.id) {
                assert!(module.node_ids.contains(&node.id));
            }
        }
    }

    #[test]
    fn dense_cpu_memory_and_video_regions_have_detail_views() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        for id in [
            "pc.fetch",
            "decode.instruction",
            "decode.microcode",
            "regs.readwrite",
            "alu.ripple",
            "romsys.pages",
            "ramsys.pages",
            "bus.stack",
            "vramsys.pages",
            "gpu.dma",
            "gpu.scanout",
            "gpu.timing",
        ] {
            let module = navigation.module(id).expect("detail module");
            assert_eq!(module.level, NavigationLevel::Detail);
            assert!(!module.node_ids.is_empty());
            let view = navigation.view_for_module(id).expect("detail view");
            assert_eq!(view.density, DetailDensity::BitExact);
        }
    }

    #[test]
    fn m3_hardware_has_first_class_navigation_modules() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        for id in ["io.shift_register", "io.formation", "io.enemy_shots", "io.shields"] {
            let module = navigation.module(id).expect("M3 detail module");
            assert_eq!(module.parent.as_deref(), Some("io"));
            assert!(!module.node_ids.is_empty());
        }
    }

    #[test]
    fn detail_lineage_walks_machine_to_subsystem_to_detail() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let view = navigation.view_for_module("gpu.timing").expect("timing view");
        let lineage = navigation.lineage_for_view(&view.id);
        let ids = lineage.iter().map(|entry| entry.module_id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids, vec!["machine", "gpu", "gpu.timing"]);
    }

    #[test]
    fn generated_navigation_is_closed_over_the_physical_topology() {
        let topology = crate::build_topology();
        let navigation = build_navigation(&topology);
        let violations = navigation_violations(&topology, &navigation);
        assert!(violations.is_empty(), "{}", violations.join("\n"));
    }
}
