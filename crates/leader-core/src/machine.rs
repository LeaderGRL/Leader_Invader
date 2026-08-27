use crate::game::{Bot, GameState, InputState, Projectile, ALIEN_COLS, ALIEN_H, ALIEN_ROWS, ALIEN_W, PLAYER_Y, SCREEN_H, SCREEN_W};
use crate::isa::{Bus, Cpu, MicroCycleKind, MicroPhase, StepOutcome};
use crate::microcode::MicroAddressTransition;
use crate::program::{build_game_rom, command, DEVICE_CMD, DEVICE_STATUS, INPUT_PORT, RAM_BASE};
use crate::rng::{hash_seed, DeterministicRng};
use crate::trace::{
    BusAddressSource, BusDataSource, BusTransactionEvent, BusTransactionKind, FrameState, KillEvent,
    MatchTrace, MicroAddressEvent, MicroCycleEvent, MicroSample, PhaseKind,
};

const ROM_LIMIT: usize = 0x2000;
const VRAM: usize = 0x8000;
const VRAM_BYTES: usize = 128 * 96 / 8;

#[derive(Debug)]
pub struct Machine {
    mem: Box<[u8; 65_536]>,
    cpu: Cpu,
    game: GameState,
    rng: DeterministicRng,
    bot: Bot,
    input: InputState,
    trace: MatchTrace,
    ordinal: u16,
    last_vram_checksum: u32,
}

impl Machine {
    #[must_use]
    pub fn run_match(seed: &str, max_frames: u32) -> MatchTrace {
        let rom = build_game_rom();
        Self::run_match_with_rom(seed, max_frames, &rom)
    }

    fn run_match_with_rom(seed: &str, max_frames: u32, rom: &[u8]) -> MatchTrace {
        let hash = hash_seed(seed);
        let mut rng = DeterministicRng::from_seed(hash);
        let bot = Bot::seeded(&mut rng);
        let mut machine = Self {
            mem: Box::new([0; 65_536]),
            cpu: Cpu::default(),
            game: GameState::default(),
            rng,
            bot,
            input: InputState::default(),
            trace: MatchTrace::new(seed.to_owned(), hash),
            ordinal: 0,
            last_vram_checksum: 0,
        };
        assert!(rom.len() <= ROM_LIMIT, "ROM exceeds 8 KiB");
        machine.mem[..rom.len()].copy_from_slice(rom);
        machine.sync_game_to_ram();
        machine.last_vram_checksum = machine.render_vram();
        machine.trace.frames.push(FrameState::from_game(
            &machine.game,
            machine.cpu.pc(),
            machine.last_vram_checksum,
        ));

        let instruction_budget = max_frames.saturating_mul(512).max(4096);
        for _ in 0..instruction_budget {
            machine.ordinal = 0;
            let mut cpu = std::mem::take(&mut machine.cpu);
            let outcome = cpu.step(&mut machine);
            machine.cpu = cpu;

            match outcome {
                StepOutcome::Continue => {}
                StepOutcome::WaitVBlank => {
                    machine.sample(
                        machine.cpu.pc(),
                        PhaseKind::VBlank,
                        Some(VRAM as u16),
                        None,
                        "VBLANK_IRQ",
                    );
                    machine.trace.frames.push(FrameState::from_game(
                        &machine.game,
                        machine.cpu.pc(),
                        machine.last_vram_checksum,
                    ));
                    if machine.game.frame >= max_frames {
                        break;
                    }
                }
                StepOutcome::Halted => break,
                StepOutcome::Fault { pc, opcode } => {
                    machine.sample(
                        pc,
                        PhaseKind::Decode,
                        Some(pc),
                        Some(opcode),
                        "CPU_FAULT",
                    );
                    break;
                }
            }
        }

        if machine.trace.frames.last().map(|f| f.frame) != Some(machine.game.frame) {
            machine.trace.frames.push(FrameState::from_game(
                &machine.game,
                machine.cpu.pc(),
                machine.last_vram_checksum,
            ));
        }
        machine.trace.finished = machine.game.is_clear();
        machine.trace.total_frames = machine.game.frame;
        machine.trace.final_score = machine.game.score;
        machine.trace.final_lives = machine.game.lives;
        machine.trace
    }

    fn device_command(&mut self, pc: u16, cmd: u8) {
        match cmd {
            command::POLL_INPUT => self.poll_input(pc),
            command::MOVE_PLAYER => self.move_player(pc),
            command::MOVE_FLEET => self.move_fleet(pc),
            command::PLAYER_SHOT => self.player_shot(pc),
            command::COLLIDE => self.collide(pc),
            command::ENEMY_SHOT => self.enemy_shot(pc),
            command::ADVANCE_FRAME => self.advance_frame(pc),
            command::RENDER_VRAM => self.render_video_device(pc),
            command::DMA_SCANOUT => self.dma_scanout(pc),
            command::CHECK_CLEAR => self.check_clear(pc),
            _ => self.sample(pc, PhaseKind::Decode, Some(DEVICE_CMD), Some(cmd), "UNKNOWN_DEVICE_CMD"),
        }
    }

    fn poll_input(&mut self, pc: u16) {
        self.input = self.bot.decide(&self.game, &mut self.rng);
        let encoded = encode_input(self.input);
        self.mem[INPUT_PORT as usize] = encoded;
        self.record_bus_transaction(
            pc,
            Some(INPUT_PORT),
            Some(encoded),
            BusAddressSource::None,
            BusDataSource::Device,
            BusTransactionKind::Input,
            "AUTOPILOT_INPUT",
        );
        self.sample(pc, PhaseKind::Input, Some(INPUT_PORT), Some(encoded), "AUTOPILOT_INPUT");
    }

    fn move_player(&mut self, pc: u16) {
        let old = self.traced_read(pc, RAM_BASE, "PLAYER_X_READ");
        let value = (i16::from(old) + i16::from(self.input.horizontal.clamp(-1, 1)) * 2)
            .clamp(6, SCREEN_W - 7) as u8;
        self.sample(pc, PhaseKind::Alu, None, Some(value), "PLAYER_X_ADD");
        self.game.player_x = i16::from(value);
        self.traced_write(pc, RAM_BASE, value, "PLAYER_X_WRITE");
    }

    fn move_fleet(&mut self, pc: u16) {
        if self.game.frame % 3 != 0 {
            return;
        }
        let old = self.traced_read(pc, RAM_BASE + 1, "FLEET_X_READ");
        let next = i16::from(old) + i16::from(self.game.fleet_dir);
        let right = next + (ALIEN_COLS as i16 - 1) * 12 + ALIEN_W;
        if next <= 3 || right >= SCREEN_W - 3 {
            self.game.fleet_dir = -self.game.fleet_dir;
            self.game.fleet_y += 2;
            self.traced_write(pc, RAM_BASE + 2, self.game.fleet_y as u8, "FLEET_Y_WRITE");
            self.traced_write(pc, RAM_BASE + 3, self.game.fleet_dir as u8, "FLEET_DIR_WRITE");
        } else {
            self.game.fleet_x = next;
            self.sample(pc, PhaseKind::Alu, None, Some(next as u8), "FLEET_X_ADD");
            self.traced_write(pc, RAM_BASE + 1, next as u8, "FLEET_X_WRITE");
        }
        let line = self.game.fleet_y + (ALIEN_ROWS as i16 - 1) * 13 + ALIEN_H;
        if line >= PLAYER_Y - 8 {
            self.game.fleet_y = 10;
            self.game.lives = self.game.lives.saturating_sub(1).max(1);
            self.traced_write(pc, RAM_BASE + 2, 10, "FLEET_RESET");
        }
    }

    fn player_shot(&mut self, pc: u16) {
        self.game.player_cooldown = self.game.player_cooldown.saturating_sub(1);
        if let Some(mut shot) = self.game.player_shot {
            shot.y -= 3;
            self.sample(pc, PhaseKind::Alu, None, Some(shot.y.max(0) as u8), "SHOT_Y_SUB");
            if shot.y <= 4 {
                self.game.player_shot = None;
                self.traced_write(pc, RAM_BASE + 6, 0, "SHOT_CLEAR");
            } else {
                self.game.player_shot = Some(shot);
                self.traced_write(pc, RAM_BASE + 5, shot.y as u8, "SHOT_Y_WRITE");
            }
        } else if self.input.fire && self.game.player_cooldown == 0 {
            let shot = Projectile { x: self.game.player_x, y: PLAYER_Y - 5 };
            self.game.player_shot = Some(shot);
            self.game.player_cooldown = 6;
            self.traced_write(pc, RAM_BASE + 4, shot.x as u8, "SHOT_X_WRITE");
            self.traced_write(pc, RAM_BASE + 5, shot.y as u8, "SHOT_Y_WRITE");
            self.traced_write(pc, RAM_BASE + 6, 1, "SHOT_ARM");
        }
    }

    fn collide(&mut self, pc: u16) {
        let Some(shot) = self.game.player_shot else { return; };
        for row in 0..ALIEN_ROWS {
            let mask = self.traced_read(pc, RAM_BASE + 0x10 + row as u16, "ALIEN_ROW_READ");
            for col in 0..ALIEN_COLS {
                if mask & (1 << col) == 0 { continue; }
                let (x, y) = self.game.alien_origin(row, col);
                if shot.x >= x && shot.x < x + ALIEN_W && shot.y >= y && shot.y < y + ALIEN_H + 2 {
                    let new_mask = mask & !(1 << col);
                    self.sample(pc, PhaseKind::Alu, None, Some(new_mask), "ALIEN_MASK_AND");
                    if self.game.clear_alien(row, col) {
                        self.traced_write(pc, RAM_BASE + 0x10 + row as u16, new_mask, "ALIEN_ROW_WRITE");
                        self.write16(pc, RAM_BASE + 0x0A, self.game.score, "SCORE_WRITE");
                        self.game.player_shot = None;
                        self.traced_write(pc, RAM_BASE + 6, 0, "SHOT_HIT_CLEAR");
                        self.trace.kills.push(KillEvent { frame: self.game.frame, row, col, score_after: self.game.score });
                    }
                    return;
                }
            }
        }
    }

    fn enemy_shot(&mut self, pc: u16) {
        self.game.enemy_cooldown = self.game.enemy_cooldown.saturating_sub(1);
        if let Some(mut shot) = self.game.enemy_shot {
            shot.y += 2;
            self.sample(pc, PhaseKind::Alu, None, Some(shot.y as u8), "ENEMY_SHOT_Y_ADD");
            let (left, right) = self.game.player_bounds();
            if shot.y >= PLAYER_Y - 2 && shot.x >= left && shot.x <= right {
                self.game.lives = self.game.lives.saturating_sub(1).max(1);
                self.game.enemy_shot = None;
                self.traced_write(pc, RAM_BASE + 9, 0, "ENEMY_SHOT_HIT");
            } else if shot.y >= SCREEN_H - 4 {
                self.game.enemy_shot = None;
                self.traced_write(pc, RAM_BASE + 9, 0, "ENEMY_SHOT_CLEAR");
            } else {
                self.game.enemy_shot = Some(shot);
                self.traced_write(pc, RAM_BASE + 8, shot.y as u8, "ENEMY_SHOT_Y_WRITE");
            }
        }
        if self.game.enemy_shot.is_none() && self.game.enemy_cooldown == 0 {
            let shooters = self.game.bottom_shooters();
            if !shooters.is_empty() {
                let pick = shooters[self.rng.range_u32(shooters.len() as u32) as usize];
                let shot = Projectile { x: pick.2, y: pick.3 + 1 };
                self.game.enemy_shot = Some(shot);
                self.game.enemy_cooldown = 22 + self.rng.range_u32(42) as u8;
                self.traced_write(pc, RAM_BASE + 7, shot.x as u8, "ENEMY_SHOT_X_WRITE");
                self.traced_write(pc, RAM_BASE + 8, shot.y as u8, "ENEMY_SHOT_Y_WRITE");
                self.traced_write(pc, RAM_BASE + 9, 1, "ENEMY_SHOT_ARM");
            }
        }
    }

    fn advance_frame(&mut self, pc: u16) {
        self.game.frame = self.game.frame.saturating_add(1);
        self.write16(pc, RAM_BASE + 0x18, self.game.frame as u16, "FRAME_WRITE");
    }

    fn render_video_device(&mut self, pc: u16) {
        self.last_vram_checksum = self.render_vram();
        let data = (self.last_vram_checksum & 0xFF) as u8;
        self.record_bus_transaction(
            pc,
            Some(VRAM as u16),
            Some(data),
            BusAddressSource::Cpu,
            BusDataSource::Cpu,
            BusTransactionKind::Write,
            "VRAM_RASTER_1536_BYTES",
        );
        self.sample(
            pc,
            PhaseKind::MemoryWrite,
            Some(VRAM as u16),
            Some(data),
            "VRAM_RASTER_1536_BYTES",
        );
    }

    fn dma_scanout(&mut self, pc: u16) {
        let data = (self.last_vram_checksum & 0xFF) as u8;
        self.record_bus_transaction(
            pc,
            Some(VRAM as u16),
            Some(data),
            BusAddressSource::Dma,
            BusDataSource::Vram,
            BusTransactionKind::Dma,
            "DMA_BURST_1536_BYTES",
        );
        self.sample(
            pc,
            PhaseKind::Dma,
            Some(VRAM as u16),
            Some(data),
            "DMA_BURST_1536_BYTES",
        );
        self.record_bus_transaction(
            pc,
            Some(VRAM as u16),
            None,
            BusAddressSource::Dma,
            BusDataSource::Vram,
            BusTransactionKind::Scanout,
            "SCANOUT_128x96_1BPP",
        );
        self.sample(
            pc,
            PhaseKind::Scanout,
            Some(VRAM as u16),
            None,
            "SCANOUT_128x96_1BPP",
        );
    }

    fn check_clear(&mut self, pc: u16) {
        let status = u8::from(self.game.is_clear());
        self.mem[DEVICE_STATUS as usize] = status;
        self.record_bus_transaction(
            pc,
            Some(DEVICE_STATUS),
            Some(status),
            BusAddressSource::Cpu,
            BusDataSource::Cpu,
            BusTransactionKind::Write,
            "GAME_CLEAR_STATUS",
        );
        self.sample(
            pc,
            PhaseKind::MemoryWrite,
            Some(DEVICE_STATUS),
            Some(status),
            "GAME_CLEAR_STATUS",
        );
    }

    fn traced_read(&mut self, pc: u16, address: u16, control: &'static str) -> u8 {
        let value = self.mem[address as usize];
        self.record_bus_transaction(
            pc,
            Some(address),
            Some(value),
            BusAddressSource::Cpu,
            bus_data_source(address),
            BusTransactionKind::Read,
            control,
        );
        self.sample(pc, PhaseKind::MemoryRead, Some(address), Some(value), control);
        value
    }

    fn traced_write(&mut self, pc: u16, address: u16, value: u8, control: &'static str) {
        self.mem[address as usize] = value;
        self.record_bus_transaction(
            pc,
            Some(address),
            Some(value),
            BusAddressSource::Cpu,
            BusDataSource::Cpu,
            BusTransactionKind::Write,
            control,
        );
        self.sample(pc, PhaseKind::MemoryWrite, Some(address), Some(value), control);
    }

    fn record_bus_transaction(
        &mut self,
        pc: u16,
        address: Option<u16>,
        data: Option<u8>,
        address_source: BusAddressSource,
        data_source: BusDataSource,
        kind: BusTransactionKind,
        control: &'static str,
    ) {
        self.trace.bus_transactions.push(BusTransactionEvent {
            frame: self.game.frame,
            ordinal: self.ordinal,
            pc,
            address,
            data,
            address_source,
            data_source,
            kind,
            control,
        });
    }

    fn write16(&mut self, pc: u16, address: u16, value: u16, control: &'static str) {
        self.traced_write(pc, address, value as u8, control);
        self.traced_write(pc, address + 1, (value >> 8) as u8, control);
    }

    fn sample(&mut self, pc: u16, phase: PhaseKind, address: Option<u16>, data: Option<u8>, control: &'static str) {
        self.trace.micro_samples.push(MicroSample {
            frame: self.game.frame,
            ordinal: self.ordinal,
            phase,
            pc,
            address,
            data,
            control: control.to_owned(),
        });
        self.ordinal = self.ordinal.saturating_add(1);
    }

    fn sync_game_to_ram(&mut self) {
        self.mem[RAM_BASE as usize] = self.game.player_x as u8;
        self.mem[(RAM_BASE + 1) as usize] = self.game.fleet_x as u8;
        self.mem[(RAM_BASE + 2) as usize] = self.game.fleet_y as u8;
        self.mem[(RAM_BASE + 3) as usize] = self.game.fleet_dir as u8;
        self.mem[(RAM_BASE + 0x0A) as usize] = self.game.score as u8;
        self.mem[(RAM_BASE + 0x0B) as usize] = (self.game.score >> 8) as u8;
        self.mem[(RAM_BASE + 0x0C) as usize] = self.game.lives;
        for row in 0..ALIEN_ROWS {
            self.mem[(RAM_BASE + 0x10 + row as u16) as usize] = self.game.alive_rows[row];
        }
    }

    fn render_vram(&mut self) -> u32 {
        self.mem[VRAM..VRAM + VRAM_BYTES].fill(0);
        for row in 0..ALIEN_ROWS {
            for col in 0..ALIEN_COLS {
                if self.game.alien_alive(row, col) {
                    let (x, y) = self.game.alien_origin(row, col);
                    draw_invader(&mut self.mem, x, y, (row + col) % 2 == 0);
                }
            }
        }
        draw_player(&mut self.mem, self.game.player_x, PLAYER_Y);
        if let Some(shot) = self.game.player_shot { draw_shot(&mut self.mem, shot.x, shot.y, 4); }
        if let Some(shot) = self.game.enemy_shot { draw_shot(&mut self.mem, shot.x, shot.y, 3); }
        for x in 0..SCREEN_W { pixel(&mut self.mem, x, SCREEN_H - 3); }
        let mut hash = 0x811c_9dc5_u32;
        for byte in &self.mem[VRAM..VRAM + VRAM_BYTES] {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        hash
    }
}

impl Bus for Machine {
    fn fetch8(&mut self, pc: u16) -> u8 {
        let value = self.mem[pc as usize];
        self.record_bus_transaction(
            pc,
            Some(pc),
            Some(value),
            BusAddressSource::ProgramCounter,
            BusDataSource::Rom,
            BusTransactionKind::Fetch,
            "ROM_FETCH",
        );
        self.sample(pc, PhaseKind::Fetch, Some(pc), Some(value), "ROM_FETCH");
        value
    }

    fn read8(&mut self, pc: u16, address: u16) -> u8 {
        self.traced_read(pc, address, "CPU_READ")
    }

    fn write8(&mut self, pc: u16, address: u16, value: u8) {
        self.traced_write(pc, address, value, "CPU_WRITE");
        if address == DEVICE_CMD {
            self.device_command(pc, value);
        }
    }

    fn trace_decode(&mut self, pc: u16, opcode: u8, mnemonic: &'static str) {
        self.sample(pc, PhaseKind::Decode, Some(pc), Some(opcode), mnemonic);
    }

    fn trace_alu(&mut self, pc: u16, value: u8, control: &'static str) {
        self.sample(pc, PhaseKind::Alu, None, Some(value), control);
    }

    fn trace_control(&mut self, pc: u16, control: &'static str) {
        self.sample(pc, PhaseKind::Decode, Some(pc), None, control);
    }

    fn trace_microaddress(
        &mut self,
        transition: MicroAddressTransition,
        opcode: u8,
        control_bits: u8,
        label: &'static str,
    ) {
        self.trace.micro_addresses.push(MicroAddressEvent {
            frame: self.game.frame,
            ordinal: self.ordinal,
            before: transition.before,
            address: transition.after,
            source: transition.source,
            opcode,
            control_bits,
            label,
        });
    }

    fn trace_microcycle(
        &mut self,
        phase: MicroPhase,
        kind: MicroCycleKind,
        pc: u16,
        mar: u16,
        mdr: u8,
        ir: u8,
        control: &'static str,
    ) {
        self.trace.micro_cycles.push(MicroCycleEvent {
            frame: self.game.frame,
            ordinal: self.ordinal,
            phase,
            kind,
            pc,
            mar,
            mdr,
            ir,
            control,
        });

        let timing = match phase {
            MicroPhase::T0 => "µT0",
            MicroPhase::T1 => "µT1",
            MicroPhase::T2 => "µT2",
        };
        self.sample(pc, PhaseKind::Decode, Some(mar), Some(mdr), timing);
    }
}

fn bus_data_source(address: u16) -> BusDataSource {
    match address {
        0x0000..=0x1fff => BusDataSource::Rom,
        0x2000..=0x7fff => BusDataSource::Ram,
        0x8000..=0x87ff => BusDataSource::Vram,
        0xa000..=0xa1ff => BusDataSource::Device,
        _ => BusDataSource::None,
    }
}

fn encode_input(input: InputState) -> u8 {
    let horizontal = if input.horizontal < 0 { 1 } else if input.horizontal > 0 { 2 } else { 0 };
    horizontal | if input.fire { 4 } else { 0 }
}

fn pixel(memory: &mut [u8; 65_536], x: i16, y: i16) {
    if !(0..SCREEN_W).contains(&x) || !(0..SCREEN_H).contains(&y) { return; }
    let index = y as usize * SCREEN_W as usize + x as usize;
    memory[VRAM + index / 8] |= 1 << (7 - index % 8);
}

fn draw_player(memory: &mut [u8; 65_536], x: i16, y: i16) {
    for dx in -5..=5 { pixel(memory, x + dx, y); }
    for dx in -3..=3 { pixel(memory, x + dx, y - 1); }
    for dy in 2..=4 { pixel(memory, x, y - dy); }
}

fn draw_shot(memory: &mut [u8; 65_536], x: i16, y: i16, height: i16) {
    for dy in 0..height { pixel(memory, x, y + dy); }
}

fn draw_invader(memory: &mut [u8; 65_536], x: i16, y: i16, variant: bool) {
    let a = ["00111100", "01111110", "11011011", "11111111", "00100100", "01000010"];
    let b = ["01100110", "11111111", "10111101", "11111111", "00100100", "01011010"];
    let bitmap = if variant { &b } else { &a };
    for (dy, row) in bitmap.iter().enumerate() {
        for (dx, value) in row.as_bytes().iter().enumerate() {
            if *value == b'1' { pixel(memory, x + dx as i16, y + dy as i16); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_rom_execution() {
        let a = Machine::run_match("same", 5000);
        let b = Machine::run_match("same", 5000);
        assert_eq!(a.kills, b.kills);
        assert_eq!(a.total_frames, b.total_frames);
    }

    #[test]
    fn bot_clears_through_rom_program() {
        let trace = Machine::run_match("ci-clear", 5000);
        assert!(trace.finished, "{} frames", trace.total_frames);
        assert_eq!(trace.final_score, 320);
        assert_eq!(trace.kills.len(), 32);
        assert!(trace.micro_samples.iter().any(|s| s.control == "CALL"));
        assert!(trace.micro_samples.iter().any(|s| s.control == "RET"));
        assert!(trace.micro_samples.iter().any(|s| s.control == "WAIT_VBLANK"));
        assert!(!trace.micro_cycles.is_empty());
        assert!(!trace.micro_addresses.is_empty());
        assert!(!trace.bus_transactions.is_empty());
        assert!(trace.bus_transactions.iter().any(|event| {
            event.kind == BusTransactionKind::Fetch
                && event.address_source == BusAddressSource::ProgramCounter
                && event.data_source == BusDataSource::Rom
        }));
        assert!(trace.bus_transactions.iter().any(|event| {
            event.kind == BusTransactionKind::Dma
                && event.address_source == BusAddressSource::Dma
                && event.data_source == BusDataSource::Vram
        }));
        assert!(trace.bus_transactions.iter().any(|event| {
            event.kind == BusTransactionKind::Input
                && event.data_source == BusDataSource::Device
        }));
        assert!(trace.micro_addresses.iter().any(|event| event.address == crate::microcode::uaddr::FETCH_T0));
        assert!(trace.micro_addresses.iter().any(|event| event.address >= crate::microcode::uaddr::EXEC_BASE));
        assert!(trace.micro_addresses.iter().any(|event| event.source == crate::microcode::MicroAddressSource::Dispatch));
        assert!(trace.micro_addresses.iter().any(|event| event.source == crate::microcode::MicroAddressSource::RoutineCall));
        assert!(trace.micro_addresses.iter().any(|event| event.source == crate::microcode::MicroAddressSource::RoutineReturn));
    }

    #[test]
    fn corrupting_rom_breaks_match_causally() {
        let mut rom = build_game_rom();
        rom[0] = crate::isa::op::HALT;
        let trace = Machine::run_match_with_rom("broken-rom", 5000, &rom);
        assert!(!trace.finished);
        assert_eq!(trace.total_frames, 0);
        assert_eq!(trace.kills.len(), 0);
    }
}
