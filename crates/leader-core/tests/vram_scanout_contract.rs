use leader_core::{BusTransactionKind, Machine};

#[test]
fn every_native_scanout_uses_the_checkpointed_framebuffer() {
    let trace = Machine::run_match("vram-scanout-contract", 5_000);

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

    assert!(!scanout.is_empty(), "complete match must exercise native scanout");
    assert_eq!(raster.len(), dma.len());
    assert_eq!(dma.len(), scanout.len());

    for ((raster, dma), scanout) in raster.iter().zip(&dma).zip(&scanout) {
        assert_eq!(raster.frame, dma.frame);
        assert_eq!(dma.frame, scanout.frame);
        assert_eq!(raster.kind, BusTransactionKind::Write);
        assert_eq!(dma.kind, BusTransactionKind::Dma);
        assert_eq!(scanout.kind, BusTransactionKind::Scanout);

        let checkpoint = trace
            .vram_checkpoints
            .iter()
            .find(|checkpoint| checkpoint.frame == scanout.frame)
            .unwrap_or_else(|| panic!("scanout frame {} has no native VRAM checkpoint", scanout.frame));

        assert_eq!(checkpoint.bytes.len(), 128 * 96 / 8);
        assert_eq!(raster.data, Some((checkpoint.checksum & 0xff) as u8));
        assert_eq!(dma.data, Some((checkpoint.checksum & 0xff) as u8));
    }
}
