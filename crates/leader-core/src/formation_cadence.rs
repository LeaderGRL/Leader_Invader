#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormationCadenceEvent {
    pub alive: u8,
    pub divisor: u8,
    pub before: u8,
    pub after: u8,
    pub tick: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormationCadence {
    counter: u8,
}

impl FormationCadence {
    #[must_use]
    pub const fn counter(self) -> u8 {
        self.counter
    }

    #[must_use]
    pub const fn divisor_for_alive(alive: u8) -> u8 {
        match alive {
            25..=u8::MAX => 3,
            13..=24 => 2,
            _ => 1,
        }
    }

    pub fn clock(&mut self, alive: u8) -> FormationCadenceEvent {
        let divisor = Self::divisor_for_alive(alive);
        let before = self.counter;
        let next = before.saturating_add(1);
        let tick = next >= divisor;
        self.counter = if tick { 0 } else { next };
        FormationCadenceEvent {
            alive,
            divisor,
            before,
            after: self.counter,
            tick,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_formation_ticks_every_third_clock() {
        let mut cadence = FormationCadence::default();
        assert!(!cadence.clock(32).tick);
        assert!(!cadence.clock(32).tick);
        let tick = cadence.clock(32);
        assert!(tick.tick);
        assert_eq!(tick.before, 2);
        assert_eq!(tick.after, 0);
        assert_eq!(tick.divisor, 3);
    }

    #[test]
    fn cadence_accelerates_as_formation_thins() {
        assert_eq!(FormationCadence::divisor_for_alive(32), 3);
        assert_eq!(FormationCadence::divisor_for_alive(24), 2);
        assert_eq!(FormationCadence::divisor_for_alive(13), 2);
        assert_eq!(FormationCadence::divisor_for_alive(12), 1);
        assert_eq!(FormationCadence::divisor_for_alive(1), 1);
    }

    #[test]
    fn low_population_ticks_every_clock() {
        let mut cadence = FormationCadence::default();
        for _ in 0..8 {
            let event = cadence.clock(8);
            assert!(event.tick);
            assert_eq!(event.after, 0);
        }
    }
}
