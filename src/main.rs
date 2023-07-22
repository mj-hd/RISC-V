use std::{fs::File, thread, time::Duration};

use bus::Bus;
use cpu::Cpu;

pub mod bus;
pub mod cpu;
pub mod timer;
pub mod utils;

fn main() {
    let mut bus = Bus::new();
    let mut kernel = File::open("linux.bin").unwrap();
    let mut dtb_f = File::open("dtb").unwrap();
    bus.load_kernel(&mut kernel);
    let dtb_addr = bus.load_dtb(&mut dtb_f);
    let mut cpu = Cpu::new(bus);

    cpu.set_a0(0x00);
    cpu.set_a1(dtb_addr);

    loop {
        cpu.tick().unwrap();

        thread::sleep(Duration::from_millis(16));
    }
}
