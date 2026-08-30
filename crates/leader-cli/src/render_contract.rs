#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeSvgValidation {
    pub overlay_groups: usize,
    pub metadata_families: usize,
    pub bus_coverage_markers: usize,
    pub bytes: usize,
}

const REQUIRED_GROUPS: [&str; 17] = [
    "f3-pc",
    "f3-native-decoder",
    "f3-microcode",
    "f3-physical-control-bank",
    "f3-control-state-latches",
    "f3-native-microcycles",
    "f3-native-alu",
    "f3-native-flags",
    "f3-native-registers",
    "f3-native-bus",
    "m3-video-pipeline",
    "f3-stack",
    "m3-formation-cadence",
    "m3-shift-register",
    "m3-enemy-shot-bank",
    "m3-shield-bank",
    "f3-timing",
];

const REQUIRED_METADATA: [&str; 45] = [
    "data-pc-before=",
    "data-opcode=",
    "data-ucontrol=",
    "data-control-state=",
    "data-control-value=",
    "data-control-valid=",
    "data-control-owner=",
    "data-micro-mar=",
    "data-alu-carry-chain=",
    "data-flags-packed=",
    "data-flag-z=",
    "data-reg-before=",
    "data-bus-address-source=",
    "data-bus-memory-owner=",
    "data-bus-mmio-port=",
    "data-bus-mmio-access=",
    "data-video-stage=",
    "data-video-frame=",
    "data-video-address=",
    "data-video-checksum=",
    "data-video-uaddr=",
    "data-video-wait-bit=",
    "data-video-control-word=",
    "data-sp-before=",
    "data-sp-chain=",
    "data-cadence-alive=",
    "data-cadence-divisor=",
    "data-cadence-counter=",
    "data-cadence-tick=",
    "data-shift-kind=",
    "data-shift-state=",
    "data-shift-offset=",
    "data-shift-result=",
    "data-enemy-shot-frame=",
    "data-enemy-shot-active-count=",
    "data-enemy-shot-slot=",
    "data-enemy-shot-active=",
    "data-enemy-shot-x=",
    "data-enemy-shot-y=",
    "data-shield-index=",
    "data-shield-byte=",
    "data-shield-mask=",
    "data-shield-before=",
    "data-shield-after=",
    "data-shield-source=",
];

const REQUIRED_BUS_COVERAGE: [&str; 33] = [
    "data-bus-memory-owner=\"rom\"",
    "data-bus-memory-owner=\"ram\"",
    "data-bus-memory-owner=\"vram\"",
    "data-bus-memory-owner=\"mmio\"",
    "data-bus-kind=\"fetch\"",
    "data-bus-kind=\"read\"",
    "data-bus-kind=\"write\"",
    "data-bus-kind=\"input\"",
    "data-bus-kind=\"dma\"",
    "data-bus-kind=\"scanout\"",
    "data-bus-address-source=\"program_counter\"",
    "data-bus-address-source=\"cpu\"",
    "data-bus-address-source=\"dma\"",
    "data-bus-address-source=\"none\"",
    "data-bus-data-source=\"rom\"",
    "data-bus-data-source=\"ram\"",
    "data-bus-data-source=\"vram\"",
    "data-bus-data-source=\"cpu\"",
    "data-bus-data-source=\"device\"",
    "data-bus-mmio-port=\"input\"",
    "data-bus-mmio-port=\"shift_data\"",
    "data-bus-mmio-port=\"shift_offset\"",
    "data-bus-mmio-port=\"shift_result\"",
    "data-bus-mmio-port=\"device_cmd\"",
    "data-bus-mmio-port=\"device_status\"",
    "data-bus-mmio-access=\"input_only\"",
    "data-bus-mmio-access=\"read_only\"",
    "data-bus-mmio-access=\"write_only\"",
    "data-bus-mmio-access=\"read_write\"",
    "data-video-stage=\"raster\"",
    "data-video-stage=\"dma\"",
    "data-video-stage=\"scanout\"",
    "data-video-stage=\"wait\"",
];

pub fn validate_native_svg_contract(svg: &str) -> Result<NativeSvgValidation, String> {
    if svg.contains("class=\"hot ") {
        return Err("production SVG still contains legacy semantic hot activity".to_owned());
    }

    let mut validation = NativeSvgValidation {
        bytes: svg.len(),
        ..NativeSvgValidation::default()
    };
    for id in REQUIRED_GROUPS {
        let marker = format!("id=\"{id}\"");
        if !svg.contains(&marker) {
            return Err(format!("production SVG is missing native overlay group {id}"));
        }
        validation.overlay_groups += 1;
    }

    for marker in REQUIRED_METADATA {
        if !svg.contains(marker) {
            return Err(format!("production SVG is missing native metadata marker {marker}"));
        }
        validation.metadata_families += 1;
    }

    for marker in REQUIRED_BUS_COVERAGE {
        if !svg.contains(marker) {
            return Err(format!(
                "production SVG is missing required native bus coverage marker {marker}"
            ));
        }
        validation.bus_coverage_markers += 1;
    }

    if svg.contains("<script") || svg.contains("javascript:") {
        return Err("production SVG violates declarative-only render contract".to_owned());
    }

    Ok(validation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_contract_fixture() -> String {
        let mut svg = String::from("<svg>");
        for id in REQUIRED_GROUPS {
            svg.push_str(&format!("<g id=\"{id}\"></g>"));
        }
        for marker in REQUIRED_METADATA {
            svg.push_str(marker);
        }
        for marker in REQUIRED_BUS_COVERAGE {
            svg.push_str(marker);
        }
        svg.push_str("</svg>");
        svg
    }

    #[test]
    fn legacy_hot_activity_is_rejected() {
        let error = validate_native_svg_contract("<svg><rect class=\"hot hot-alu\"/></svg>")
            .expect_err("legacy activity must fail");
        assert!(error.contains("legacy semantic hot activity"));
    }

    #[test]
    fn missing_native_group_is_rejected() {
        let error = validate_native_svg_contract("<svg/>").expect_err("missing overlays must fail");
        assert!(error.contains("missing native overlay group"));
    }

    #[test]
    fn artifact_size_is_observational_not_a_validity_limit() {
        let mut svg = valid_contract_fixture();
        svg.push_str(&"x".repeat(5_100_000));
        let validation = validate_native_svg_contract(&svg)
            .expect("large native SVG must remain valid when its semantic contract is complete");
        assert_eq!(validation.bytes, svg.len());
        assert!(validation.bytes > 5_000_000);
    }

    #[test]
    fn generic_bus_metadata_without_concrete_coverage_is_rejected() {
        let mut svg = String::from("<svg>");
        for id in REQUIRED_GROUPS {
            svg.push_str(&format!("<g id=\"{id}\"></g>"));
        }
        for marker in REQUIRED_METADATA {
            svg.push_str(marker);
        }
        svg.push_str("</svg>");
        let error = validate_native_svg_contract(&svg)
            .expect_err("generic bus metadata must not satisfy concrete coverage");
        assert!(error.contains("missing required native bus coverage marker"));
    }
}
