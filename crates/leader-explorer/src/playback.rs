use leader_core::{
    BusTransactionEvent, BusTransactionKind, FrameState, Machine, MatchTrace, MicroCycleEvent,
    MicroCycleKind, MicroSample, PhaseKind,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Playback {
    trace: Option<MatchTrace>,
    cursor: usize,
    playing: bool,
}

impl Default for Playback {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Playback {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    #[must_use]
    pub const fn new() -> Self {
        Self { trace: None, cursor: 0, playing: false }
    }

    pub fn load_match(&mut self, seed: &str, max_frames: u32) -> bool {
        if seed.is_empty() || max_frames == 0 { return false; }
        let trace = Machine::run_match(seed, max_frames);
        if trace.micro_cycles.is_empty() { return false; }
        self.trace = Some(trace);
        self.cursor = 0;
        self.playing = false;
        true
    }

    #[must_use]
    pub fn is_loaded(&self) -> bool { self.trace.is_some() }

    #[must_use]
    pub const fn is_playing(&self) -> bool { self.playing }

    pub fn play(&mut self) -> bool {
        if self.trace.is_none() || self.is_at_end() { return false; }
        self.playing = true;
        true
    }

    pub fn pause(&mut self) { self.playing = false; }

    pub fn reset(&mut self) -> bool {
        if self.trace.is_none() { return false; }
        self.cursor = 0;
        self.playing = false;
        true
    }

    pub fn step_microcycle(&mut self) -> bool {
        self.playing = false;
        self.advance_one()
    }

    pub fn step_instruction(&mut self) -> bool {
        self.playing = false;
        let Some(trace) = &self.trace else { return false; };
        if trace.micro_cycles.is_empty() || self.cursor >= trace.micro_cycles.len() - 1 { return false; }
        let start = self.cursor.saturating_add(1);
        let Some(offset) = trace.micro_cycles[start..]
            .iter()
            .position(|event| event.kind == MicroCycleKind::FetchAddress)
        else {
            self.cursor = trace.micro_cycles.len() - 1;
            return true;
        };
        self.cursor = start + offset;
        true
    }

    pub fn tick(&mut self, microcycles: u32) -> u32 {
        if !self.playing || microcycles == 0 { return 0; }
        let mut advanced = 0;
        for _ in 0..microcycles {
            if !self.advance_one() {
                self.playing = false;
                break;
            }
            advanced += 1;
        }
        advanced
    }

    pub fn seek_frame(&mut self, frame: u32) -> bool {
        self.playing = false;
        let Some(trace) = &self.trace else { return false; };
        let Some(index) = trace.micro_cycles.iter().position(|event| event.frame >= frame) else { return false; };
        self.cursor = index;
        true
    }

    pub fn seek_next_bus(&mut self) -> bool {
        let Some(target) = self.next_bus_after_cursor(None).map(bus_key) else { return false; };
        self.seek_key(target)
    }

    pub fn seek_next_dma(&mut self) -> bool {
        let Some(target) = self.next_bus_after_cursor(Some(BusTransactionKind::Dma)).map(bus_key) else { return false; };
        self.seek_key(target)
    }

    pub fn seek_next_vblank(&mut self) -> bool {
        let Some(target) = self.next_vblank_after_cursor().map(sample_key) else { return false; };
        self.seek_key(target)
    }

    #[must_use]
    pub fn current_microcycle_json(&self) -> String {
        self.current_event().map(microcycle_json).unwrap_or_else(|| "null".to_owned())
    }

    #[must_use]
    pub fn current_bus_json(&self) -> String {
        self.current_bus_event().map(bus_json).unwrap_or_else(|| "null".to_owned())
    }

    #[must_use]
    pub fn current_frame_json(&self) -> String {
        self.current_frame_state().map(frame_json).unwrap_or_else(|| "null".to_owned())
    }

    #[must_use]
    pub fn follow_pc_json(&self) -> String {
        let Some(event) = self.current_event() else { return "null".to_owned(); };
        format!("{{\"targetView\":\"view-pc.fetch\",\"primaryNode\":\"pcBit0\",\"pc\":{},\"frame\":{},\"ordinal\":{}}}", event.pc, event.frame, event.ordinal)
    }

    #[must_use]
    pub fn follow_bus_json(&self) -> String {
        let Some(event) = self.current_bus_event() else { return "null".to_owned(); };
        format!("{{\"targetView\":\"view-bus.arbitration\",\"primaryNode\":\"arb\",\"transaction\":{}}}", bus_json(event))
    }

    #[must_use]
    pub fn follow_dma_json(&self) -> String {
        let Some(event) = self.current_dma_event() else { return "null".to_owned(); };
        format!("{{\"targetView\":\"view-gpu.dma\",\"primaryNode\":\"dmaAddr\",\"transaction\":{}}}", bus_json(event))
    }

    #[must_use]
    pub fn follow_vblank_json(&self) -> String {
        let Some(sample) = self.current_vblank_sample() else { return "null".to_owned(); };
        format!("{{\"targetView\":\"view-gpu.timing\",\"primaryNode\":\"vblankLatch\",\"frame\":{},\"ordinal\":{},\"pc\":{},\"control\":\"{}\"}}", sample.frame, sample.ordinal, sample.pc, json_escape(&sample.control))
    }

    #[must_use]
    pub fn summary_json(&self) -> String {
        let Some(trace) = &self.trace else { return "null".to_owned(); };
        format!("{{\"seed\":\"{}\",\"seedHash\":\"{:016x}\",\"finished\":{},\"totalFrames\":{},\"finalScore\":{},\"finalLives\":{},\"microcycles\":{},\"cursor\":{},\"playing\":{}}}", json_escape(&trace.seed), trace.seed_hash, trace.finished, trace.total_frames, trace.final_score, trace.final_lives, trace.micro_cycles.len(), self.cursor, self.playing)
    }

    #[must_use]
    pub fn progress(&self) -> f64 {
        let Some(trace) = &self.trace else { return 0.0; };
        if trace.micro_cycles.len() <= 1 { return 1.0; }
        self.cursor as f64 / (trace.micro_cycles.len() - 1) as f64
    }

    #[must_use]
    pub fn cursor(&self) -> u32 { u32::try_from(self.cursor).unwrap_or(u32::MAX) }

    #[must_use]
    pub fn microcycle_count(&self) -> u32 {
        self.trace.as_ref().map_or(0, |trace| u32::try_from(trace.micro_cycles.len()).unwrap_or(u32::MAX))
    }
}

impl Playback {
    fn current_event(&self) -> Option<&MicroCycleEvent> {
        self.trace.as_ref().and_then(|trace| trace.micro_cycles.get(self.cursor))
    }

    fn current_key(&self) -> Option<(u32, u16)> { self.current_event().map(microcycle_key) }

    fn current_bus_event(&self) -> Option<&BusTransactionEvent> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace.bus_transactions.iter().rev().find(|event| bus_key(event) <= key)
    }

    fn current_dma_event(&self) -> Option<&BusTransactionEvent> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace.bus_transactions.iter().rev().find(|event| event.kind == BusTransactionKind::Dma && bus_key(event) <= key)
    }

    fn current_vblank_sample(&self) -> Option<&MicroSample> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace.micro_samples.iter().rev().find(|sample| sample.phase == PhaseKind::VBlank && sample_key(sample) <= key)
    }

    fn current_frame_state(&self) -> Option<&FrameState> {
        let trace = self.trace.as_ref()?;
        let frame = self.current_event()?.frame;
        trace.frames.iter().rev().find(|state| state.frame <= frame)
    }

    fn next_bus_after_cursor(&self, kind: Option<BusTransactionKind>) -> Option<&BusTransactionEvent> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace.bus_transactions.iter().find(|event| bus_key(event) > key && kind.map_or(true, |expected| event.kind == expected))
    }

    fn next_vblank_after_cursor(&self) -> Option<&MicroSample> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace.micro_samples.iter().find(|sample| sample.phase == PhaseKind::VBlank && sample_key(sample) > key)
    }

    fn seek_key(&mut self, target: (u32, u16)) -> bool {
        self.playing = false;
        let Some(trace) = &self.trace else { return false; };
        let Some(index) = trace.micro_cycles.iter().position(|event| microcycle_key(event) >= target) else { return false; };
        self.cursor = index;
        true
    }

    fn is_at_end(&self) -> bool {
        match &self.trace {
            Some(trace) => trace.micro_cycles.is_empty() || self.cursor >= trace.micro_cycles.len() - 1,
            None => true,
        }
    }

    fn advance_one(&mut self) -> bool {
        let Some(trace) = &self.trace else { return false; };
        if self.cursor >= trace.micro_cycles.len().saturating_sub(1) { return false; }
        self.cursor += 1;
        true
    }
}

fn microcycle_key(event: &MicroCycleEvent) -> (u32, u16) { (event.frame, event.ordinal) }
fn bus_key(event: &BusTransactionEvent) -> (u32, u16) { (event.frame, event.ordinal) }
fn sample_key(sample: &MicroSample) -> (u32, u16) { (sample.frame, sample.ordinal) }

fn microcycle_json(event: &MicroCycleEvent) -> String {
    format!("{{\"frame\":{},\"ordinal\":{},\"phase\":\"{}\",\"kind\":\"{}\",\"pc\":{},\"mar\":{},\"mdr\":{},\"ir\":{},\"control\":\"{}\"}}", event.frame, event.ordinal, event.phase.as_str(), event.kind.as_str(), event.pc, event.mar, event.mdr, event.ir, json_escape(event.control))
}

fn bus_json(event: &BusTransactionEvent) -> String {
    let address = event.address.map_or_else(|| "null".to_owned(), |value| value.to_string());
    let data = event.data.map_or_else(|| "null".to_owned(), |value| value.to_string());
    format!("{{\"frame\":{},\"ordinal\":{},\"pc\":{},\"address\":{},\"data\":{},\"addressSource\":\"{}\",\"dataSource\":\"{}\",\"kind\":\"{}\",\"control\":\"{}\"}}", event.frame, event.ordinal, event.pc, address, data, event.address_source.as_str(), event.data_source.as_str(), event.kind.as_str(), json_escape(event.control))
}

fn frame_json(frame: &FrameState) -> String {
    format!("{{\"frame\":{},\"playerX\":{},\"fleetX\":{},\"fleetY\":{},\"fleetDir\":{},\"score\":{},\"lives\":{},\"pc\":{},\"vramChecksum\":{}}}", frame.frame, frame.player_x, frame.fleet_x, frame.fleet_y, frame.fleet_dir, frame.score, frame.lives, frame.pc, frame.vram_checksum)
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_loads_native_microcycles_and_starts_paused() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-playback", 32));
        assert!(playback.is_loaded());
        assert!(!playback.is_playing());
        assert!(playback.microcycle_count() > 100);
        assert!(playback.current_microcycle_json().contains("\"kind\":\"fetch_address\""));
    }

    #[test]
    fn microstep_moves_exactly_one_native_event() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-microstep", 16));
        let before = playback.cursor();
        assert!(playback.step_microcycle());
        assert_eq!(playback.cursor(), before + 1);
    }

    #[test]
    fn instruction_step_lands_on_next_fetch_address() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-instruction-step", 16));
        assert!(playback.step_instruction());
        assert!(playback.cursor() > 0);
        assert!(playback.current_microcycle_json().contains("\"kind\":\"fetch_address\""));
    }

    #[test]
    fn play_tick_and_pause_do_not_recompute_machine_state() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-play", 16));
        assert!(playback.play());
        assert_eq!(playback.tick(4), 4);
        assert_eq!(playback.cursor(), 4);
        playback.pause();
        assert_eq!(playback.tick(4), 0);
        assert_eq!(playback.cursor(), 4);
    }

    #[test]
    fn seek_frame_uses_first_native_microcycle_at_or_after_frame() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-seek", 32));
        assert!(playback.seek_frame(2));
        let event = playback.current_event().expect("current event");
        assert!(event.frame >= 2);
    }

    #[test]
    fn follow_pc_and_bus_expose_native_state_without_ui_reconstruction() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-follow", 16));
        assert!(playback.follow_pc_json().contains("view-pc.fetch"));
        assert!(playback.seek_next_bus());
        assert!(playback.current_bus_json().contains("\"kind\":"));
        assert!(playback.follow_bus_json().contains("view-bus.arbitration"));
        assert!(playback.current_frame_json().contains("\"vramChecksum\":"));
    }

    #[test]
    fn dma_and_vblank_follow_modes_seek_real_native_events() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-follow-video", 32));
        assert!(playback.seek_next_dma());
        assert!(playback.follow_dma_json().contains("view-gpu.dma"));
        assert!(playback.current_bus_json().contains("\"kind\":\"dma\""));
        assert!(playback.seek_next_vblank());
        assert!(playback.follow_vblank_json().contains("view-gpu.timing"));
    }
}
