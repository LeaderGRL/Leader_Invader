use crate::{
    BusTransactionKind, MatchTrace, ShieldBank, SHIELD_BYTES_PER, SHIELD_COUNT, SHIELD_RAM_BASE,
    SHIELD_TOTAL_BYTES,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShieldValidation {
    pub damages: usize,
    pub player_damages: usize,
    pub enemy_damages: usize,
    pub shields_damaged: usize,
    pub pixels_before: u32,
    pub pixels_after: u32,
}

pub fn validate_shield_bank_contract(trace: &MatchTrace) -> Result<ShieldValidation, String> {
    let initial = ShieldBank::default();
    let mut model = *initial.bytes();
    let mut validation = ShieldValidation {
        pixels_before: initial.remaining_pixels(),
        ..ShieldValidation::default()
    };
    let mut touched = [false; SHIELD_COUNT];

    for transaction in trace.bus_transactions.iter().filter(|transaction| {
        transaction.kind == BusTransactionKind::Write
            && transaction.address.is_some_and(|address| {
                (SHIELD_RAM_BASE..SHIELD_RAM_BASE + SHIELD_TOTAL_BYTES as u16).contains(&address)
            })
    }) {
        let address = transaction.address.expect("filtered shield address");
        let index = usize::from(address - SHIELD_RAM_BASE);
        let after = transaction.data.ok_or_else(|| {
            format!(
                "shield RAM write has no data at frame={} address={address:04X}",
                transaction.frame
            )
        })?;
        let before = model[index];
        let changed = before ^ after;

        if changed.count_ones() != 1 || after & !before != 0 {
            return Err(format!(
                "shield write must clear exactly one existing bit at frame={} address={address:04X}: before={before:02X} after={after:02X}",
                transaction.frame
            ));
        }

        match transaction.control {
            "SHIELD_DAMAGE_PLAYER" => validation.player_damages += 1,
            "SHIELD_DAMAGE_ENEMY" => validation.enemy_damages += 1,
            other => {
                return Err(format!(
                    "shield RAM write has invalid control {other} at frame={} address={address:04X}",
                    transaction.frame
                ));
            }
        }

        model[index] = after;
        touched[index / SHIELD_BYTES_PER] = true;
        validation.damages += 1;
    }

    validation.shields_damaged = touched.into_iter().filter(|value| *value).count();
    validation.pixels_after = model.iter().map(|byte| byte.count_ones()).sum();

    if validation.damages == 0 {
        return Err("shield bank receives no projectile damage".to_owned());
    }
    if validation.pixels_after + validation.damages as u32 != validation.pixels_before {
        return Err(format!(
            "shield population does not match one-bit damage count: before={} damages={} after={}",
            validation.pixels_before, validation.damages, validation.pixels_after
        ));
    }
    if validation.shields_damaged < 2 {
        return Err(format!(
            "shield replay exercises too little of the bank: {} shields damaged",
            validation.shields_damaged
        ));
    }

    Ok(validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn complete_match_has_causal_bit_clear_shield_writes() {
        let trace = Machine::run_match("m3-shield-contract", 5000);
        let validation = validate_shield_bank_contract(&trace).expect("valid shield RAM replay");
        assert!(validation.damages > 0);
        assert!(validation.player_damages + validation.enemy_damages == validation.damages);
        assert!(validation.shields_damaged >= 2);
        assert!(validation.pixels_after < validation.pixels_before);
    }

    #[test]
    fn setting_a_bit_is_rejected() {
        let mut trace = Machine::run_match("m3-shield-set-negative", 5000);
        let write = trace
            .bus_transactions
            .iter_mut()
            .find(|transaction| matches!(
                transaction.control,
                "SHIELD_DAMAGE_PLAYER" | "SHIELD_DAMAGE_ENEMY"
            ))
            .expect("shield damage write");
        write.data = Some(0xFF);
        let error = validate_shield_bank_contract(&trace).expect_err("setting bits must fail");
        assert!(error.contains("clear exactly one existing bit"));
    }

    #[test]
    fn clearing_multiple_bits_is_rejected() {
        let mut trace = Machine::run_match("m3-shield-multibit-negative", 5000);
        let write = trace
            .bus_transactions
            .iter_mut()
            .find(|transaction| matches!(
                transaction.control,
                "SHIELD_DAMAGE_PLAYER" | "SHIELD_DAMAGE_ENEMY"
            ))
            .expect("shield damage write");
        write.data = Some(0);
        let error = validate_shield_bank_contract(&trace).expect_err("multi-bit clear must fail");
        assert!(error.contains("clear exactly one existing bit"));
    }

    #[test]
    fn wrong_damage_source_is_rejected() {
        let mut trace = Machine::run_match("m3-shield-control-negative", 5000);
        let write = trace
            .bus_transactions
            .iter_mut()
            .find(|transaction| matches!(
                transaction.control,
                "SHIELD_DAMAGE_PLAYER" | "SHIELD_DAMAGE_ENEMY"
            ))
            .expect("shield damage write");
        write.control = "CPU_WRITE";
        let error = validate_shield_bank_contract(&trace).expect_err("wrong source must fail");
        assert!(error.contains("invalid control"));
    }
}
