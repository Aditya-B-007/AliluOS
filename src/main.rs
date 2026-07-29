//! # AliluOS Root Kernel Entry Point (`main.rs`)
//!
//! - **WHAT**: This file serves as the top-level binary entry point for the AliluOS kernel.
//!   It disables standard library linking (`#![no_std]`), removes standard main entry points (`#![no_main]`),
//!   declares internal kernel modules, defines the panic handler, and transfers CPU control to the `Kernel` struct.
//! - **WHY**: Bare-metal operating systems execute directly on hardware without an underlying OS or runtime C library.
//!   We must handle panic landing pads and symbol linkage (`_start`) manually.
//! - **WHEN**: Executed immediately after the BIOS/bootloader sets up 64-bit long mode and jumps to the kernel ELF symbol `_start`.
//! - **HOW**: Links `alloc` for dynamic heap memory, configures module declarations, initializes the kernel object,
//!   enables x86 interrupts via the `sti` instruction, and enters the main execution event loop.

#![no_std]
#![no_main]

extern crate alloc;

// --- Subsystem Module Declarations ---
mod kernel;     // Core kernel lifecycle and event dispatcher
mod vga;        // VGA text mode display hardware driver & global synchronized WRITER
mod keyboard;   // PS/2 keyboard scan code translator & event queue consumer
mod interrupts; // GDT, IDT, PIC, PIT timer, and hardware IRQ interrupt handlers
mod allocator;  // Heap memory allocator and memory map manager
mod fs;         // B-Tree indexed hierarchical filesystem
mod shell;      // Interactive Command Line Interface and Help Viewer
mod game;       // Built-in text games and drawing canvas application
mod network;    // Integrated Networking, smoltcp TCP/IP, HTTP service, Git client & Text Browser


use core::panic::PanicInfo;
use kernel::Kernel;

/// Kernel Panic Handler Function.
///
/// - **WHAT**: Catches unrecoverable runtime errors and fatal assertions across the kernel.
/// - **WHY**: In a `#![no_std]` environment, there is no operating system or stack unwind runtime to catch panics.
///   Rust requires a custom panic handler symbol to define how the CPU responds when a panic occurs.
/// - **WHEN**: Invoked automatically by the Rust runtime whenever an `assert!`, `unwrap()`, or `panic!` macro fails.
/// - **HOW**: Accepts `PanicInfo` containing file/line metadata. Halts execution safely by entering an infinite loop.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut vga = crate::vga::WRITER.lock();
    vga.set_color(crate::vga::Color::LightRed, crate::vga::Color::Black);
    vga.println("\n--- KERNEL PANIC ---");
    if let Some(location) = info.location() {
        vga.write("Location: ");
        vga.write(location.file());
        vga.println("");
    }
    vga.set_color(crate::vga::Color::White, crate::vga::Color::Black);
    loop {}
}

/// Low-level Kernel Entry Symbol (`_start`).
///
/// - **WHAT**: The absolute first function executed when the kernel receives control from the bootloader.
/// - **WHY**: Standard C/Rust programs use `main()`, but bare-metal kernels require `_start` as the default ELF symbol.
///   `#[no_mangle]` preserves the exact name `_start` in the compiled binary symbol table so the bootloader can find it.
/// - **WHEN**: Triggered by the bootloader after kernel loading, stack initialization, and page table setup.
/// - **HOW**:
///   1. Instantiates the main `Kernel` state object.
///   2. Calls `kernel.initialize()` to setup VGA, keyboard, GDT, IDT, PIC, PIT, and Heap memory.
///   3. Executes inline assembly `sti` (Set Interrupt Flag) to enable hardware interrupts.
///   4. Calls `kernel.run()`, which never returns (`-> !`).
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 1. Construct the core Kernel state instance
    let mut kernel = Kernel::new();

    // 2. Initialize low-level drivers, memory allocators, and interrupt descriptor tables
    kernel.initialize();

    // 3. Enable x86 CPU hardware interrupts using assembly `sti` instruction.
    //    WHY: Allows PIT timer ticks (IRQ0) and PS/2 keyboard inputs (IRQ1) to interrupt the CPU.
    //    HOW: `sti` sets the IF (Interrupt Flag) in the CPU's EFLAGS register.
    unsafe {
        core::arch::asm!("sti", options(nomem, nostack));
    }

    // 4. Enter the infinite kernel event polling loop
    kernel.run();
}
