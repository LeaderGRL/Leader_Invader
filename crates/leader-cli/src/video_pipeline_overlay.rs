use std::collections::BTreeSet;

use leader_core::{
    execute_address, BusTransactionEvent, MatchTrace, MicroAddressEvent, Topology, VBlankAckEvent,
    VideoScanEvent, VideoTiming, H_TOTAL, V_TOTAL,
};
use leader_core::isa::op;
use leader_svg::RenderConfig;

// Presentation density, not an artifact-size constraint. The native trace remains exhaustive.
const MAX_PER_STAGE: usize = 256;
const WAIT_BIT: u32 = 1 << 6;

#[derive(Debug)]
struct TimingReplay {
    scans: Vec<VideoScanEvent>,
    acks: Vec<VBlankAckEvent>,
}

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 {
        return svg;
    }
    let overlay = render(topology, trace, config);
    let Some(svg_close) = svg.rfind("</svg>") else {
        return svg;
    };
    let Some(world_close) = svg[..svg_close].rfind("</g>") else {
        return svg;
    };
    svg.insert_str(world_close, &overlay);
    svg
}

fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let raster = trace
        .bus_transactions
        .iter()
        .filter(|event| event.control == "VRAM_RASTER_1536_BYTES")
        .collect::<Vec<_>>();
    let dma = trace
        .bus_transactions
        .iter()
        .filter(|event| event.control == "DMA_BURST_1536_BYTES")
        .collect::<Vec<_>>();
    let scanout = trace
        .bus_transactions
        .iter()
        .filter(|event| event.control == "SCANOUT_128x96_1BPP")
        .collect::<Vec<_>>();
    let wait_uaddr = execute_address(op::WAIT_VBLANK).expect("WAIT_VBLANK execute address");
    let waits = trace
        .micro_addresses
        .iter()
        .filter(|event| event.opcode == op::WAIT_VBLANK && event.address == wait_uaddr)
        .collect::<Vec<_>>();
    let timing = replay_timing(&dma, &scanout, &waits);

    let mut out = String::with_capacity(2_100_000);
    out.push_str("<g id=\"m3-video-pipeline\">\n");

    for index in sampled_positions(raster.len(), &[]) {
        render_bus_stage(
            &mut out,
            topology,
            trace,
            config,
            raster[index],
            "raster",
            0.10,
            &["writeBus", "dataBuf", "vramPageDec", "vramByteDec"],
            None,
        );
    }
    for index in sampled_positions(dma.len(), &[]) {
        render_bus_stage(
            &mut out,
            topology,
            trace,
            config,
            dma[index],
            "dma",
            0.34,
            &["arb", "dmaAddr", "dmaData", "vramPageDec"],
            None,
        );
    }

    let overrun_indices = timing
        .scans
        .iter()
        .enumerate()
        .filter_map(|(index, event)| event.vblank_overrun.then_some(index))
        .collect::<Vec<_>>();
    for index in sampled_positions(scanout.len(), &overrun_indices) {
        render_bus_stage(
            &mut out,
            topology,
            trace,
            config,
            scanout[index],
            "scanout",
            0.58,
            &[
                "xCounter",
                "yCounter",
                "pixelMux",
                "scanShift",
                "hsync",
                "vsync",
                "vblankLatch",
                "display",
            ],
            timing.scans.get(index).copied(),
        );
    }
    for index in sampled_positions(waits.len(), &[]) {
        render_wait_stage(
            &mut out,
            topology,
            trace,
            config,
            waits[index],
            timing.acks.get(index).copied(),
        );
    }

    out.push_str("</g>\n");
    out
}

fn replay_timing(
    dma: &[&BusTransactionEvent],
    scanout: &[&BusTransactionEvent],
    waits: &[&MicroAddressEvent],
) -> TimingReplay {
    let mut hardware = VideoTiming::default();
    let mut scans = vec![None; scanout.len()];
    let mut acks = vec![None; waits.len()];
    let max_frame = scanout
        .iter()
        .map(|event| event.frame)
        .chain(waits.iter().map(|event| event.frame))
        .max()
        .unwrap_or(0);

    for frame in 0..=max_frame {
        for (index, event) in scanout.iter().enumerate().filter(|(_, event)| event.frame == frame) {
            let checksum = dma
                .get(index)
                .and_then(|event| event.data)
                .expect("validated video pipeline DMA checksum");
            scans[index] = Some(hardware.complete_scanout(checksum));
        }
        for (index, event) in waits.iter().enumerate().filter(|(_, event)| event.frame == frame) {
            let _ = event;
            acks[index] = Some(
                hardware
                    .acknowledge_vblank()
                    .expect("validated video pipeline armed VBlank"),
            );
        }
    }

    TimingReplay {
        scans: scans
            .into_iter()
            .map(|event| event.expect("scan timing event"))
            .collect(),
        acks: acks
            .into_iter()
            .map(|event| event.expect("VBlank acknowledge event"))
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_bus_stage(
    out: &mut String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
    event: &BusTransactionEvent,
    stage: &str,
    frame_offset: f32,
    nodes: &[&str],
    timing: Option<VideoScanEvent>,
) {
    let total = config.total();
    let moment = video_moment(event.frame, frame_offset, trace, config);
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.025, total);
    let k3 = norm(moment + 0.18, total);
    let checksum = event
        .data
        .map_or_else(|| "none".to_owned(), |value| format!("{value:02X}"));
    let address = event
        .address
        .map_or_else(|| "none".to_owned(), |value| format!("{value:04X}"));
    let timing_metadata = timing.map_or_else(String::new, |timing| {
        format!(
            " data-video-frame-counter=\"{}\" data-video-h-total=\"{}\" data-video-v-total=\"{}\" data-video-pixel-clocks=\"{}\" data-video-vblank-pending=\"1\" data-video-vblank-overrun=\"{}\"",
            timing.frame_after,
            H_TOTAL,
            V_TOTAL,
            timing.pixel_clocks,
            u8::from(timing.vblank_overrun)
        )
    });

    out.push_str(&format!(
        "<g opacity=\"0\" data-video-stage=\"{stage}\" data-video-frame=\"{}\" data-video-address=\"{address}\" data-video-checksum=\"{checksum}\" data-video-control=\"{}\"{timing_metadata}><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
        event.frame, event.control
    ));
    for node in nodes {
        glow_node(out, topology, node);
    }
    out.push_str("</g>\n");
}

fn render_wait_stage(
    out: &mut String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
    event: &MicroAddressEvent,
    ack: Option<VBlankAckEvent>,
) {
    let total = config.total();
    let moment = video_moment(event.frame, 0.82, trace, config);
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.025, total);
    let k3 = norm(moment + 0.18, total);
    let ack_metadata = ack.map_or_else(String::new, |ack| {
        format!(
            " data-video-vblank-ack=\"1\" data-video-frame-counter=\"{}\" data-video-ack-checksum=\"{:02X}\" data-video-vblank-pending=\"0\"",
            ack.frame_counter, ack.checksum
        )
    });
    out.push_str(&format!(
        "<g opacity=\"0\" data-video-stage=\"wait\" data-video-frame=\"{}\" data-video-uaddr=\"{:02X}\" data-video-wait-bit=\"{}\" data-video-control-word=\"{:06X}\"{ack_metadata}><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
        event.frame,
        event.address,
        u8::from(event.control_bits & WAIT_BIT != 0),
        event.control_bits
    ));
    glow_node(out, topology, "microRom");
    glow_node(out, topology, "ctrlWait");
    glow_node(out, topology, "vsync");
    glow_node(out, topology, "vblankLatch");
    glow_node(out, topology, "vblankWaitGate");
    glow_node(out, topology, "irqLatch");
    out.push_str("</g>\n");
}

fn sampled_positions(len: usize, required: &[usize]) -> Vec<usize> {
    if len <= MAX_PER_STAGE {
        return (0..len).collect();
    }
    let mut selected = BTreeSet::new();
    selected.insert(0usize);
    selected.insert(len - 1);
    for index in required.iter().copied().filter(|index| *index < len) {
        selected.insert(index);
    }
    for slot in 1..MAX_PER_STAGE {
        if selected.len() >= MAX_PER_STAGE {
            break;
        }
        let index = slot * (len - 1) / MAX_PER_STAGE;
        selected.insert(index);
    }
    if selected.len() < MAX_PER_STAGE {
        for index in 0..len {
            if selected.len() >= MAX_PER_STAGE {
                break;
            }
            selected.insert(index);
        }
    }
    selected.into_iter().take(MAX_PER_STAGE).collect()
}

fn glow_node(out: &mut String, topology: &Topology, id: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"8\" fill=\"#72d4e7\" fill-opacity=\".18\" stroke=\"#72d4e7\" stroke-width=\"9\" filter=\"url(#glow)\"/>",
        b.x - 3.0,
        b.y - 3.0,
        b.w + 6.0,
        b.h + 6.0
    ));
}

fn video_moment(frame: u32, offset: f32, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + offset * 0.06
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, validate_video_pipeline_contract, Machine};

    #[test]
    fn native_video_overlay_exposes_all_physical_stage_families_and_vblank_state() {
        let topology = build_topology();
        let trace = Machine::run_match("video-pipeline-overlay", 5000);
        let validation = validate_video_pipeline_contract(&trace).expect("valid video pipeline");
        assert!(validation.timing_overruns > 0);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("id=\"m3-video-pipeline\""));
        assert!(svg.contains("data-video-stage=\"raster\""));
        assert!(svg.contains("data-video-stage=\"dma\""));
        assert!(svg.contains("data-video-stage=\"scanout\""));
        assert!(svg.contains("data-video-stage=\"wait\""));
        assert!(svg.contains("data-video-address=\"8000\""));
        assert!(svg.contains("data-video-checksum=\""));
        assert!(svg.contains("data-video-wait-bit=\"1\""));
        assert!(svg.contains("data-video-h-total=\"160\""));
        assert!(svg.contains("data-video-v-total=\"112\""));
        assert!(svg.contains("data-video-pixel-clocks=\"17920\""));
        assert!(svg.contains("data-video-vblank-pending=\"1\""));
        assert!(svg.contains("data-video-vblank-pending=\"0\""));
        assert!(svg.contains("data-video-vblank-overrun=\"1\""));
        assert!(svg.contains("data-video-vblank-ack=\"1\""));
        assert!(svg.contains("data-video-ack-checksum=\""));
    }

    #[test]
    fn video_presentation_is_dense_and_preserves_overruns_and_timeline_extremes() {
        let trace = Machine::run_match("video-pipeline-sampling", 5000);
        let dma = trace
            .bus_transactions
            .iter()
            .filter(|event| event.control == "DMA_BURST_1536_BYTES")
            .collect::<Vec<_>>();
        let scanout = trace
            .bus_transactions
            .iter()
            .filter(|event| event.control == "SCANOUT_128x96_1BPP")
            .collect::<Vec<_>>();
        let wait_uaddr = execute_address(op::WAIT_VBLANK).expect("WAIT address");
        let waits = trace
            .micro_addresses
            .iter()
            .filter(|event| event.opcode == op::WAIT_VBLANK && event.address == wait_uaddr)
            .collect::<Vec<_>>();
        let timing = replay_timing(&dma, &scanout, &waits);
        let overruns = timing
            .scans
            .iter()
            .enumerate()
            .filter_map(|(index, event)| event.vblank_overrun.then_some(index))
            .collect::<Vec<_>>();
        let selected = sampled_positions(scanout.len(), &overruns);
        assert_eq!(selected.len(), MAX_PER_STAGE);
        assert_eq!(selected.first(), Some(&0));
        assert_eq!(selected.last(), Some(&(scanout.len() - 1)));
        for overrun in overruns {
            assert!(selected.contains(&overrun));
        }
    }
}
