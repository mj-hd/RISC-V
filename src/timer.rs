use crate::utils::ApplyByte;
use anyhow::Result;

pub struct Clint {
    pub msip: u32,
    mtimecmp: u64,
    mtime: u64,
}

impl Clint {
    pub fn new() -> Self {
        Self {
            msip: 0,
            mtimecmp: 0,
            mtime: 0,
        }
    }

    pub fn tick(&mut self) -> Result<()> {
        self.mtime = self.mtime.wrapping_add(1);

        if self.mtime >= self.mtimecmp {
            self.msip |= 0x80;
        }

        Ok(())
    }

    pub fn read8(&self, addr: u32) -> u8 {
        match addr {
            0x0000..=0x0003 => (self.msip >> (addr * 8)) as u8,
            0x4000..=0x4007 => (self.mtimecmp >> (addr - 0x4000) * 8) as u8,
            0xBFF8..=0xBFFF => (self.mtime >> ((addr - 0xBFF8) * 8)) as u8,
            _ => 0,
        }
    }

    pub fn write8(&mut self, addr: u32, val: u8) {
        match addr {
            0x0000..=0x0003 => self.msip = ApplyByte::apply_byte(self.msip, val, addr as usize),
            0x4000..=0x4007 => {
                self.mtimecmp = ApplyByte::apply_byte(self.mtimecmp, val, (addr - 0x4000) as usize);
                self.msip = 0x00;
            }
            0xBFF8..=0xBFFF => {
                self.mtime = ApplyByte::apply_byte(self.mtime, val, (addr - 0xBFF8) as usize)
            }
            _ => {}
        }
    }
}
