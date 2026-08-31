use leader_core::{
    physical_activity_nodes, physical_alu_node_values, physical_flag_bit_changes,
    physical_pc_bit_changes, physical_register_bit_changes, physical_sp_bit_changes,
    BusTransactionEvent, BusTransactionKind, FrameState, Machine, MatchTrace, MicroCycleEvent,
    MicroCycleKind, MicroSample, PcEventKind, PhaseKind, PhysicalBitChange,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Playback {
    trace: Option<MatchTrace>,
    cursor: usize,
    playing: bool,
    bus_focus: Option<usize>,
    vblank_focus: Option<usize>,
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
    pub fn new() -> Self {
        Self {
            trace: None,
            cursor: 0,
            playing: false,
            bus_focus: None,
            vblank_focus: None,
        }
    }

    pub fn load_match(&mut self, seed: &str, max_frames: u32) -> bool {
        if seed.is_empty() || max_frames == 0 {
            return false;
        }
        let trace = Machine::run_match(seed, max_frames);
        if trace.micro_cycles.is_empty() {
            return false;
        }
        self.trace = Some(trace);
        self.cursor = 0;
        self.playing = false;
        self.clear_event_focus();
        true
    }

    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.trace.is_some()
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn play(&mut self) -> bool {
        if self.trace.is_none() || self.is_at_end() {
            return false;
        }
        self.playing = true;
        true
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn reset(&mut self) -> bool {
        if self.trace.is_none() {
            return false;
        }
        self.cursor = 0;
        self.playing = false;
        self.clear_event_focus();
        true
    }

    pub fn step_microcycle(&mut self) -> bool {
        self.playing = false;
        self.advance_one()
    }

    pub fn step_instruction(&mut self) -> bool {
        self.playing = false;
        self.clear_event_focus();
        let Some(trace) = &self.trace else {
            return false;
        };
        if trace.micro_cycles.is_empty() || self.cursor >= trace.micro_cycles.len() - 1 {
            return false;
        }
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
        if !self.playing || microcycles == 0 {
            return 0;
        }
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
        self.clear_event_focus();
        let Some(trace) = &self.trace else {
            return false;
        };
        let Some(index) = trace
            .micro_cycles
            .iter()
            .position(|event| event.frame >= frame)
        else {
            return false;
        };
        self.cursor = index;
        true
    }

    pub fn seek_cursor(&mut self, cursor: u32) -> bool {
        self.playing = false;
        self.clear_event_focus();
        let Ok(index) = usize::try_from(cursor) else {
            return false;
        };
        let Some(trace) = &self.trace else {
            return false;
        };
        if index >= trace.micro_cycles.len() {
            return false;
        }
        self.cursor = index;
        true
    }

    pub fn seek_progress(&mut self, progress: f64) -> bool {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return false;
        }
        let Some(trace) = &self.trace else {
            return false;
        };
        if trace.micro_cycles.is_empty() {
            return false;
        }
        let last = trace.micro_cycles.len() - 1;
        let target = (progress * last as f64).round() as usize;
        self.playing = false;
        self.clear_event_focus();
        self.cursor = target.min(last);
        true
    }

    pub fn seek_next_bus(&mut self) -> bool {
        let Some((index, target)) = self
            .next_bus_index_after_cursor(None)
            .map(|(index, event)| (index, bus_key(event)))
        else {
            return false;
        };
        if !self.seek_key(target) {
            return false;
        }
        self.bus_focus = Some(index);
        self.vblank_focus = None;
        true
    }

    pub fn seek_next_dma(&mut self) -> bool {
        let Some((index, target)) = self
            .next_bus_index_after_cursor(Some(BusTransactionKind::Dma))
            .map(|(index, event)| (index, bus_key(event)))
        else {
            return false;
        };
        if !self.seek_key(target) {
            return false;
        }
        self.bus_focus = Some(index);
        self.vblank_focus = None;
        true
    }

    pub fn seek_next_vblank(&mut self) -> bool {
        let Some((index, target)) = self
            .next_vblank_index_after_cursor()
            .map(|(index, sample)| (index, sample_key(sample)))
        else {
            return false;
        };
        if !self.seek_key(target) {
            return false;
        }
        self.vblank_focus = Some(index);
        self.bus_focus = None;
        true
    }

    #[must_use]
    pub fn current_microcycle_json(&self) -> String {
        self.current_event()
            .map(microcycle_json)
            .unwrap_or_else(|| "null".to_owned())
    }

    #[must_use]
    pub fn current_bus_json(&self) -> String {
        self.current_bus_event()
            .map(bus_json)
            .unwrap_or_else(|| "null".to_owned())
    }

    #[must_use]
    pub fn current_frame_json(&self) -> String {
        self.current_frame_state()
            .map(frame_json)
            .unwrap_or_else(|| "null".to_owned())
    }

    #[must_use]
    pub fn current_activity_json(&self) -> String {
        let Some((phase, address, data)) = self.current_activity() else {
            return "null".to_owned();
        };
        let nodes = physical_activity_nodes(phase, address)
            .iter()
            .map(|id| format!("\"{}\"", json_escape(id)))
            .collect::<Vec<_>>()
            .join(",");
        let address = address.map_or_else(|| "null".to_owned(), |value| value.to_string());
        let data = data.map_or_else(|| "null".to_owned(), |value| value.to_string());
        format!(
            "{{\"phase\":\"{}\",\"address\":{},\"data\":{},\"nodes\":[{}]}}",
            phase.as_str(),
            address,
            data,
            nodes
        )
    }

    #[must_use]
    pub fn current_alu_values_json(&self) -> String {
        let Some(trace) = &self.trace else {
            return "[]".to_owned();
        };
        let Some(key) = self.current_key() else {
            return "[]".to_owned();
        };
        let Some(event) = trace
            .alu_events
            .iter()
            .find(|event| (event.frame, event.ordinal) == key)
        else {
            return "[]".to_owned();
        };
        let values = physical_alu_node_values(event.trace)
            .into_iter()
            .map(|value| {
                format!(
                    "{{\"node\":\"{}\",\"bit\":{},\"stage\":\"{}\",\"value\":{}}}",
                    json_escape(&value.node_id),
                    value.bit,
                    json_escape(value.stage),
                    value.value
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{values}]")
    }

    #[must_use]
    pub fn current_bit_changes_json(&self) -> String {
        let Some(trace) = &self.trace else {
            return "[]".to_owned();
        };
        let Some(key) = self.current_key() else {
            return "[]".to_owned();
        };
        let mut changes = Vec::new();

        for event in trace
            .register_writes
            .iter()
            .filter(|event| (event.frame, event.ordinal) == key)
        {
            append_changes(
                &mut changes,
                physical_register_bit_changes(event.reg, event.before, event.after),
                &format!("reg:{}", event.reg.name()),
            );
        }

        for event in trace
            .pc_events
            .iter()
            .filter(|event| (event.frame, event.ordinal) == key)
        {
            let (before, after, source) = match event.kind {
                PcEventKind::Increment(step) => (step.before, step.after, "pc:increment"),
                PcEventKind::Load {
                    before,
                    after,
                    source,
                    ..
                } => (before, after, source.as_str()),
            };
            append_changes(
                &mut changes,
                physical_pc_bit_changes(before, after),
                source,
            );
        }

        for event in trace
            .sp_events
            .iter()
            .filter(|event| (event.frame, event.ordinal) == key)
        {
            append_changes(
                &mut changes,
                physical_sp_bit_changes(event.kind.before(), event.kind.after()),
                event.kind.as_str(),
            );
        }

        if let Some((index, event)) = trace
            .flag_events
            .iter()
            .enumerate()
            .find(|(_, event)| (event.frame, event.ordinal) == key)
        {
            let before = index
                .checked_sub(1)
                .and_then(|previous| trace.flag_events.get(previous))
                .map_or(0, |event| event.packed());
            append_changes(
                &mut changes,
                physical_flag_bit_changes(before, event.packed()),
                "flags",
            );
        }

        format!("[{}]", changes.join(","))
    }

    #[must_use]
    pub fn follow_pc_json(&self) -> String {
        let Some(event) = self.current_event() else {
            return "null".to_owned();
        };
        format!(
            "{{\"targetView\":\"view-pc.fetch\",\"primaryNode\":\"pcBit0\",\"pc\":{},\"frame\":{},\"ordinal\":{}}}",
            event.pc, event.frame, event.ordinal
        )
    }

    #[must_use]
    pub fn follow_bus_json(&self) -> String {
        let Some(event) = self.current_bus_event() else {
            return "null".to_owned();
        };
        format!(
            "{{\"targetView\":\"view-bus.arbitration\",\"primaryNode\":\"arb\",\"transaction\":{}}}",
            bus_json(event)
        )
    }

    #[must_use]
    pub fn follow_dma_json(&self) -> String {
        let Some(event) = self.current_dma_event() else {
            return "null".to_owned();
        };
        format!(
            "{{\"targetView\":\"view-gpu.dma\",\"primaryNode\":\"dmaAddr\",\"transaction\":{}}}",
            bus_json(event)
        )
    }

    #[must_use]
    pub fn follow_vblank_json(&self) -> String {
        let Some(sample) = self.current_vblank_sample() else {
            return "null".to_owned();
        };
        format!(
            "{{\"targetView\":\"view-gpu.timing\",\"primaryNode\":\"vblankLatch\",\"frame\":{},\"ordinal\":{},\"pc\":{},\"control\":\"{}\"}}",
            sample.frame,
            sample.ordinal,
            sample.pc,
            json_escape(&sample.control)
        )
    }

    #[must_use]
    pub fn summary_json(&self) -> String {
        let Some(trace) = &self.trace else {
            return "null".to_owned();
        };
        format!(
            "{{\"seed\":\"{}\",\"seedHash\":\"{:016x}\",\"finished\":{},\"totalFrames\":{},\"finalScore\":{},\"finalLives\":{},\"microcycles\":{},\"cursor\":{},\"playing\":{}}}",
            json_escape(&trace.seed),
            trace.seed_hash,
            trace.finished,
            trace.total_frames,
            trace.final_score,
            trace.final_lives,
            trace.micro_cycles.len(),
            self.cursor,
            self.playing
        )
    }

    #[must_use]
    pub fn progress(&self) -> f64 {
        let Some(trace) = &self.trace else {
            return 0.0;
        };
        if trace.micro_cycles.len() <= 1 {
            return 1.0;
        }
        self.cursor as f64 / (trace.micro_cycles.len() - 1) as f64
    }

    #[must_use]
    pub fn cursor(&self) -> u32 {
        u32::try_from(self.cursor).unwrap_or(u32::MAX)
    }

    #[must_use]
    pub fn microcycle_count(&self) -> u32 {
        self.trace.as_ref().map_or(0, |trace| {
            u32::try_from(trace.micro_cycles.len()).unwrap_or(u32::MAX)
        })
    }
}

impl Playback {
    fn current_event(&self) -> Option<&MicroCycleEvent> {
        self.trace
            .as_ref()
            .and_then(|trace| trace.micro_cycles.get(self.cursor))
    }

    fn current_key(&self) -> Option<(u32, u16)> {
        self.current_event().map(microcycle_key)
    }

    fn current_activity(&self) -> Option<(PhaseKind, Option<u16>, Option<u8>)> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;

        if self.vblank_focus.is_some() {
            if let Some(sample) = self.focused_vblank_sample() {
                return Some((PhaseKind::VBlank, sample.address, sample.data));
            }
        }

        if let Some(event) = self.focused_bus_event() {
            return Some((bus_phase(event.kind), event.address, event.data));
        }

        if let Some(event) = trace
            .bus_transactions
            .iter()
            .find(|event| bus_key(event) == key)
        {
            return Some((bus_phase(event.kind), event.address, event.data));
        }

        if trace
            .alu_events
            .iter()
            .any(|event| (event.frame, event.ordinal) == key)
        {
            return Some((PhaseKind::Alu, None, None));
        }

        let event = self.current_event()?;
        match event.kind {
            MicroCycleKind::FetchAddress | MicroCycleKind::FetchData => {
                Some((PhaseKind::Fetch, Some(event.mar), Some(event.mdr)))
            }
            MicroCycleKind::DecodeLatch => Some((PhaseKind::Decode, None, Some(event.ir))),
            _ => None,
        }
    }

    fn focused_bus_event(&self) -> Option<&BusTransactionEvent> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        let event = trace.bus_transactions.get(self.bus_focus?)?;
        (bus_key(event) <= key).then_some(event)
    }

    fn current_bus_event(&self) -> Option<&BusTransactionEvent> {
        if let Some(event) = self.focused_bus_event() {
            return Some(event);
        }
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace
            .bus_transactions
            .iter()
            .rev()
            .find(|event| bus_key(event) <= key)
    }

    fn current_dma_event(&self) -> Option<&BusTransactionEvent> {
        if let Some(event) = self.focused_bus_event() {
            if event.kind == BusTransactionKind::Dma {
                return Some(event);
            }
        }
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace
            .bus_transactions
            .iter()
            .rev()
            .find(|event| event.kind == BusTransactionKind::Dma && bus_key(event) <= key)
    }

    fn focused_vblank_sample(&self) -> Option<&MicroSample> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        let sample = trace.micro_samples.get(self.vblank_focus?)?;
        (sample_key(sample) <= key).then_some(sample)
    }

    fn current_vblank_sample(&self) -> Option<&MicroSample> {
        if let Some(sample) = self.focused_vblank_sample() {
            return Some(sample);
        }
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace
            .micro_samples
            .iter()
            .rev()
            .find(|sample| sample.phase == PhaseKind::VBlank && sample_key(sample) <= key)
    }

    fn current_frame_state(&self) -> Option<&FrameState> {
        let trace = self.trace.as_ref()?;
        let frame = self.current_event()?.frame;
        trace
            .frames
            .iter()
            .rev()
            .find(|state| state.frame <= frame)
    }

    fn next_bus_index_after_cursor(
        &self,
        kind: Option<BusTransactionKind>,
    ) -> Option<(usize, &BusTransactionEvent)> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace
            .bus_transactions
            .iter()
            .enumerate()
            .find(|(_, event)| {
                bus_key(event) > key && kind.is_none_or(|expected| event.kind == expected)
            })
    }

    fn next_vblank_index_after_cursor(&self) -> Option<(usize, &MicroSample)> {
        let trace = self.trace.as_ref()?;
        let key = self.current_key()?;
        trace
            .micro_samples
            .iter()
            .enumerate()
            .find(|(_, sample)| sample.phase == PhaseKind::VBlank && sample_key(sample) > key)
    }

    fn seek_key(&mut self, target: (u32, u16)) -> bool {
        self.playing = false;
        let Some(trace) = &self.trace else {
            return false;
        };
        let Some(index) = trace
            .micro_cycles
            .iter()
            .position(|event| microcycle_key(event) >= target)
        else {
            return false;
        };
        self.cursor = index;
        true
    }

    fn clear_event_focus(&mut self) {
        self.bus_focus = None;
        self.vblank_focus = None;
    }

    fn is_at_end(&self) -> bool {
        match &self.trace {
            Some(trace) => {
                trace.micro_cycles.is_empty() || self.cursor >= trace.micro_cycles.len() - 1
            }
            None => true,
        }
    }

    fn advance_one(&mut self) -> bool {
        let Some(trace) = &self.trace else {
            return false;
        };
        if self.cursor >= trace.micro_cycles.len().saturating_sub(1) {
            return false;
        }
        self.cursor += 1;
        self.clear_event_focus();
        true
    }
}

fn append_changes(target: &mut Vec<String>, changes: Vec<PhysicalBitChange>, source: &str) {
    target.extend(changes.into_iter().map(|change| {
        format!(
            "{{\"node\":\"{}\",\"before\":{},\"after\":{},\"source\":\"{}\"}}",
            json_escape(&change.node_id),
            change.before,
            change.after,
            json_escape(source)
        )
    }));
}

fn bus_phase(kind: BusTransactionKind) -> PhaseKind {
    match kind {
        BusTransactionKind::Fetch => PhaseKind::Fetch,
        BusTransactionKind::Read => PhaseKind::MemoryRead,
        BusTransactionKind::Write => PhaseKind::MemoryWrite,
        BusTransactionKind::Input => PhaseKind::Input,
        BusTransactionKind::Dma => PhaseKind::Dma,
        BusTransactionKind::Scanout => PhaseKind::Scanout,
    }
}

fn microcycle_key(event: &MicroCycleEvent) -> (u32, u16) {
    (event.frame, event.ordinal)
}

fn bus_key(event: &BusTransactionEvent) -> (u32, u16) {
    (event.frame, event.ordinal)
}

fn sample_key(sample: &MicroSample) -> (u32, u16) {
    (sample.frame, sample.ordinal)
}

fn microcycle_json(event: &MicroCycleEvent) -> String {
    format!(
        "{{\"frame\":{},\"ordinal\":{},\"phase\":\"{}\",\"kind\":\"{}\",\"pc\":{},\"mar\":{},\"mdr\":{},\"ir\":{},\"control\":\"{}\"}}",
        event.frame,
        event.ordinal,
        event.phase.as_str(),
        event.kind.as_str(),
        event.pc,
        event.mar,
        event.mdr,
        event.ir,
        json_escape(event.control)
    )
}

fn bus_json(event: &BusTransactionEvent) -> String {
    let address = event
        .address
        .map_or_else(|| "null".to_owned(), |value| value.to_string());
    let data = event
        .data
        .map_or_else(|| "null".to_owned(), |value| value.to_string());
    format!(
        "{{\"frame\":{},\"ordinal\":{},\"pc\":{},\"address\":{},\"data\":{},\"addressSource\":\"{}\",\"dataSource\":\"{}\",\"kind\":\"{}\",\"control\":\"{}\"}}",
        event.frame,
        event.ordinal,
        event.pc,
        address,
        data,
        event.address_source.as_str(),
        event.data_source.as_str(),
        event.kind.as_str(),
        json_escape(event.control)
    )
}

fn frame_json(frame: &FrameState) -> String {
    format!(
        "{{\"frame\":{},\"playerX\":{},\"fleetX\":{},\"fleetY\":{},\"fleetDir\":{},\"score\":{},\"lives\":{},\"pc\":{},\"vramChecksum\":{}}}",
        frame.frame,
        frame.player_x,
        frame.fleet_x,
        frame.fleet_y,
        frame.fleet_dir,
        frame.score,
        frame.lives,
        frame.pc,
        frame.vram_checksum
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
    fn playback_loads_native_microcycles_and_starts_paused() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-playback", 32));
        assert!(playback.is_loaded());
        assert!(!playback.is_playing());
        assert!(playback.microcycle_count() > 100);
        assert!(playback
            .current_microcycle_json()
            .contains("\"kind\":\"fetch_address\""));
        let activity = playback.current_activity_json();
        assert!(activity.contains("\"phase\":\"fetch\""));
        assert!(activity.contains("\"pcMuxLo\""));
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
        assert!(playback
            .current_microcycle_json()
            .contains("\"kind\":\"fetch_address\""));
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
    fn direct_cursor_and_progress_seek_use_native_microcycle_indices() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-scrub", 16));
        let count = playback.microcycle_count();
        assert!(count > 10);
        assert!(playback.seek_cursor(10));
        assert_eq!(playback.cursor(), 10);
        assert!(playback.seek_progress(0.5));
        let expected = ((count - 1) as f64 * 0.5).round() as u32;
        assert_eq!(playback.cursor(), expected);
        assert!(!playback.seek_progress(f64::NAN));
        assert!(!playback.seek_progress(1.1));
        assert!(!playback.seek_cursor(count));
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
    fn dma_and_vblank_follow_modes_keep_the_exact_native_event_focus() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-follow-video", 32));
        assert!(playback.seek_next_dma());
        assert!(playback.follow_dma_json().contains("view-gpu.dma"));
        assert!(playback.current_bus_json().contains("\"kind\":\"dma\""));
        let dma_activity = playback.current_activity_json();
        assert!(dma_activity.contains("\"phase\":\"dma\""));
        assert!(dma_activity.contains("\"dmaAddr\""));
        assert!(playback.seek_next_vblank());
        assert!(playback.follow_vblank_json().contains("view-gpu.timing"));
        assert!(playback
            .current_activity_json()
            .contains("\"phase\":\"vblank\""));
    }

    #[test]
    fn manual_timeline_motion_clears_auxiliary_event_focus() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-focus-clear", 32));
        assert!(playback.seek_next_dma());
        assert!(playback.bus_focus.is_some());
        assert!(playback.step_microcycle());
        assert!(playback.bus_focus.is_none());
        assert!(playback.vblank_focus.is_none());
    }

    #[test]
    fn exact_native_alu_gate_values_resolve_to_physical_slice_nodes() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-alu-values", 64));
        let count = playback.microcycle_count();
        let mut found = None;
        for cursor in 0..count {
            assert!(playback.seek_cursor(cursor));
            let json = playback.current_alu_values_json();
            if json != "[]" {
                found = Some(json);
                break;
            }
        }
        let json = found.expect("native trace contains ALU activity");
        assert!(json.contains("\"node\":\"xorA0\""));
        assert!(json.contains("\"node\":\"orC7\""));
        assert!(json.contains("\"stage\":\"result\""));
        assert!(json.contains("\"value\":"));
    }

    #[test]
    fn exact_native_state_changes_resolve_to_physical_bit_nodes() {
        let mut playback = Playback::new();
        assert!(playback.load_match("explorer-bit-changes", 64));
        let count = playback.microcycle_count();
        let mut found = None;
        for cursor in 0..count {
            assert!(playback.seek_cursor(cursor));
            let json = playback.current_bit_changes_json();
            if json != "[]" {
                found = Some(json);
                break;
            }
        }
        let json = found.expect("native trace contains architectural bit changes");
        assert!(json.contains("\"node\":"));
        assert!(json.contains("\"before\":"));
        assert!(json.contains("\"after\":"));
        assert!(json.contains("\"source\":"));
    }
}
