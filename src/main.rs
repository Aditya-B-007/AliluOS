//! # AliluOS Root Kernel Entry Point (`main.rs`)
//!
//! - **WHAT**: Top-level binary entry point for the AliluOS bare-metal kernel.
//! - **WHY**: Declares module paths across Privilege Ring subdirectories (`ring0/`, `ring1/`, `ring2/`).

#![no_std]
#![no_main]

extern crate alloc;

// ========================================================================
// PRIVILEGE RING MODULE DECLARATIONS (ring0, ring1, ring2)
// ========================================================================

// --- RING 0: Kernel Core, Low-Level Hardware Drivers, & Scheduler ---
#[path = "ring0/config.rs"]     mod config;     // Centralized Architecture & Privilege Ring Parameters
#[path = "ring0/kernel.rs"]     mod kernel;     // Core kernel lifecycle and event dispatcher
#[path = "ring0/vga.rs"]        mod vga;        // VGA text mode display hardware driver (0xB8000)
#[path = "ring0/keyboard.rs"]   mod keyboard;   // PS/2 keyboard hardware driver & scancode queue
#[path = "ring0/interrupts.rs"] mod interrupts; // GDT, IDT, TSS, PIC/PIT, IRQs, and Syscall Gate
#[path = "ring0/allocator.rs"]  mod allocator;  // Heap memory allocator (5 MiB memory pool)
#[path = "ring0/scheduler.rs"]  mod scheduler;  // Preemptive multi-threaded round-robin scheduler

// --- RING 1: Single-Process Resource Manager, Storage, & Networking ---
#[path = "ring1/process.rs"]    mod process;    // Single-Process Resource Manager (RAM, Disk, Net quotas)
#[path = "ring1/disk.rs"]       mod disk;       // Secondary ATA Disk Hardware Driver & persistent storage
#[path = "ring1/btree.rs"]      mod btree;      // On-Disk B+ Tree Indexing Engine (1024-byte blocks)
#[path = "ring1/vat.rs"]        mod vat;        // In-Memory Virtual Address Table & demand paging page cache
#[path = "ring1/fs.rs"]         mod fs;         // B+ Tree persistent filesystem API
#[path = "ring1/network.rs"]    mod network;    // Integrated Networking, smoltcp TCP/IP stack, RTL8139 NIC

// --- RING 2: Execution Threads, System Calls, Shell, & Applications ---
#[path = "ring2/thread.rs"]     mod thread;     // Thread Control Block (TCB), CpuContext, & single handler method
#[path = "ring2/syscall.rs"]    mod syscall;    // 15-function System Call Dispatcher Gate (0x01..0x0F)
#[path = "ring2/shell.rs"]      mod shell;      // Interactive CLI shell, text editor, and tasks monitor
#[path = "ring2/game.rs"]       mod game;       // Built-in text games and drawing canvas application

use core::panic::PanicInfo;
use kernel::Kernel;

/// Kernel Panic Handler Function.
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
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut kernel = Kernel::new();
    kernel.initialize();

    unsafe {
        core::arch::asm!("sti", options(nomem, nostack));
    }

    kernel.run();
}
