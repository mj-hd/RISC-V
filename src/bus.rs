const RAM_SIZE: usize = 0x4000;

pub struct Bus {
    ram: [u8; RAM_SIZE], // TODO: RAMサイズ後で調べる
}

impl Bus {
    pub fn new() -> Self {
        Self { ram: [0; RAM_SIZE] }
    }

    pub fn read8(&self, addr: u32) -> u8 {
        self.ram[addr as usize]
    }

    pub fn write8(&mut self, addr: u32, val: u8) {
        self.ram[addr as usize] = val
    }

    pub fn read16(&self, addr: u32) -> u16 {
        let low = self.read8(addr) as u16;
        let high = self.read8(addr + 1) as u16;

        low | (high << 8)
    }

    pub fn write16(&mut self, addr: u32, val: u16) {
        self.ram[addr as usize] = val as u8;
        self.ram[(addr + 1) as usize] = (val >> 8) as u8;
    }

    pub fn read32(&self, addr: u32) -> u32 {
        let lowest = self.read8(addr) as u32;
        let lower = self.read8(addr + 1) as u32;
        let higher = self.read8(addr + 2) as u32;
        let highest = self.read8(addr + 3) as u32;

        lowest | (lower << 8) | (higher << 16) | (highest << 24)
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        self.ram[addr as usize] = val as u8;
        self.ram[(addr + 1) as usize] = (val >> 8) as u8;
        self.ram[(addr + 2) as usize] = (val >> 16) as u8;
        self.ram[(addr + 3) as usize] = (val >> 24) as u8;
    }
}
