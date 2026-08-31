use leader_core::{activity::physical_activity_links, build_topology, PhaseKind, SignalKind, Topology};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct ActivityResolver {
    topology: Topology,
}

impl Default for ActivityResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl ActivityResolver {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    #[must_use]
    pub fn new() -> Self {
        Self {
            topology: build_topology(),
        }
    }

    #[must_use]
    pub fn links_json(&self, phase: &str, address: i32, data: i32) -> String {
        let Some(phase) = parse_phase(phase) else {
            return "[]".to_owned();
        };
        let address = optional_u16(address);
        let data = optional_u8(data);
        let links = physical_activity_links(&self.topology, phase, address)
            .into_iter()
            .map(|link| {
                let (value, width) = signal_value(link.signal, address, data);
                format!(
                    "{{\"id\":\"{}\",\"signal\":\"{}\",\"value\":{},\"width\":{}}}",
                    json_escape(&link.id),
                    link.signal.css_class(),
                    value.map_or_else(|| "null".to_owned(), |value| value.to_string()),
                    width.map_or_else(|| "null".to_owned(), |width| width.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{links}]")
    }
}

fn parse_phase(value: &str) -> Option<PhaseKind> {
    match value {
        "fetch" => Some(PhaseKind::Fetch),
        "decode" => Some(PhaseKind::Decode),
        "input" => Some(PhaseKind::Input),
        "memory_read" => Some(PhaseKind::MemoryRead),
        "alu" => Some(PhaseKind::Alu),
        "memory_write" => Some(PhaseKind::MemoryWrite),
        "dma" => Some(PhaseKind::Dma),
        "scanout" => Some(PhaseKind::Scanout),
        "vblank" => Some(PhaseKind::VBlank),
        _ => None,
    }
}

fn optional_u16(value: i32) -> Option<u16> {
    u16::try_from(value).ok()
}

fn optional_u8(value: i32) -> Option<u8> {
    u8::try_from(value).ok()
}

fn signal_value(
    signal: SignalKind,
    address: Option<u16>,
    data: Option<u8>,
) -> (Option<u16>, Option<u8>) {
    match signal {
        SignalKind::Address => (address, Some(16)),
        SignalKind::Data => (data.map(u16::from), Some(8)),
        _ => (None, None),
    }
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
    fn resolver_returns_only_canonical_activity_links() {
        let resolver = ActivityResolver::new();
        let json = resolver.links_json("scanout", -1, 0x5a);
        assert!(json.contains("g-spriteRom-pixelMux"));
        assert!(json.contains("g-scanShift-display"));
        assert!(json.contains("\"signal\":\"data\""));
        assert!(json.contains("\"value\":90"));
        assert!(json.contains("\"width\":8"));
    }

    #[test]
    fn resolver_attaches_address_value_only_to_address_links() {
        let resolver = ActivityResolver::new();
        let json = resolver.links_json("dma", 0x8123, 0xa5);
        assert!(json.contains("\"signal\":\"address\""));
        assert!(json.contains("\"value\":33059"));
        assert!(json.contains("\"width\":16"));
    }

    #[test]
    fn resolver_rejects_unknown_phase_without_frontend_fallback() {
        let resolver = ActivityResolver::new();
        assert_eq!(resolver.links_json("not-a-phase", -1, -1), "[]");
    }
}
