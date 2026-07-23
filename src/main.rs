#![no_std]
#![no_main]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
mod kernel;
mod vga;
mod keyboard;
mod interrupts;
mod allocator;
mod fs;
mod shell;
mod game;

use core::panic::PanicInfo;
use kernel::Kernel;

entry_point!(kernel_main);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // TODO:
    // Display panic information using VGA once the console
    // is fully initialized.
    let _ = info;

    loop {}
}

/// Kernel entry point.
/// Execution begins here after the bootloader transfers control.
fn kernel_main(_boot_info: &'static BootInfo) -> ! {
    let mut kernel = Kernel::new();
    kernel.initialize();
    unsafe {
        core::arch::asm!("sti", options(nomem, nostack));
    }
    kernel.run();
}
