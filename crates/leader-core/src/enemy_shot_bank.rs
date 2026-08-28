use crate::game::Projectile;

pub const ENEMY_SHOT_SLOTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnemyShotBank {
    slots: [Option<Projectile>; ENEMY_SHOT_SLOTS],
    cooldown: u8,
    next_slot: u8,
}

impl Default for EnemyShotBank {
    fn default() -> Self {
        Self {
            slots: [None; ENEMY_SHOT_SLOTS],
            cooldown: 0,
            next_slot: 0,
        }
    }
}

impl EnemyShotBank {
    #[must_use]
    pub const fn slots(&self) -> &[Option<Projectile>; ENEMY_SHOT_SLOTS] {
        &self.slots
    }

    #[must_use]
    pub const fn slot(&self, index: usize) -> Option<Projectile> {
        self.slots[index]
    }

    #[must_use]
    pub const fn cooldown(&self) -> u8 {
        self.cooldown
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn clock_cooldown(&mut self) -> (u8, u8) {
        let before = self.cooldown;
        self.cooldown = self.cooldown.saturating_sub(1);
        (before, self.cooldown)
    }

    pub fn set_cooldown(&mut self, value: u8) {
        self.cooldown = value;
    }

    pub fn update(&mut self, slot: usize, projectile: Projectile) {
        self.slots[slot] = Some(projectile);
    }

    pub fn clear(&mut self, slot: usize) -> Option<Projectile> {
        self.slots[slot].take()
    }

    pub fn spawn(&mut self, projectile: Projectile) -> Option<usize> {
        if self.cooldown != 0 || self.active_count() == ENEMY_SHOT_SLOTS {
            return None;
        }
        for offset in 0..ENEMY_SHOT_SLOTS {
            let slot = (usize::from(self.next_slot) + offset) % ENEMY_SHOT_SLOTS;
            if self.slots[slot].is_none() {
                self.slots[slot] = Some(projectile);
                self.next_slot = ((slot + 1) % ENEMY_SHOT_SLOTS) as u8;
                return Some(slot);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(x: i16) -> Projectile {
        Projectile { x, y: 10 }
    }

    #[test]
    fn round_robin_allocates_all_three_slots() {
        let mut bank = EnemyShotBank::default();
        assert_eq!(bank.spawn(shot(1)), Some(0));
        assert_eq!(bank.spawn(shot(2)), Some(1));
        assert_eq!(bank.spawn(shot(3)), Some(2));
        assert_eq!(bank.active_count(), 3);
        assert_eq!(bank.spawn(shot(4)), None);
    }

    #[test]
    fn cleared_slot_is_reused_in_round_robin_order() {
        let mut bank = EnemyShotBank::default();
        assert_eq!(bank.spawn(shot(1)), Some(0));
        assert_eq!(bank.spawn(shot(2)), Some(1));
        assert_eq!(bank.spawn(shot(3)), Some(2));
        assert_eq!(bank.clear(1), Some(shot(2)));
        assert_eq!(bank.spawn(shot(4)), Some(1));
    }

    #[test]
    fn cooldown_blocks_spawn_until_clocked_to_zero() {
        let mut bank = EnemyShotBank::default();
        bank.set_cooldown(2);
        assert_eq!(bank.spawn(shot(1)), None);
        assert_eq!(bank.clock_cooldown(), (2, 1));
        assert_eq!(bank.spawn(shot(1)), None);
        assert_eq!(bank.clock_cooldown(), (1, 0));
        assert_eq!(bank.spawn(shot(1)), Some(0));
    }
}
