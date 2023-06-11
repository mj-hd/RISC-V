use std::{thread, time::Duration};

use bus::Bus;
use cpu::Cpu;

pub mod bus;
pub mod cpu;
pub mod timer;
pub mod utils;

fn main() {
    let bus = Bus::new();
    let mut cpu = Cpu::new(bus);

    loop {
        cpu.tick().unwrap();

        thread::sleep(Duration::from_millis(16));
    }
}
