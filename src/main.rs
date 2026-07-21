#![no_std]
#![no_main]

mod kernel;
mod vga;
mod keyboard;
mod interrupts;
mod allocator;
mod fs;
mod game;

use core::panic::PanicInfo;
use kernel::Kernel;

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
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut kernel = Kernel::new();
    kernel.initialize();
    kernel.run();
}
