use std::fmt::Write;

use crate::game::{GameState, Projectile, ALIEN_ROWS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseKind {
    Fetch,
    Decode,
    Input,
    MemoryRead,
    Alu,
    MemoryWrite,
    Dma,
    Scanout,
    VBlank,
}

impl PhaseKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Decode => "decode",
            Self::Input => "input",
            Self::MemoryRead => "memory_read",
            Self::Alu => "alu",
            Self::MemoryWrite => "memory_write",
            Self::Dma => "dma",
            Self::Scanout => "scanout",
            Self::VBlank => "vblank",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MicroSample {
    pub frame: u32,
    pub ordinal: u16,
    pub phase: PhaseKind,
    pub pc: u16,
    pub address: Option<u16>,
    pub data: Option<u8>,
    pub control: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectileSnapshot {
    pub x: i16,
    pub y: i16,
}

impl From<Projectile> for ProjectileSnapshot {
    fn from(value: Projectile) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameState {
    pub frame: u32,
    pub player_x: i16,
    pub fleet_x: i16,
    pub fleet_y: i16,
    pub fleet_dir: i8,
    pub player_shot: Option<ProjectileSnapshot>,
    pub enemy_shot: Option<ProjectileSnapshot>,
    pub alive_rows: [u8; ALIEN_ROWS],
    pub score: u16,
    pub lives: u8,
    pub pc: u16,
    pub vram_checksum: u32,
}

impl FrameState {
    #[must_use]
    pub fn from_game(game: &GameState, pc: u16, vram_checksum: u32) -> Self {
        Self {
            frame: game.frame,
            player_x: game.player_x,
            fleet_x: game.fleet_x,
            fleet_y: game.fleet_y,
            fleet_dir: game.fleet_dir,
            player_shot: game.player_shot.map(Into::into),
            enemy_shot: game.enemy_shot.map(Into::into),
            alive_rows: game.alive_rows,
            score: game.score,
            lives: game.lives,
            pc,
            vram_checksum,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KillEvent {
    pub frame: u32,
    pub row: usize,
    pub col: usize,
    pub score_after: u16,
}

#[derive(Debug, Clone)]
pub struct MatchTrace {
    pub seed: String,
    pub seed_hash: u64,
    pub frames: Vec<FrameState>,
    pub micro_samples: Vec<MicroSample>,
    pub kills: Vec<KillEvent>,
    pub finished: bool,
    pub total_frames: u32,
    pub final_score: u16,
    pub final_lives: u8,
}

impl MatchTrace {
    #[must_use]
    pub fn new(seed: String, seed_hash: u64) -> Self {
        Self {
            seed,
            seed_hash,
            frames: Vec::new(),
            micro_samples: Vec::new(),
            kills: Vec::new(),
            finished: false,
            total_frames: 0,
            final_score: 0,
            final_lives: 0,
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let mut out = String::with_capacity(self.frames.len() * 180);
        let _ = write!(
            out,
            "{{\n  \"seed\": \"{}\",\n  \"seed_hash\": \"{:016x}\",\n  \"finished\": {},\n  \"total_frames\": {},\n  \"final_score\": {},\n  \"final_lives\": {},\n  \"kills\": [",
            json_escape(&self.seed),
            self.seed_hash,
            self.finished,
            self.total_frames,
            self.final_score,
            self.final_lives
        );

        for (index, kill) in self.kills.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "\n    {{\"frame\":{},\"row\":{},\"col\":{},\"score_after\":{}}}",
                kill.frame, kill.row, kill.col, kill.score_after
            );
        }

        out.push_str("\n  ],\n  \"frames\": [");
        for (index, frame) in self.frames.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let player_shot = projectile_json(frame.player_shot);
            let enemy_shot = projectile_json(frame.enemy_shot);
            let _ = write!(
                out,
                "\n    {{\"frame\":{},\"player_x\":{},\"fleet_x\":{},\"fleet_y\":{},\"fleet_dir\":{},\"player_shot\":{},\"enemy_shot\":{},\"alive_rows\":[{},{},{},{}],\"score\":{},\"lives\":{},\"pc\":{},\"vram_checksum\":{}}}",
                frame.frame,
                frame.player_x,
                frame.fleet_x,
                frame.fleet_y,
                frame.fleet_dir,
                player_shot,
                enemy_shot,
                frame.alive_rows[0],
                frame.alive_rows[1],
                frame.alive_rows[2],
                frame.alive_rows[3],
                frame.score,
                frame.lives,
                frame.pc,
                frame.vram_checksum
            );
        }

        out.push_str("\n  ],\n  \"micro_samples\": [");
        for (index, sample) in self.micro_samples.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let address = sample
                .address
                .map_or_else(|| "null".to_owned(), |value| value.to_string());
            let data = sample
                .data
                .map_or_else(|| "null".to_owned(), |value| value.to_string());
            let _ = write!(
                out,
                "\n    {{\"frame\":{},\"ordinal\":{},\"phase\":\"{}\",\"pc\":{},\"address\":{},\"data\":{},\"control\":\"{}\"}}",
                sample.frame,
                sample.ordinal,
                sample.phase.as_str(),
                sample.pc,
                address,
                data,
                json_escape(&sample.control)
            );
        }
        out.push_str("\n  ]\n}\n");
        out
    }
}

fn projectile_json(projectile: Option<ProjectileSnapshot>) -> String {
    projectile.map_or_else(
        || "null".to_owned(),
        |value| format!("{{\"x\":{},\"y\":{}}}", value.x, value.y),
    )
}

fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
