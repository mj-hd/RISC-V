use anyhow::{bail, Result};

use crate::bus::Bus;

// Machineだけ対応する
#[derive(Clone, Copy, Debug)]
enum Mode {
    User = 0,
    Supervisor = 1,
    Reserved = 2,
    Machine = 3,
}

pub struct Cpu {
    // 汎用レジスタ
    xr: [u32; 32],
    pc: u32,

    // CSRレジスタ
    mstatus: u32,
    mie: u32,
    mtvec: u32,
    mscratch: u32,
    mepc: u32,
    mcause: u32,
    mtval: u32,
    mip: u32,

    mode: Mode,
    prev_mode: Mode,

    bus: Bus,
}

impl Cpu {
    pub fn new(bus: Bus) -> Self {
        Self {
            xr: [0; 32],
            pc: 0,
            bus,
            mstatus: 0,
            mie: 0,
            mtvec: 0,
            mscratch: 0,
            mepc: 0,
            mcause: 0,
            mtval: 0,
            mip: 0,
            mode: Mode::Machine,
            prev_mode: Mode::Machine,
        }
    }

    fn get_x(&self, i: usize) -> u32 {
        match i {
            0 => 0,
            x => self.xr[x],
        }
    }

    fn set_x(&mut self, i: usize, val: u32) {
        self.xr[i] = val
    }

    fn get_a0(&self) -> u32 {
        self.xr[10]
    }

    pub fn set_a0(&mut self, val: u32) {
        self.xr[10] = val;
    }

    fn get_a1(&self) -> u32 {
        self.xr[11]
    }

    pub fn set_a1(&mut self, val: u32) {
        self.xr[11] = val;
    }

    fn get_csr(&self, no: u16) -> u32 {
        match no {
            0x300 => self.mstatus,
            0x304 => self.mie,
            0x305 => self.mtvec,
            0x340 => self.mscratch,
            0x341 => self.mepc,
            0x342 => self.mcause,
            0x343 => self.mtval,
            0x344 => self.mip,
            _ => panic!("unknown csr no {:04X}", no),
        }
    }

    fn set_csr(&mut self, no: u16, val: u32) {
        match no {
            0x300 => {
                self.mstatus = val;
            }
            0x304 => {
                self.mie = val;
            }
            0x305 => {
                self.mtvec = val;
            }
            0x340 => {
                self.mscratch = val;
            }
            0x341 => {
                self.mepc = val;
            }
            0x342 => {
                self.mcause = val;
            }
            0x343 => {
                self.mtval = val;
            }
            0x344 => {
                self.mip = val;
            }
            _ => panic!("unknown csr no {:04X}", no),
        }
    }

    pub fn tick(&mut self) -> Result<()> {
        self.bus.tick()?;

        self.check_interrupt();

        self.pc = self.pc.wrapping_add(4);

        let ir = self.bus.read32(self.pc);

        self.do_mnemonic(ir)
    }

    fn check_interrupt(&mut self) {
        self.mip |= self.bus.clint.msip;

        if self.mstatus & 0b1000 > 0 {
            let it = self.mie & self.mip;
            if it > 0 {
                self.do_interrupt(it);
            }
        }
    }

    fn do_interrupt(&mut self, it: u32) {
        // 一旦、Machine Timer Interrupt Pendingだけ対応
        if it & 0x80 == 0 {
            return;
        }

        self.mtval = 0;
        self.mepc = self.pc;
        self.mcause = 0x8000_0007; // Interrupt: 1, Exception Code: 7 (Machine Timer Interrupt)
        self.mstatus = (self.mstatus & 0x08 << 4) | ((self.prev_mode as u32) << 11);

        self.pc = self.mtvec;
        self.prev_mode = Mode::Machine;
    }

    fn do_mnemonic(&mut self, ir: u32) -> Result<()> {
        let opecode = ir & 0x7F;
        match opecode {
            // 000系
            0b00_000_11 => self.load(Inst::from_i(ir)),
            0b01_000_11 => self.store(Inst::from_s(ir)),
            0b11_000_11 => self.branch(Inst::from_b(ir)),
            // 001系
            0b11_001_11 => self.jalr(Inst::from_i(ir)),
            // 011系
            0b00_011_11 => self.misc_mem(Inst::from_i(ir)),
            0b01_011_11 => self.amo(Inst::from_r(ir)),
            0b11_011_11 => self.jal(Inst::from_j(ir)),
            // 100系
            0b00_100_11 => self.opimm(Inst::from_i(ir)),
            0b01_100_11 => self.op(Inst::from_r(ir)),
            0b11_100_11 => self.system(Inst::from_i(ir)),
            // 101系
            0b00_101_11 => self.auipc(Inst::from_u(ir)),
            0b01_101_11 => self.lui(Inst::from_u(ir)),
            _ => bail!("unknown instruction {:08X}", ir),
        }
    }

    fn opimm(&mut self, inst: Inst) -> Result<()> {
        match inst {
            Inst {
                funct3: 0b000,
                rd,
                rs1,
                imm12,
                ..
            } => self.addi(rd, rs1, imm12),
            Inst {
                funct3: 0b001,
                funct7: 0b000000,
                rd,
                rs1,
                imm12,
                ..
            } => self.slli(rd, rs1, imm12),
            Inst {
                funct3: 0b010,
                rd,
                rs1,
                imm12,
                ..
            } => self.slti(rd, rs1, imm12),
            Inst {
                funct3: 0b011,
                rd,
                rs1,
                imm12,
                ..
            } => self.sltiu(rd, rs1, imm12),
            Inst {
                funct3: 0b100,
                rd,
                rs1,
                imm12,
                ..
            } => self.xori(rd, rs1, imm12),
            Inst {
                funct3: 0b101,
                funct7: 0b000000,
                rd,
                rs1,
                imm12,
                ..
            } => self.srli(rd, rs1, imm12),
            Inst {
                funct3: 0b101,
                funct7: 0b010000,
                rd,
                rs1,
                imm12,
                ..
            } => self.srai(rd, rs1, imm12),
            Inst {
                funct3: 0b110,
                rd,
                rs1,
                imm12,
                ..
            } => self.ori(rd, rs1, imm12),
            Inst {
                funct3: 0b111,
                rd,
                rs1,
                imm12,
                ..
            } => self.andi(rd, rs1, imm12),
            _ => bail!("unknown OP-IMM {:?}", inst),
        }
    }

    fn branch(&mut self, inst: Inst) -> Result<()> {
        match inst {
            Inst {
                funct3: 0b000,
                rs1,
                rs2,
                imm12,
                ..
            } => self.beq(rs1, rs2, imm12),
            Inst {
                funct3: 0b001,
                rs1,
                rs2,
                imm12,
                ..
            } => self.bne(rs1, rs2, imm12),
            Inst {
                funct3: 0b100,
                rs1,
                rs2,
                imm12,
                ..
            } => self.blt(rs1, rs2, imm12),
            Inst {
                funct3: 0b101,
                rs1,
                rs2,
                imm12,
                ..
            } => self.bge(rs1, rs2, imm12),
            Inst {
                funct3: 0b110,
                rs1,
                rs2,
                imm12,
                ..
            } => self.bltu(rs1, rs2, imm12),
            Inst {
                funct3: 0b111,
                rs1,
                rs2,
                imm12,
                ..
            } => self.bgeu(rs1, rs2, imm12),
            _ => bail!("unknown Branch {:?}", inst),
        }
    }

    fn load(&mut self, inst: Inst) -> Result<()> {
        match inst {
            Inst {
                funct3: 0b000,
                rd,
                rs1,
                imm12,
                ..
            } => self.lb(rd, rs1, imm12),
            Inst {
                funct3: 0b001,
                rd,
                rs1,
                imm12,
                ..
            } => self.lh(rd, rs1, imm12),
            Inst {
                funct3: 0b010,
                rd,
                rs1,
                imm12,
                ..
            } => self.lw(rd, rs1, imm12),
            Inst {
                funct3: 0b100,
                rd,
                rs1,
                imm12,
                ..
            } => self.lbu(rd, rs1, imm12),
            Inst {
                funct3: 0b101,
                rd,
                rs1,
                imm12,
                ..
            } => self.lhu(rd, rs1, imm12),
            _ => bail!("unknown Load {:?}", inst),
        }
    }

    fn store(&mut self, inst: Inst) -> Result<()> {
        match inst {
            Inst {
                funct3: 0b000,
                rs1,
                rs2,
                imm12,
                ..
            } => self.sb(rs1, rs2, imm12),
            Inst {
                funct3: 0b001,
                rs1,
                rs2,
                imm12,
                ..
            } => self.sh(rs1, rs2, imm12),
            Inst {
                funct3: 0b010,
                rs1,
                rs2,
                imm12,
                ..
            } => self.sw(rs1, rs2, imm12),
            _ => bail!("unknown Store {:?}", inst),
        }
    }

    fn andi(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, self.get_x(rs1) & ((imm12 & 0x0FFF) as u32));
        Ok(())
    }

    fn addi(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, ((self.get_x(rs1) as i32) + imm12 as i32) as u32);
        Ok(())
    }

    fn slli(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, self.get_x(rs1) << imm12);
        Ok(())
    }

    fn slti(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, ((self.get_x(rs1) as i32) < (imm12 as i32)) as u32);
        Ok(())
    }

    fn sltiu(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, (self.get_x(rs1) < ((imm12 & 0x0FFF) as u32)) as u32);
        Ok(())
    }

    fn xori(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, self.get_x(rs1) ^ ((imm12 & 0x0FFF) as u32));
        Ok(())
    }

    fn srli(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, self.get_x(rs1) >> imm12);
        Ok(())
    }

    fn srai(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, ((self.get_x(rs1) as i32) >> imm12) as u32);
        Ok(())
    }

    fn ori(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        self.set_x(rd, self.get_x(rs1) | ((imm12 & 0x0FFF) as u32));
        Ok(())
    }

    fn jal(&mut self, ir: Inst) -> Result<()> {
        let Inst { rd, imm32, .. } = ir;
        self.set_x(rd, self.pc.wrapping_add(4));
        self.pc = (self.pc as i32).wrapping_add(imm32) as u32;
        Ok(())
    }

    fn jalr(&mut self, ir: Inst) -> Result<()> {
        let Inst { rd, rs1, imm32, .. } = ir;
        let base_addr = self.get_x(rs1);
        self.set_x(rd, self.pc.wrapping_add(4));
        self.pc = (base_addr as i32).wrapping_add(imm32) as u32;
        Ok(())
    }

    fn beq(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        if left == right {
            self.pc = (self.pc as i32).wrapping_add(imm12 as i32) as u32;
        }
        Ok(())
    }

    fn bne(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        if left != right {
            self.pc = (self.pc as i32).wrapping_add(imm12 as i32) as u32;
        }
        Ok(())
    }

    fn blt(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        if left < right {
            self.pc = (self.pc as i32).wrapping_add(imm12 as i32) as u32;
        }
        Ok(())
    }

    fn bge(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        if left >= right {
            self.pc = (self.pc as i32).wrapping_add(imm12 as i32) as u32;
        }
        Ok(())
    }

    fn bltu(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        if left < right {
            self.pc = (self.pc as i32).wrapping_add(imm12 as i32) as u32;
        }
        Ok(())
    }

    fn bgeu(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        if left >= right {
            self.pc = (self.pc as i32).wrapping_add(imm12 as i32) as u32;
        }
        Ok(())
    }

    fn lb(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let base_addr = self.get_x(rs1);
        self.set_x(
            rd,
            self.bus
                .read8((base_addr as i32).wrapping_add(imm12 as i32) as u32) as i8
                as i32 as u32,
        );
        Ok(())
    }

    fn lh(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let base_addr = self.get_x(rs1);
        self.set_x(
            rd,
            self.bus
                .read16((base_addr as i32).wrapping_add(imm12 as i32) as u32) as i16
                as i32 as u32,
        );
        Ok(())
    }

    fn lw(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let base_addr = self.get_x(rs1);
        self.set_x(
            rd,
            self.bus
                .read32((base_addr as i32).wrapping_add(imm12 as i32) as u32),
        );
        Ok(())
    }

    fn lbu(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let base_addr = self.get_x(rs1);
        self.set_x(
            rd,
            self.bus
                .read8((base_addr as i32).wrapping_add(imm12 as i32) as u32) as u32,
        );
        Ok(())
    }

    fn lhu(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let base_addr = self.get_x(rs1);
        self.set_x(
            rd,
            self.bus
                .read16((base_addr as i32).wrapping_add(imm12 as i32) as u32) as u32,
        );
        Ok(())
    }

    fn sb(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let base_addr = self.get_x(rs1);
        self.bus.write8(
            (base_addr as i32).wrapping_add(imm12 as i32) as u32,
            self.get_x(rs2) as u8,
        );
        Ok(())
    }

    fn sh(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let base_addr = self.get_x(rs1);
        self.bus.write16(
            (base_addr as i32).wrapping_add(imm12 as i32) as u32,
            self.get_x(rs2) as u16,
        );
        Ok(())
    }

    fn sw(&mut self, rs1: usize, rs2: usize, imm12: i16) -> Result<()> {
        let base_addr = self.get_x(rs1);
        self.bus.write32(
            (base_addr as i32).wrapping_add(imm12 as i32) as u32,
            self.get_x(rs2),
        );
        Ok(())
    }

    fn misc_mem(&mut self, ir: Inst) -> Result<()> {
        match ir {
            Inst {
                funct3: 0b000,
                imm12,
                ..
            } => self.fence(imm12),
            Inst { funct3: 0b001, .. } => self.fencei(),
            _ => bail!("unknown MISC-MEM {:?}", ir),
        }
    }

    fn fence(&self, _: i16) -> Result<()> {
        todo!()
    }

    fn fencei(&self) -> Result<()> {
        // hartが一つなので何もしない
        Ok(())
    }

    fn system(&mut self, ir: Inst) -> Result<()> {
        match ir {
            Inst {
                funct3: 0b001,
                rd,
                rs1,
                imm12,
                ..
            } => self.csrrw(rd, rs1, imm12),
            Inst {
                funct3: 0b010,
                rd,
                rs1,
                imm12,
                ..
            } => self.csrrs(rd, rs1, imm12),
            Inst {
                funct3: 0b011,
                rd,
                rs1,
                imm12,
                ..
            } => self.csrrc(rd, rs1, imm12),
            Inst {
                funct3: 0b101,
                rd,
                rs1,
                imm12,
                ..
            } => self.csrrwi(rd, rs1, imm12),
            Inst {
                funct3: 0b110,
                rd,
                rs1,
                imm12,
                ..
            } => self.csrrsi(rd, rs1, imm12),
            Inst {
                funct3: 0b111,
                rd,
                rs1,
                imm12,
                ..
            } => self.csrrci(rd, rs1, imm12),
            _ => bail!("unknown SYSTEM {:?}", ir),
        }
    }

    fn csrrw(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let csr_val = self.get_csr(imm12 as u16);
        let src_val = self.get_x(rs1);
        self.set_x(rd, csr_val);
        self.set_csr(imm12 as u16, src_val);
        Ok(())
    }

    fn csrrs(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let csr_val = self.get_csr(imm12 as u16);
        let src_val = self.get_x(rs1);
        self.set_x(rd, csr_val);
        self.set_csr(imm12 as u16, src_val | csr_val);
        Ok(())
    }

    fn csrrc(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let csr_val = self.get_csr(imm12 as u16);
        let src_val = self.get_x(rs1);
        self.set_x(rd, csr_val);
        self.set_csr(imm12 as u16, src_val & !csr_val);
        Ok(())
    }

    fn csrrwi(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let csr_val = self.get_csr(imm12 as u16);
        let src_val = rs1 as u32;
        self.set_x(rd, csr_val);
        self.set_csr(imm12 as u16, src_val);
        Ok(())
    }

    fn csrrsi(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let csr_val = self.get_csr(imm12 as u16);
        let src_val = rs1 as u32;
        self.set_x(rd, csr_val);
        self.set_csr(imm12 as u16, src_val | csr_val);
        Ok(())
    }

    fn csrrci(&mut self, rd: usize, rs1: usize, imm12: i16) -> Result<()> {
        let csr_val = self.get_csr(imm12 as u16);
        let src_val = rs1 as u32;
        self.set_x(rd, csr_val);
        self.set_csr(imm12 as u16, src_val & !csr_val);
        Ok(())
    }

    fn op(&mut self, ir: Inst) -> Result<()> {
        match ir {
            Inst {
                funct3: 0b000,
                funct7: 0b0000000,
                rd,
                rs1,
                rs2,
                ..
            } => self.add(rd, rs1, rs2),
            Inst {
                funct3: 0b000,
                funct7: 0b0000001,
                rd,
                rs1,
                rs2,
                ..
            } => self.mul(rd, rs1, rs2),
            Inst {
                funct3: 0b000,
                funct7: 0b0100000,
                rd,
                rs1,
                rs2,
                ..
            } => self.sub(rd, rs1, rs2),
            Inst {
                funct3: 0b001,
                funct7: 0b0000000,
                rd,
                rs1,
                rs2,
                ..
            } => self.sll(rd, rs1, rs2),
            Inst {
                funct3: 0b001,
                funct7: 0b0000001,
                rd,
                rs1,
                rs2,
                ..
            } => self.mulh(rd, rs1, rs2),
            Inst {
                funct3: 0b010,
                funct7: 0b0000000,
                rd,
                rs1,
                rs2,
                ..
            } => self.slt(rd, rs1, rs2),
            Inst {
                funct3: 0b010,
                funct7: 0b0000001,
                rd,
                rs1,
                rs2,
                ..
            } => self.mulhsu(rd, rs1, rs2),
            Inst {
                funct3: 0b011,
                funct7: 0b0000000,
                rd,
                rs1,
                rs2,
                ..
            } => self.sltu(rd, rs1, rs2),
            Inst {
                funct3: 0b011,
                funct7: 0b0000001,
                rd,
                rs1,
                rs2,
                ..
            } => self.mulhu(rd, rs1, rs2),
            Inst {
                funct3: 0b100,
                funct7: 0b0000000,
                rd,
                rs1,
                rs2,
                ..
            } => self.xor(rd, rs1, rs2),
            Inst {
                funct3: 0b100,
                funct7: 0b0000001,
                rd,
                rs1,
                rs2,
                ..
            } => self.div(rd, rs1, rs2),
            Inst {
                funct3: 0b101,
                funct7: 0b0000000,
                rd,
                rs1,
                rs2,
                ..
            } => self.srl(rd, rs1, rs2),
            Inst {
                funct3: 0b101,
                funct7: 0b0000001,
                rd,
                rs1,
                rs2,
                ..
            } => self.divu(rd, rs1, rs2),
            Inst {
                funct3: 0b101,
                funct7: 0b0100000,
                rd,
                rs1,
                rs2,
                ..
            } => self.sra(rd, rs1, rs2),
            Inst {
                funct3: 0b110,
                funct7: 0b0000000,
                rd,
                rs1,
                rs2,
                ..
            } => self.or(rd, rs1, rs2),
            Inst {
                funct3: 0b110,
                funct7: 0b0000001,
                rd,
                rs1,
                rs2,
                ..
            } => self.rem(rd, rs1, rs2),
            Inst {
                funct3: 0b111,
                funct7: 0b0000000,
                rd,
                rs1,
                rs2,
                ..
            } => self.and(rd, rs1, rs2),
            Inst {
                funct3: 0b111,
                funct7: 0b0000001,
                rd,
                rs1,
                rs2,
                ..
            } => self.remu(rd, rs1, rs2),
            _ => bail!("unknown OP {:?}", ir),
        }
    }

    fn lui(&mut self, ir: Inst) -> Result<()> {
        let Inst { rd, imm32, .. } = ir;
        self.set_x(rd, imm32 as u32);
        Ok(())
    }

    fn auipc(&mut self, ir: Inst) -> Result<()> {
        let Inst { rd, imm32, .. } = ir;
        self.set_x(rd, (self.pc as i32).wrapping_add(imm32) as u32);
        Ok(())
    }

    fn add(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        self.set_x(rd, left.wrapping_add(right) as u32);
        Ok(())
    }

    fn sub(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        self.set_x(rd, left.wrapping_sub(right) as u32);
        Ok(())
    }

    fn sll(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        self.set_x(rd, (left << right) as u32);
        Ok(())
    }

    fn slt(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        self.set_x(rd, (left < right) as u32);
        Ok(())
    }

    fn sltu(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        self.set_x(rd, (left < right) as u32);
        Ok(())
    }

    fn xor(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        self.set_x(rd, left ^ right);
        Ok(())
    }

    fn srl(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        self.set_x(rd, (left >> right) as u32);
        Ok(())
    }

    fn sra(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        self.set_x(rd, (left >> right) as u32);
        Ok(())
    }

    fn or(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        self.set_x(rd, left | right);
        Ok(())
    }

    fn and(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        self.set_x(rd, left & right);
        Ok(())
    }

    fn mul(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        self.set_x(rd, left.wrapping_mul(right) as u32);
        Ok(())
    }

    fn mulh(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i64;
        let right = self.get_x(rs2) as i64;
        self.set_x(rd, (left.wrapping_mul(right) >> 32) as u32);
        Ok(())
    }

    fn mulhsu(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i64;
        let right = self.get_x(rs2) as u64 as i64;
        self.set_x(rd, (left.wrapping_mul(right) >> 32) as u32);
        Ok(())
    }

    fn mulhu(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as u64;
        let right = self.get_x(rs2) as u64;
        self.set_x(rd, (left.wrapping_mul(right) >> 32) as u32);
        Ok(())
    }

    fn div(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        self.set_x(rd, left.wrapping_div(right) as u32);
        Ok(())
    }

    fn divu(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        self.set_x(rd, left.wrapping_div(right));
        Ok(())
    }

    fn rem(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1) as i32;
        let right = self.get_x(rs2) as i32;
        self.set_x(rd, left.wrapping_rem(right) as u32);
        Ok(())
    }

    fn remu(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<()> {
        let left = self.get_x(rs1);
        let right = self.get_x(rs2);
        self.set_x(rd, left.wrapping_rem(right));
        Ok(())
    }

    fn amo(&mut self, ir: Inst) -> Result<()> {
        // NOTE: AMO系はaq/rlを無視する
        match ir {
            Inst {
                funct5: 0b00010,
                rd,
                rs1,
                rs2,
                ..
            } => self.lrw(rd, rs1, rs2),
            Inst {
                funct5: 0b00011,
                rd,
                rs1,
                rs2,
                ..
            } => self.scw(rd, rs1, rs2),
            Inst {
                funct5: 0b00001,
                rd,
                rs1,
                rs2,
                ..
            } => self.amoswapw(rd, rs1, rs2),
            Inst {
                funct5: 0b00000,
                rd,
                rs1,
                rs2,
                ..
            } => self.amoaddw(rd, rs1, rs2),
            Inst {
                funct5: 0b00100,
                rd,
                rs1,
                rs2,
                ..
            } => self.amoxorw(rd, rs1, rs2),
            Inst {
                funct5: 0b01100,
                rd,
                rs1,
                rs2,
                ..
            } => self.amoandw(rd, rs1, rs2),
            Inst {
                funct5: 0b01000,
                rd,
                rs1,
                rs2,
                ..
            } => self.amoorw(rd, rs1, rs2),
            Inst {
                funct5: 0b10000,
                rd,
                rs1,
                rs2,
                ..
            } => self.amominw(rd, rs1, rs2),
            Inst {
                funct5: 0b10100,
                rd,
                rs1,
                rs2,
                ..
            } => self.amomaxw(rd, rs1, rs2),
            Inst {
                funct5: 0b11000,
                rd,
                rs1,
                rs2,
                ..
            } => self.amominuw(rd, rs1, rs2),
            Inst {
                funct5: 0b11100,
                rd,
                rs1,
                rs2,
                ..
            } => self.amomaxuw(rd, rs1, rs2),
            _ => bail!("unknown AMO {:?}", ir),
        }
    }

    fn lrw(&mut self, rd: usize, rs1: usize, _: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let val = self.bus.read32(addr);
        self.set_x(rd, val);
        Ok(())
    }

    fn scw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let val = self.get_x(rs2);
        self.bus.write32(addr, val);
        self.set_x(rd, 0);
        Ok(())
    }

    fn amoswapw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus.write32(addr, right);
        Ok(())
    }

    fn amoaddw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus
            .write32(addr, (left as i32).wrapping_add(right as i32) as u32);
        Ok(())
    }

    fn amoxorw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus.write32(addr, left ^ right);
        Ok(())
    }

    fn amoandw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus.write32(addr, left & right);
        Ok(())
    }

    fn amoorw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus.write32(addr, left | right);
        Ok(())
    }

    fn amominw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus
            .write32(addr, std::cmp::min(left as i32, right as i32) as u32);
        Ok(())
    }

    fn amomaxw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus
            .write32(addr, std::cmp::max(left as i32, right as i32) as u32);
        Ok(())
    }

    fn amominuw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus.write32(addr, std::cmp::min(left, right));
        Ok(())
    }

    fn amomaxuw(&mut self, rd: usize, rs1: usize, rs2: usize) -> Result<(), anyhow::Error> {
        let addr = self.get_x(rs1);
        let left = self.bus.read32(addr);
        let right = self.get_x(rs2);
        self.set_x(rd, left);
        self.bus.write32(addr, std::cmp::max(left, right));
        Ok(())
    }
}

#[derive(Debug)]
struct Inst {
    funct7: u8,
    funct5: u8,
    rs2: usize,
    rs1: usize,
    funct3: u8,
    rd: usize,
    imm12: i16,
    imm32: i32,
}

pub trait IntoI12 {
    fn into_i12(&self) -> i16;
}

impl IntoI12 for u16 {
    fn into_i12(&self) -> i16 {
        let mut result = self & 0x0FFF;

        if self & 0x0800 > 0 {
            result |= 0xF000;
        }

        result as i16
    }
}

impl Inst {
    fn from_r(ir: u32) -> Self {
        Self {
            funct7: ((ir >> 25) & 0b1111111) as u8,
            funct5: ((ir >> 27) & 0b11111) as u8,
            rs2: ((ir >> 20) & 0b11111) as usize,
            rs1: ((ir >> 15) & 0b11111) as usize,
            funct3: ((ir >> 12) & 0b111) as u8,
            rd: ((ir >> 7) & 0b11111) as usize,
            imm12: 0,
            imm32: 0,
        }
    }

    fn from_i(ir: u32) -> Self {
        Self {
            imm12: (((ir >> 20) & 0b111111111111) as u16).into_i12(),
            rs1: ((ir >> 15) & 0b11111) as usize,
            funct3: ((ir >> 12) & 0b111) as u8,
            rd: ((ir >> 7) & 0b11111) as usize,
            funct7: ((ir >> 25) & 0b1111111) as u8,
            funct5: ((ir >> 27) & 0b11111) as u8,
            rs2: 0,
            imm32: 0,
        }
    }

    fn from_s(ir: u32) -> Self {
        Self {
            rs2: ((ir >> 20) & 0b11111) as usize,
            rs1: ((ir >> 15) & 0b11111) as usize,
            funct3: ((ir >> 12) & 0b111) as u8,
            imm12: ((((ir >> 7) & 0b11111) | ((ir >> 25) & 0b1111111)) as u16).into_i12(),
            funct7: 0,
            funct5: 0,
            rd: 0,
            imm32: 0,
        }
    }

    fn from_u(ir: u32) -> Self {
        Self {
            rd: ((ir >> 7) & 0b11111) as usize,
            funct7: 0,
            funct5: 0,
            rs2: 0,
            rs1: 0,
            funct3: 0,
            imm12: 0,
            imm32: ((ir >> 12) << 12) as i32,
        }
    }

    fn from_b(ir: u32) -> Self {
        let mut imm12: u16 = 0;

        imm12 |= (((ir >> 8) & 0b1111) << 1) as u16; // imm[4:1]
        imm12 |= (((ir >> 25) & 0b111111) << 5) as u16; // imm[10:5]
        imm12 |= (((ir >> 7) & 0b1) << 11) as u16; // imm[11]
        imm12 |= (((ir >> 31) & 0b1) << 31) as u16; // imm[12]

        Self {
            rs2: ((ir >> 20) & 0b11111) as usize,
            rs1: ((ir >> 15) & 0b11111) as usize,
            funct3: ((ir >> 12) & 0b111) as u8,
            imm12: imm12.into_i12(),
            funct7: 0,
            funct5: 0,
            rd: 0,
            imm32: 0,
        }
    }

    fn from_j(ir: u32) -> Self {
        let mut imm32: u32 = 0;

        imm32 |= (((ir >> 21) & 0b1111111111) << 1) as u32; // imm[10:1]
        imm32 |= (((ir >> 20) & 0b1) << 11) as u32; // imm[11]
        imm32 |= (((ir >> 12) & 0b11111111) << 12) as u32; // imm[19:12]
        imm32 |= (((ir >> 31) & 0b1) << 20) as u32; // imm[20]

        Self {
            rd: ((ir >> 7) & 0b11111) as usize,
            funct7: 0,
            funct5: 0,
            rs2: 0,
            rs1: 0,
            funct3: 0,
            imm12: 0,
            imm32: imm32 as i32,
        }
    }
}
