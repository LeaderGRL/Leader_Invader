#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeSvgValidation {
    pub overlay_groups: usize,
    pub metadata_families: usize,
}

const REQUIRED_GROUPS: [&str; 11] = [
    "f3-pc",
    "f3-native-decoder",
    "f3-microcode",
    "f3-physical-control-bank",
    "f3-control-state-latches",
    "f3-native-microcycles",
    "f3-native-alu",
    "f3-native-registers",
    "f3-native-bus",
    "f3-stack",
    "f3-timing",
];

const REQUIRED_METADATA: [&str; 10] = [
    "data-pc-before=",
    "data-opcode=",
    "data-ucontrol=",
    "data-control-state=",
    "data-micro-mar=",
    "data-alu-carry-chain=",
    "data-reg-before=",
    "data-bus-address-source=",
    "data-sp-before=",
    "data-stack-value=",
];

pub fn validate_native_svg_contract(svg: &str) -> Result<NativeSvgValidation, String> {
    if svg.contains("class=\"hot ") {
        return Err("production SVG still contains legacy semantic hot activity".to_owned());
    }

    let mut validation = NativeSvgValidation::default();
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

    if svg.contains("<script") || svg.contains("javascript:") {
        return Err("production SVG violates declarative-only render contract".to_owned());
    }

    Ok(validation)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
