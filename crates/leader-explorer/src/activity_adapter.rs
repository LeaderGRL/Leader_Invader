use leader_core::{
    activity::physical_activity_links, build_topology, physical_bus_link_values, BusAddressSource,
    BusDataSource, BusTransactionEvent, BusTransactionKind, PhaseKind, SignalKind, Topology,
};

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

    /// Resolves one already-native bus transaction into dependency-ordered
    /// canonical physical links. The browser passes through values reported by
    /// Playback and never reconstructs memory ownership, page selection or bus
    /// direction itself.
    #[must_use]
    pub fn bus_links_json(
        &self,
        address: i32,
        data: i32,
        address_source: &str,
        data_source: &str,
        kind: &str,
    ) -> String {
        let (Some(address), Some(address_source), Some(data_source), Some(kind)) = (
            optional_u16(address),
            parse_address_source(address_source),
            parse_data_source(data_source),
            parse_bus_kind(kind),
        ) else {
            return "[]".to_owned();
        };
        let event = BusTransactionEvent {
            frame: 0,
            ordinal: 0,
            pc: 0,
            address: Some(address),
            data: optional_u8(data),
            address_source,
            data_source,
            kind,
            control: "WASM_ACTIVITY_RESOLVER",
        };
        let links = physical_bus_link_values(&self.topology, event)
            .into_iter()
            .map(|link| {
                format!(
                    "{{\"id\":\"{}\",\"rank\":{},\"stage\":\"{}\",\"signal\":\"{}\",\"value\":{},\"width\":{}}}",
                    json_escape(&link.link_id),
                    link.rank,
                    link.stage,
                    link.signal.css_class(),
                    link.value,
                    link.width
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

fn parse_address_source(value: &str) -> Option<BusAddressSource> {
    match value {
        "program_counter" => Some(BusAddressSource::ProgramCounter),
        "cpu" => Some(BusAddressSource::Cpu),
        "dma" => Some(BusAddressSource::Dma),
        "none" => Some(BusAddressSource::None),
        _ => None,
    }
}

fn parse_data_source(value: &str) -> Option<BusDataSource> {
    match value {
        "rom" => Some(BusDataSource::Rom),
        "ram" => Some(BusDataSource::Ram),
        "vram" => Some(BusDataSource::Vram),
        "cpu" => Some(BusDataSource::Cpu),
        "device" => Some(BusDataSource::Device),
        "none" => Some(BusDataSource::None),
        _ => None,
    }
}

fn parse_bus_kind(value: &str) -> Option<BusTransactionKind> {
    match value {
        "fetch" => Some(BusTransactionKind::Fetch),
        "read" => Some(BusTransactionKind::Read),
        "write" => Some(BusTransactionKind::Write),
        "input" => Some(BusTransactionKind::Input),
        "dma" => Some(BusTransactionKind::Dma),
        "scanout" => Some(BusTransactionKind::Scanout),
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
    fn resolver_exposes_dependency_ordered_native_bus_path() {
        let resolver = ActivityResolver::new();
        let json = resolver.bus_links_json(0x83fe, 0x5a, "dma", "vram", "dma");
        assert!(json.contains("\"stage\":\"dma_address_driver\""));
        assert!(json.contains("\"rank\":1"));
        assert!(json.contains("\"stage\":\"page_select\""));
        assert!(json.contains("vram"));
        assert!(json.contains("\"stage\":\"dma_data_latch\""));
        assert!(json.contains("\"rank\":6"));
    }

    #[test]
    fn resolver_rejects_unknown_bus_semantics_without_frontend_fallback() {
        let resolver = ActivityResolver::new();
        assert_eq!(
            resolver.bus_links_json(0x8000, 0x5a, "browser_guess", "vram", "dma"),
            "[]"
        );
    }

    #[test]
    fn resolver_rejects_unknown_phase_without_frontend_fallback() {
        let resolver = ActivityResolver::new();
        assert_eq!(resolver.links_json("not-a-phase", -1, -1), "[]");
    }
}