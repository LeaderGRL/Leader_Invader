use crate::{
    enemy_shot_bank::ENEMY_SHOT_SLOTS, program::RAM_BASE, BusTransactionKind, MatchTrace,
    ProjectileSnapshot,
};

pub const ENEMY_SHOT_RAM_BASE: u16 = RAM_BASE + 0x20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnemyShotValidation {
    pub transitions: usize,
    pub ram_writes: usize,
    pub spawns: usize,
    pub moves: usize,
    pub clears: usize,
    pub shield_clears: usize,
    pub max_active: usize,
    pub slots_used: usize,
}

#[must_use]
pub const fn enemy_shot_ram(slot: usize, component: u16) -> u16 {
    ENEMY_SHOT_RAM_BASE + slot as u16 * 3 + component
}

pub fn validate_enemy_shot_bank_contract(
    trace: &MatchTrace,
) -> Result<EnemyShotValidation, String> {
    if trace.frames.len() < 2 {
        return Err("native trace contains fewer than two frame snapshots".to_owned());
    }

    if trace.frames[0].enemy_shots.iter().any(Option::is_some) {
        return Err("enemy-shot bank is not empty at reset snapshot".to_owned());
    }

    let mut validation = EnemyShotValidation::default();
    let mut used = [false; ENEMY_SHOT_SLOTS];

    for frame in &trace.frames {
        let active = frame.enemy_shots.iter().flatten().count();
        validation.max_active = validation.max_active.max(active);
        for (slot, shot) in frame.enemy_shots.iter().enumerate() {
            used[slot] |= shot.is_some();
        }
    }

    for pair in trace.frames.windows(2) {
        let before = &pair[0];
        let after = &pair[1];
        if after.frame <= before.frame {
            continue;
        }

        for slot in 0..ENEMY_SHOT_SLOTS {
            let mut x = before.enemy_shots[slot].map_or(0, |shot| shot.x as u8);
            let mut y = before.enemy_shots[slot].map_or(0, |shot| shot.y as u8);
            let mut active = before.enemy_shots[slot].is_some();
            let mut writes = 0usize;
            let mut armed_count = 0usize;
            let mut clear_count = 0usize;
            let mut shield_clear_count = 0usize;
            let mut move_count = 0usize;

            for transaction in trace.bus_transactions.iter().filter(|transaction| {
                transaction.frame >= before.frame
                    && transaction.frame < after.frame
                    && transaction.kind == BusTransactionKind::Write
                    && transaction.address.is_some_and(|address| {
                        (enemy_shot_ram(slot, 0)..=enemy_shot_ram(slot, 2)).contains(&address)
                    })
            }) {
                let address = transaction.address.expect("filtered address");
                let data = transaction.data.ok_or_else(|| {
                    format!(
                        "enemy-shot RAM write has no data at frame={}",
                        transaction.frame
                    )
                })?;
                match address - enemy_shot_ram(slot, 0) {
                    0 => {
                        if transaction.control != "ENEMY_SHOT_X_WRITE" {
                            return Err(format!(
                                "slot {slot} X write has wrong control {} at frame={}",
                                transaction.control, transaction.frame
                            ));
                        }
                        x = data;
                    }
                    1 => {
                        if transaction.control != "ENEMY_SHOT_Y_WRITE" {
                            return Err(format!(
                                "slot {slot} Y write has wrong control {} at frame={}",
                                transaction.control, transaction.frame
                            ));
                        }
                        if active {
                            move_count += 1;
                        }
                        y = data;
                    }
                    2 => match (data, transaction.control) {
                        (1, "ENEMY_SHOT_ARM") => {
                            if active {
                                return Err(format!(
                                    "slot {slot} is armed while already active at frame={}",
                                    transaction.frame
                                ));
                            }
                            active = true;
                            armed_count += 1;
                        }
                        (0, "ENEMY_SHOT_HIT" | "ENEMY_SHOT_CLEAR") => {
                            if !active {
                                return Err(format!(
                                    "slot {slot} is cleared while inactive at frame={}",
                                    transaction.frame
                                ));
                            }
                            active = false;
                            clear_count += 1;
                        }
                        (0, "ENEMY_SHOT_SHIELD_CLEAR") => {
                            if !active {
                                return Err(format!(
                                    "slot {slot} is shield-cleared while inactive at frame={}",
                                    transaction.frame
                                ));
                            }
                            let has_immediate_shield_damage = trace.bus_transactions.iter().any(|candidate| {
                                candidate.frame == transaction.frame
                                    && candidate.pc == transaction.pc
                                    && candidate.kind == BusTransactionKind::Write
                                    && candidate.control == "SHIELD_DAMAGE_ENEMY"
                                    && candidate.ordinal.saturating_add(1) == transaction.ordinal
                            });
                            if !has_immediate_shield_damage {
                                return Err(format!(
                                    "slot {slot} shield clear lacks immediately preceding SHIELD_DAMAGE_ENEMY authority at frame={} ordinal={}",
                                    transaction.frame, transaction.ordinal
                                ));
                            }
                            active = false;
                            clear_count += 1;
                            shield_clear_count += 1;
                        }
                        _ => {
                            return Err(format!(
                                "slot {slot} ACTIVE write is invalid: data={data} control={} frame={}",
                                transaction.control, transaction.frame
                            ));
                        }
                    },
                    _ => unreachable!(),
                }
                writes += 1;
            }

            let expected = active.then_some(ProjectileSnapshot {
                x: i16::from(x),
                y: i16::from(y),
            });
            if expected != after.enemy_shots[slot] {
                return Err(format!(
                    "enemy-shot slot {slot} RAM replay diverges at frame {} -> {}: expected {:?}, snapshot {:?}",
                    before.frame, after.frame, expected, after.enemy_shots[slot]
                ));
            }

            if before.enemy_shots[slot] != after.enemy_shots[slot] {
                validation.transitions += 1;
            }
            validation.ram_writes += writes;
            validation.spawns += armed_count;
            validation.clears += clear_count;
            validation.shield_clears += shield_clear_count;
            validation.moves += move_count;
        }
    }

    validation.slots_used = used.into_iter().filter(|used| *used).count();
    if validation.spawns == 0 || validation.moves == 0 || validation.clears == 0 {
        return Err(format!(
            "enemy-shot trace does not exercise full lifecycle: spawn={} move={} clear={}",
            validation.spawns, validation.moves, validation.clears
        ));
    }
    if validation.max_active < 2 {
        return Err("enemy-shot bank never exercises concurrent projectiles".to_owned());
    }
    if validation.slots_used != ENEMY_SHOT_SLOTS {
        return Err(format!(
            "enemy-shot bank does not exercise all slots: {}/{}",
            validation.slots_used, ENEMY_SHOT_SLOTS
        ));
    }

    Ok(validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn complete_match_has_three_slot_ram_authority() {
        let trace = Machine::run_match("m3-enemy-shot-contract", 5000);
        let validation =
            validate_enemy_shot_bank_contract(&trace).expect("valid enemy-shot bank trace");
        assert_eq!(validation.slots_used, ENEMY_SHOT_SLOTS);
        assert!(validation.max_active >= 2);
        assert!(validation.spawns >= ENEMY_SHOT_SLOTS);
        assert!(validation.moves > 0);
        assert!(validation.clears > 0);
        assert!(validation.shield_clears > 0);
    }

    #[test]
    fn corrupting_slot_snapshot_is_detected() {
        let mut trace = Machine::run_match("m3-enemy-shot-state-negative", 5000);
        let frame = trace
            .frames
            .iter_mut()
            .find(|frame| frame.enemy_shots.iter().any(Option::is_some))
            .expect("active enemy shot");
        let shot = frame
            .enemy_shots
            .iter_mut()
            .find_map(Option::as_mut)
            .expect("active shot");
        shot.y = shot.y.saturating_add(1);
        let error = validate_enemy_shot_bank_contract(&trace)
            .expect_err("corrupt snapshot must fail");
        assert!(error.contains("RAM replay diverges"));
    }

    #[test]
    fn removing_slot_ram_write_is_detected() {
        let mut trace = Machine::run_match("m3-enemy-shot-bus-negative", 5000);
        let index = trace
            .bus_transactions
            .iter()
            .position(|transaction| transaction.control == "ENEMY_SHOT_ARM")
            .expect("enemy-shot arm write");
        trace.bus_transactions.remove(index);
        let error = validate_enemy_shot_bank_contract(&trace)
            .expect_err("missing authority write must fail");
        assert!(error.contains("RAM replay diverges"));
    }

    #[test]
    fn shield_clear_without_immediate_damage_authority_is_detected() {
        let mut trace = Machine::run_match("m3-enemy-shot-shield-negative", 5000);
        let clear = trace
            .bus_transactions
            .iter()
            .find(|transaction| transaction.control == "ENEMY_SHOT_SHIELD_CLEAR")
            .copied()
            .expect("shield-caused enemy shot clear");
        let index = trace
            .bus_transactions
            .iter()
            .position(|transaction| {
                transaction.frame == clear.frame
                    && transaction.pc == clear.pc
                    && transaction.control == "SHIELD_DAMAGE_ENEMY"
                    && transaction.ordinal.saturating_add(1) == clear.ordinal
            })
            .expect("immediately preceding shield damage write");
        trace.bus_transactions.remove(index);
        let error = validate_enemy_shot_bank_contract(&trace)
            .expect_err("orphan shield clear must fail");
        assert!(error.contains("lacks immediately preceding SHIELD_DAMAGE_ENEMY authority"));
    }
}
