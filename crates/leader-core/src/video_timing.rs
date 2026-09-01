pub const H_VISIBLE: u16 = 128;
pub const H_FRONT_PORCH: u16 = 8;
pub const H_SYNC: u16 = 16;
pub const H_BACK_PORCH: u16 = 8;
pub const H_TOTAL: u16 = H_VISIBLE + H_FRONT_PORCH + H_SYNC + H_BACK_PORCH;

pub const V_VISIBLE: u16 = 96;
pub const V_FRONT_PORCH: u16 = 4;
pub const V_SYNC: u16 = 4;
pub const V_BACK_PORCH: u16 = 8;
pub const V_TOTAL: u16 = V_VISIBLE + V_FRONT_PORCH + V_SYNC + V_BACK_PORCH;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VideoTiming {
    frame_counter: u32,
    vblank_pending: bool,
    last_checksum: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoScanEvent {
    pub frame_before: u32,
    pub frame_after: u32,
    pub pixel_clocks: u32,
    pub visible_lines: u16,
    pub blank_lines: u16,
    pub hsync_pulses: u16,
    pub vsync_lines: u16,
    pub checksum: u8,
    pub vblank_overrun: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VBlankAckEvent {
    pub frame_counter: u32,
    pub checksum: u8,
}

impl VideoTiming {
    #[must_use]
    pub const fn frame_counter(&self) -> u32 { self.frame_counter }

    #[must_use]
    pub const fn vblank_pending(&self) -> bool { self.vblank_pending }

    #[must_use]
    pub const fn last_checksum(&self) -> u8 { self.last_checksum }

    pub fn complete_scanout(&mut self, checksum: u8) -> VideoScanEvent {
        let frame_before = self.frame_counter;
        let vblank_overrun = self.vblank_pending;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        self.vblank_pending = true;
        self.last_checksum = checksum;
        VideoScanEvent {
            frame_before,
            frame_after: self.frame_counter,
            pixel_clocks: u32::from(H_TOTAL) * u32::from(V_TOTAL),
            visible_lines: V_VISIBLE,
            blank_lines: V_TOTAL - V_VISIBLE,
            hsync_pulses: V_TOTAL,
            vsync_lines: V_SYNC,
            checksum,
            vblank_overrun,
        }
    }

    pub fn acknowledge_vblank(&mut self) -> Option<VBlankAckEvent> {
        if !self.vblank_pending {
            return None;
        }
        self.vblank_pending = false;
        Some(VBlankAckEvent {
            frame_counter: self.frame_counter,
            checksum: self.last_checksum,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_geometry_is_closed_and_matches_display() {
        assert_eq!(H_TOTAL, 160);
        assert_eq!(V_TOTAL, 112);
        assert_eq!(H_VISIBLE, 128);
        assert_eq!(V_VISIBLE, 96);
        assert_eq!(u32::from(H_TOTAL) * u32::from(V_TOTAL), 17_920);
    }

    #[test]
    fn scanout_arms_persistent_vblank_and_wait_acknowledges_once() {
        let mut timing = VideoTiming::default();
        let scan = timing.complete_scanout(0xA5);
        assert_eq!(scan.frame_before, 0);
        assert_eq!(scan.frame_after, 1);
        assert!(!scan.vblank_overrun);
        assert!(timing.vblank_pending());

        let ack = timing.acknowledge_vblank().expect("pending VBlank");
        assert_eq!(ack.frame_counter, 1);
        assert_eq!(ack.checksum, 0xA5);
        assert!(!timing.vblank_pending());
        assert!(timing.acknowledge_vblank().is_none());
    }

    #[test]
    fn repeated_scanout_before_wait_sets_overrun_but_keeps_latest_frame() {
        let mut timing = VideoTiming::default();
        assert!(!timing.complete_scanout(0x11).vblank_overrun);
        let second = timing.complete_scanout(0x22);
        assert!(second.vblank_overrun);
        assert_eq!(second.frame_after, 2);
        let ack = timing.acknowledge_vblank().expect("latched VBlank");
        assert_eq!(ack.frame_counter, 2);
        assert_eq!(ack.checksum, 0x22);
    }
}
