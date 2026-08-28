use crate::{
    isa::op, memory_map::VRAM_BASE, BusAddressSource, BusDataSource, BusTransactionKind,
    MatchTrace,
};

const WAIT_BIT: u32 = 1 << 6;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VideoPipelineValidation {
    pub raster_writes: usize,
    pub dma_bursts: usize,
    pub scanouts: usize,
    pub waits: usize,
}

pub fn validate_video_pipeline_contract(
    trace: &MatchTrace,
) -> Result<VideoPipelineValidation, String> {
    let raster = trace
        .bus_transactions
        .iter()
        .enumerate()
        .filter(|(_, event)| event.control == "VRAM_RASTER_1536_BYTES")
        .collect::<Vec<_>>();
    let dma = trace
        .bus_transactions
        .iter()
        .enumerate()
        .filter(|(_, event)| event.control == "DMA_BURST_1536_BYTES")
        .collect::<Vec<_>>();
    let scanout = trace
        .bus_transactions
        .iter()
        .enumerate()
        .filter(|(_, event)| event.control == "SCANOUT_128x96_1BPP")
        .collect::<Vec<_>>();

    if raster.is_empty() || dma.is_empty() || scanout.is_empty() {
        return Err(format!(
            "native video pipeline is incomplete: raster={} dma={} scanout={}",
            raster.len(),
            dma.len(),
            scanout.len()
        ));
    }
    if raster.len() != dma.len() || raster.len() != scanout.len() {
        return Err(format!(
            "native video pipeline stage counts diverge: raster={} dma={} scanout={}",
            raster.len(),
            dma.len(),
            scanout.len()
        ));
    }

    for ((raster_stage, dma_stage), scan_stage) in
        raster.iter().zip(&dma).zip(&scanout)
    {
        let (raster_index, raster_event) = *raster_stage;
        let (dma_index, dma_event) = *dma_stage;
        let (scan_index, scan_event) = *scan_stage;

        if !(raster_index < dma_index && dma_index < scan_index) {
            return Err(format!(
                "native video stage ordering is invalid at frame {}: raster_index={} dma_index={} scanout_index={}",
                raster_event.frame, raster_index, dma_index, scan_index
            ));
        }
        if raster_event.frame != dma_event.frame || raster_event.frame != scan_event.frame {
            return Err(format!(
                "native video stages cross frame boundaries: raster={} dma={} scanout={}",
                raster_event.frame, dma_event.frame, scan_event.frame
            ));
        }
        if raster_event.address != Some(VRAM_BASE)
            || dma_event.address != Some(VRAM_BASE)
            || scan_event.address != Some(VRAM_BASE)
        {
            return Err(format!(
                "native video stage does not target VRAM base at frame {}",
                raster_event.frame
            ));
        }
        if raster_event.kind != BusTransactionKind::Write
            || raster_event.address_source != BusAddressSource::Cpu
            || raster_event.data_source != BusDataSource::Cpu
        {
            return Err(format!(
                "VRAM raster authority is invalid at frame {}",
                raster_event.frame
            ));
        }
        if dma_event.kind != BusTransactionKind::Dma
            || dma_event.address_source != BusAddressSource::Dma
            || dma_event.data_source != BusDataSource::Vram
        {
            return Err(format!(
                "DMA burst authority is invalid at frame {}",
                dma_event.frame
            ));
        }
        if scan_event.kind != BusTransactionKind::Scanout
            || scan_event.address_source != BusAddressSource::Dma
            || scan_event.data_source != BusDataSource::Vram
        {
            return Err(format!(
                "scanout authority is invalid at frame {}",
                scan_event.frame
            ));
        }
        if raster_event.data.is_none() || raster_event.data != dma_event.data {
            return Err(format!(
                "raster/DMA checksum byte diverges at frame {}: raster={:?} dma={:?}",
                raster_event.frame, raster_event.data, dma_event.data
            ));
        }
    }

    let waits = trace
        .micro_addresses
        .iter()
        .filter(|event| event.opcode == op::WAIT_VBLANK)
        .collect::<Vec<_>>();
    if waits.is_empty() {
        return Err("native trace contains no WAIT_VBLANK µword".to_owned());
    }

    for wait in &waits {
        if wait.control_bits & WAIT_BIT == 0 {
            return Err(format!(
                "WAIT_VBLANK µword lacks physical WAIT bit at frame={} uaddr={:02X}",
                wait.frame, wait.address
            ));
        }
        if !scanout
            .iter()
            .any(|(_, event)| event.frame == wait.frame)
        {
            return Err(format!(
                "WAIT_VBLANK at frame {} has no native scanout in that frame",
                wait.frame
            ));
        }
    }

    Ok(VideoPipelineValidation {
        raster_writes: raster.len(),
        dma_bursts: dma.len(),
        scanouts: scanout.len(),
        waits: waits.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn complete_match_has_ordered_native_video_pipeline() {
        let trace = Machine::run_match("video-pipeline-contract", 5000);
        let validation =
            validate_video_pipeline_contract(&trace).expect("valid native video pipeline");
        assert!(validation.raster_writes > 0);
        assert_eq!(validation.raster_writes, validation.dma_bursts);
        assert_eq!(validation.raster_writes, validation.scanouts);
        assert!(validation.waits > 0);
        assert!(validation.waits < validation.scanouts);
    }

    #[test]
    fn missing_dma_stage_is_detected() {
        let mut trace = Machine::run_match("video-pipeline-missing-dma", 5000);
        let index = trace
            .bus_transactions
            .iter()
            .position(|event| event.control == "DMA_BURST_1536_BYTES")
            .expect("DMA burst");
        trace.bus_transactions.remove(index);
        let error = validate_video_pipeline_contract(&trace)
            .expect_err("missing DMA stage must fail");
        assert!(error.contains("stage counts diverge"));
    }

    #[test]
    fn corrupted_dma_checksum_is_detected() {
        let mut trace = Machine::run_match("video-pipeline-checksum", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.control == "DMA_BURST_1536_BYTES")
            .expect("DMA burst");
        event.data = event.data.map(|value| value ^ 1);
        let error = validate_video_pipeline_contract(&trace)
            .expect_err("checksum corruption must fail");
        assert!(error.contains("checksum byte diverges"));
    }

    #[test]
    fn wait_without_physical_wait_bit_is_detected() {
        let mut trace = Machine::run_match("video-pipeline-wait-bit", 5000);
        let event = trace
            .micro_addresses
            .iter_mut()
            .find(|event| event.opcode == op::WAIT_VBLANK && event.control_bits & WAIT_BIT != 0)
            .expect("WAIT_VBLANK µword");
        event.control_bits &= !WAIT_BIT;
        let error = validate_video_pipeline_contract(&trace)
            .expect_err("missing WAIT bit must fail");
        assert!(error.contains("lacks physical WAIT bit"));
    }
}
