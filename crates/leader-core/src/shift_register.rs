#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShiftRegister16 {
    value: u16,
    offset: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftRegisterEventKind {
    DataWrite {
        before: u16,
        after: u16,
        input: u8,
    },
    OffsetWrite {
        before: u8,
        after: u8,
        input: u8,
    },
    Read {
        value: u16,
        offset: u8,
        result: u8,
    },
}

impl ShiftRegisterEventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DataWrite { .. } => "data_write",
            Self::OffsetWrite { .. } => "offset_write",
            Self::Read { .. } => "read",
        }
    }
}

impl ShiftRegister16 {
    #[must_use]
    pub const fn value(self) -> u16 {
        self.value
    }

    #[must_use]
    pub const fn offset(self) -> u8 {
        self.offset
    }

    pub fn write_data(&mut self, input: u8) -> ShiftRegisterEventKind {
        let before = self.value;
        self.value = (u16::from(input) << 8) | (before >> 8);
        ShiftRegisterEventKind::DataWrite {
            before,
            after: self.value,
            input,
        }
    }

    pub fn write_offset(&mut self, input: u8) -> ShiftRegisterEventKind {
        let before = self.offset;
        self.offset = input & 0x07;
        ShiftRegisterEventKind::OffsetWrite {
            before,
            after: self.offset,
            input,
        }
    }

    #[must_use]
    pub const fn read(&self) -> u8 {
        ((self.value >> (8 - self.offset)) & 0x00ff) as u8
    }

    #[must_use]
    pub const fn read_event(self) -> ShiftRegisterEventKind {
        ShiftRegisterEventKind::Read {
            value: self.value,
            offset: self.offset,
            result: self.read(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_byte_load_matches_arcade_shift_register_order() {
        let mut device = ShiftRegister16::default();
        assert_eq!(
            device.write_data(0x12),
            ShiftRegisterEventKind::DataWrite {
                before: 0x0000,
                after: 0x1200,
                input: 0x12,
            }
        );
        assert_eq!(
            device.write_data(0x34),
            ShiftRegisterEventKind::DataWrite {
                before: 0x1200,
                after: 0x3412,
                input: 0x34,
            }
        );
        assert_eq!(device.value(), 0x3412);
    }

    #[test]
    fn offset_is_three_bits_and_read_matches_hardware_window() {
        let mut device = ShiftRegister16::default();
        device.write_data(0x12);
        device.write_data(0x34);
        device.write_offset(0x0b);
        assert_eq!(device.offset(), 3);
        assert_eq!(device.read(), 0xa0);
        assert_eq!(
            device.read_event(),
            ShiftRegisterEventKind::Read {
                value: 0x3412,
                offset: 3,
                result: 0xa0,
            }
        );
    }

    #[test]
    fn every_offset_matches_direct_reference_expression() {
        for offset in 0..8 {
            let mut device = ShiftRegister16::default();
            device.write_data(0xcd);
            device.write_data(0xab);
            device.write_offset(offset);
            assert_eq!(
                device.read(),
                ((0xabcd_u16 >> (8 - offset)) & 0xff) as u8
            );
        }
    }
}
