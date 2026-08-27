use crate::assembler::Assembler;
use crate::isa::Reg;

pub const RAM_BASE: u16 = 0x2000;
pub const INPUT_PORT: u16 = 0xA000;
pub const DEVICE_CMD: u16 = 0xA100;
pub const DEVICE_STATUS: u16 = 0xA101;
pub const DEVICE_ARG0: u16 = 0xA102;
pub const DEVICE_ARG1: u16 = 0xA103;

pub mod command {
    pub const POLL_INPUT: u8 = 1;
    pub const MOVE_PLAYER: u8 = 2;
    pub const MOVE_FLEET: u8 = 3;
    pub const PLAYER_SHOT: u8 = 4;
    pub const COLLIDE: u8 = 5;
    pub const ENEMY_SHOT: u8 = 6;
    pub const ADVANCE_FRAME: u8 = 7;
    pub const RENDER_VRAM: u8 = 8;
    pub const DMA_SCANOUT: u8 = 9;
    pub const CHECK_CLEAR: u8 = 10;
}

#[must_use]
pub fn build_game_rom() -> Vec<u8> {
    let mut a = Assembler::new();

    a.label("reset");
    a.ldi(Reg::A, 0);
    a.st(DEVICE_STATUS, Reg::A);
    a.call("frame");
    a.label("main");
    a.call("frame");
    a.ld(Reg::A, DEVICE_STATUS);
    a.cmpi(Reg::A, 1);
    a.jz("clear");
    a.wait_vblank();
    a.jmp("main");

    a.label("frame");
    device_call(&mut a, command::POLL_INPUT);
    device_call(&mut a, command::MOVE_PLAYER);
    device_call(&mut a, command::MOVE_FLEET);
    device_call(&mut a, command::PLAYER_SHOT);
    device_call(&mut a, command::COLLIDE);
    device_call(&mut a, command::ENEMY_SHOT);
    device_call(&mut a, command::ADVANCE_FRAME);
    device_call(&mut a, command::RENDER_VRAM);
    device_call(&mut a, command::DMA_SCANOUT);
    device_call(&mut a, command::CHECK_CLEAR);
    a.ret();

    a.label("clear");
    a.halt();

    let rom = a.finish();
    assert!(rom.len() <= 0x2000, "game ROM exceeds 8 KiB");
    rom
}

fn device_call(a: &mut Assembler, command: u8) {
    a.ldi(Reg::A, command);
    a.st(DEVICE_CMD, Reg::A);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_fits_first_8k() {
        let rom = build_game_rom();
        assert!(!rom.is_empty());
        assert!(rom.len() < 0x2000);
    }
}
