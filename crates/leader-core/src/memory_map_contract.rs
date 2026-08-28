use crate::{
    memory_map::{
        mmio_port, owner, MemoryOwner, MmioAccess, DEVICE_CMD, MMIO_BASE, SHIFT_DATA,
        SHIFT_OFFSET, SHIFT_RESULT,
    },
    BusAddressSource, BusDataSource, BusTransactionKind, MatchTrace,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryMapValidation {
    pub addressed_transactions: usize,
    pub rom: usize,
    pub ram: usize,
    pub vram: usize,
    pub mmio: usize,
    pub shift_data_writes: usize,
    pub shift_offset_writes: usize,
    pub shift_result_reads: usize,
}

fn require_mmio_access(address: u16, kind: BusTransactionKind) -> Result<(), String> {
    let port = mmio_port(address)
        .ok_or_else(|| format!("native bus transaction targets undeclared MMIO port {address:04X}"))?;
    let allowed = match kind {
        BusTransactionKind::Read => port.access.allows_read(),
        BusTransactionKind::Write => port.access.allows_write(),
        BusTransactionKind::Input => port.access.allows_input(),
        BusTransactionKind::Fetch | BusTransactionKind::Dma | BusTransactionKind::Scanout => false,
    };
    if !allowed {
        return Err(format!(
            "MMIO access direction is invalid for {} at {address:04X}: policy={:?} kind={:?}",
            port.name, port.access, kind
        ));
    }
    Ok(())
}

pub fn validate_memory_map_contract(trace: &MatchTrace) -> Result<MemoryMapValidation, String> {
    let mut validation = MemoryMapValidation::default();

    for event in &trace.bus_transactions {
        let Some(address) = event.address else {
            continue;
        };
        let region = owner(address);
        if region == MemoryOwner::Unmapped {
            return Err(format!(
                "native bus transaction targets unmapped address {address:04X} at frame={} ordinal={} control={}",
                event.frame, event.ordinal, event.control
            ));
        }

        validation.addressed_transactions += 1;
        match region {
            MemoryOwner::Rom => validation.rom += 1,
            MemoryOwner::Ram => validation.ram += 1,
            MemoryOwner::Vram => validation.vram += 1,
            MemoryOwner::Mmio => validation.mmio += 1,
            MemoryOwner::Unmapped => unreachable!(),
        }

        let expected_read_source = match region {
            MemoryOwner::Rom => BusDataSource::Rom,
            MemoryOwner::Ram => BusDataSource::Ram,
            MemoryOwner::Vram => BusDataSource::Vram,
            MemoryOwner::Mmio => BusDataSource::Device,
            MemoryOwner::Unmapped => unreachable!(),
        };

        match event.kind {
            BusTransactionKind::Fetch => {
                if region != MemoryOwner::Rom
                    || event.data_source != BusDataSource::Rom
                    || event.address_source != BusAddressSource::ProgramCounter
                {
                    return Err(format!(
                        "fetch authority is invalid at {address:04X}: owner={region:?} address={:?} data={:?}",
                        event.address_source, event.data_source
                    ));
                }
            }
            BusTransactionKind::Read => {
                if event.address_source != BusAddressSource::Cpu {
                    return Err(format!(
                        "read address driver disagrees at {address:04X}: expected=Cpu actual={:?}",
                        event.address_source
                    ));
                }
                if event.data_source != expected_read_source {
                    return Err(format!(
                        "read source disagrees with memory map at {address:04X}: owner={region:?} expected={expected_read_source:?} actual={:?}",
                        event.data_source
                    ));
                }
                if region == MemoryOwner::Mmio {
                    require_mmio_access(address, event.kind)?;
                }
                if address == SHIFT_RESULT {
                    validation.shift_result_reads += 1;
                }
            }
            BusTransactionKind::Write => {
                if event.address_source != BusAddressSource::Cpu {
                    return Err(format!(
                        "write address driver disagrees at {address:04X}: expected=Cpu actual={:?}",
                        event.address_source
                    ));
                }
                if event.data_source != BusDataSource::Cpu {
                    return Err(format!(
                        "write at {address:04X} is not CPU-driven: owner={region:?} source={:?}",
                        event.data_source
                    ));
                }
                if region == MemoryOwner::Rom {
                    return Err(format!("write targets read-only ROM at {address:04X}"));
                }
                if region == MemoryOwner::Mmio {
                    require_mmio_access(address, event.kind)?;
                }
                if address == SHIFT_DATA {
                    validation.shift_data_writes += 1;
                } else if address == SHIFT_OFFSET {
                    validation.shift_offset_writes += 1;
                }
            }
            BusTransactionKind::Input => {
                if region != MemoryOwner::Mmio
                    || event.address_source != BusAddressSource::None
                    || event.data_source != BusDataSource::Device
                {
                    return Err(format!(
                        "input authority is invalid at {address:04X}: owner={region:?} address={:?} data={:?}",
                        event.address_source, event.data_source
                    ));
                }
                require_mmio_access(address, event.kind)?;
            }
            BusTransactionKind::Dma | BusTransactionKind::Scanout => {
                if region != MemoryOwner::Vram
                    || event.address_source != BusAddressSource::Dma
                    || event.data_source != BusDataSource::Vram
                {
                    return Err(format!(
                        "DMA/scanout authority is invalid at {address:04X}: owner={region:?} address={:?} data={:?}",
                        event.address_source, event.data_source
                    ));
                }
            }
        }
    }

    if validation.rom == 0
        || validation.ram == 0
        || validation.vram == 0
        || validation.mmio == 0
    {
        return Err(format!(
            "complete trace does not exercise every mapped owner: rom={} ram={} vram={} mmio={}",
            validation.rom, validation.ram, validation.vram, validation.mmio
        ));
    }
    if validation.shift_data_writes < 2
        || validation.shift_offset_writes == 0
        || validation.shift_result_reads == 0
    {
        return Err(format!(
            "complete trace does not exercise the directional shift-register MMIO contract: data_writes={} offset_writes={} result_reads={}",
            validation.shift_data_writes,
            validation.shift_offset_writes,
            validation.shift_result_reads
        ));
    }

    Ok(validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn complete_match_obeys_canonical_memory_ownership() {
        let trace = Machine::run_match("m3-memory-map-contract", 5000);
        let validation = validate_memory_map_contract(&trace).expect("valid memory ownership");
        assert!(validation.addressed_transactions > 0);
        assert!(validation.rom > 0);
        assert!(validation.ram > 0);
        assert!(validation.vram > 0);
        assert!(validation.mmio > 0);
        assert!(validation.shift_data_writes >= 2);
        assert!(validation.shift_offset_writes > 0);
        assert!(validation.shift_result_reads > 0);
    }

    #[test]
    fn unmapped_transaction_is_rejected() {
        let mut trace = Machine::run_match("m3-memory-map-unmapped", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Read)
            .expect("read event");
        event.address = Some(0x9000);
        event.address_source = BusAddressSource::Cpu;
        let error = validate_memory_map_contract(&trace).expect_err("unmapped bus access must fail");
        assert!(error.contains("unmapped address 9000"));
    }

    #[test]
    fn undeclared_mmio_port_is_rejected() {
        let mut trace = Machine::run_match("m3-memory-map-undeclared-mmio", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Read)
            .expect("read event");
        event.address = Some(MMIO_BASE + 1);
        event.address_source = BusAddressSource::Cpu;
        event.data_source = BusDataSource::Device;
        let error = validate_memory_map_contract(&trace).expect_err("undeclared MMIO must fail");
        assert!(error.contains("undeclared MMIO port"));
    }

    #[test]
    fn wrong_region_data_source_is_rejected() {
        let mut trace = Machine::run_match("m3-memory-map-source", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| {
                event.kind == BusTransactionKind::Read
                    && event.address.is_some_and(|address| owner(address) == MemoryOwner::Ram)
            })
            .expect("RAM read event");
        event.data_source = BusDataSource::Rom;
        let error = validate_memory_map_contract(&trace).expect_err("wrong RAM source must fail");
        assert!(error.contains("read source disagrees"));
    }

    #[test]
    fn wrong_address_driver_is_rejected() {
        let mut trace = Machine::run_match("m3-memory-map-address-driver", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Read)
            .expect("read event");
        event.address_source = BusAddressSource::Dma;
        let error =
            validate_memory_map_contract(&trace).expect_err("wrong read address driver must fail");
        assert!(error.contains("read address driver disagrees"));
    }

    #[test]
    fn fetch_outside_rom_is_rejected() {
        let mut trace = Machine::run_match("m3-memory-map-fetch", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Fetch)
            .expect("fetch event");
        event.address = Some(crate::memory_map::RAM_BASE);
        let error = validate_memory_map_contract(&trace).expect_err("RAM fetch must fail");
        assert!(error.contains("fetch authority is invalid"));
    }

    #[test]
    fn write_to_rom_is_rejected() {
        let mut trace = Machine::run_match("m3-memory-map-rom-write", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Write)
            .expect("write event");
        event.address = Some(crate::memory_map::ROM_BASE);
        event.address_source = BusAddressSource::Cpu;
        event.data_source = BusDataSource::Cpu;
        let error = validate_memory_map_contract(&trace).expect_err("ROM write must fail");
        assert!(error.contains("read-only ROM"));
    }

    #[test]
    fn shift_result_write_is_rejected_by_port_policy() {
        let mut trace = Machine::run_match("m3-memory-map-shift-result-write", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Write)
            .expect("write event");
        event.address = Some(SHIFT_RESULT);
        event.address_source = BusAddressSource::Cpu;
        event.data_source = BusDataSource::Cpu;
        let error =
            validate_memory_map_contract(&trace).expect_err("SHIFT_RESULT write must fail");
        assert!(error.contains("MMIO access direction is invalid"));
        assert!(error.contains("shift_result"));
    }

    #[test]
    fn shift_data_read_is_rejected_by_port_policy() {
        let mut trace = Machine::run_match("m3-memory-map-shift-data-read", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Read)
            .expect("read event");
        event.address = Some(SHIFT_DATA);
        event.address_source = BusAddressSource::Cpu;
        event.data_source = BusDataSource::Device;
        let error = validate_memory_map_contract(&trace).expect_err("SHIFT_DATA read must fail");
        assert!(error.contains("MMIO access direction is invalid"));
        assert!(error.contains("shift_data"));
    }

    #[test]
    fn device_command_read_is_rejected_by_port_policy() {
        let mut trace = Machine::run_match("m3-memory-map-device-cmd-read", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Read)
            .expect("read event");
        event.address = Some(DEVICE_CMD);
        event.address_source = BusAddressSource::Cpu;
        event.data_source = BusDataSource::Device;
        let error = validate_memory_map_contract(&trace).expect_err("DEVICE_CMD read must fail");
        assert!(error.contains("MMIO access direction is invalid"));
        assert!(error.contains("device_cmd"));
    }

    #[test]
    fn input_event_on_non_input_port_is_rejected() {
        let mut trace = Machine::run_match("m3-memory-map-input-port", 5000);
        let event = trace
            .bus_transactions
            .iter_mut()
            .find(|event| event.kind == BusTransactionKind::Input)
            .expect("input event");
        event.address = Some(SHIFT_RESULT);
        let error = validate_memory_map_contract(&trace).expect_err("wrong input port must fail");
        assert!(error.contains("MMIO access direction is invalid"));
        assert!(error.contains("shift_result"));
    }

    #[test]
    fn declared_mmio_policies_match_expected_core_ports() {
        assert_eq!(mmio_port(SHIFT_DATA).unwrap().access, MmioAccess::WriteOnly);
        assert_eq!(mmio_port(SHIFT_RESULT).unwrap().access, MmioAccess::ReadOnly);
        assert_eq!(mmio_port(DEVICE_CMD).unwrap().access, MmioAccess::WriteOnly);
    }
}
