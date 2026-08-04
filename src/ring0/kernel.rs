//! # AliluOS Main Kernel Module (`ring0/kernel.rs`)

#![allow(dead_code)]

use crate::vga::{Color, WRITER};
use crate::keyboard::{Key, KeyEvent, Keyboard};
use crate::thread::PrivilegeLevel;

pub struct Kernel {
    keyboard: Keyboard,
    shell: crate::shell::Shell,
}

impl Kernel {
    pub fn new() -> Self {
        Self {
            keyboard: Keyboard::new(),
            shell: crate::shell::Shell::new(),
        }
    }

    pub fn initialize(&mut self) {
        // 1. Initialize VGA display
        WRITER.lock().init();

        // 2. Initialize PS/2 Keyboard modifier state
        self.keyboard.init();

        // 3. Configure GDT, IDT, dual 8259 PICs, and PIT timer (100 Hz)
        crate::interrupts::init();

        // 4. Initialize global heap memory allocator (5 MiB heap)
        crate::allocator::init_heap();

        // 5. Initialize PCI Network Interface Card driver
        crate::network::NIC_DRIVER.lock().init();

        // 6. Initialize Secondary Storage Disk & B+ Tree Filesystem
        crate::fs::FS.lock().init();

        // 7. Register Threads TID 0..10 across Privilege Rings (Ring 0, Ring 1, Ring 2)
        {
            let mut pm = crate::process::PROCESS_MANAGER.lock();
            let dummy_stack_top = 0x_4444_5000_0000u64;

            // Ring 0 Kernel Core Threads
            let _ = pm.register_thread_with_ring("idle_thread", PrivilegeLevel::Ring0, idle_thread_fn, dummy_stack_top);
            let _ = pm.register_thread_with_ring("watchdog_thread", PrivilegeLevel::Ring0, watchdog_thread_fn, dummy_stack_top + 0x50000);
            let _ = pm.register_thread_with_ring("deadlock_monitor_thread", PrivilegeLevel::Ring0, deadlock_monitor_thread_fn, dummy_stack_top + 0x90000);
            let _ = pm.register_thread_with_ring("mutex_manager_thread", PrivilegeLevel::Ring0, mutex_manager_thread_fn, dummy_stack_top + 0xA0000);

            // Ring 1 Driver & Resource Service Threads
            let _ = pm.register_thread_with_ring("vat_syncer_thread", PrivilegeLevel::Ring1, vat_syncer_thread_fn, dummy_stack_top + 0x20000);
            let _ = pm.register_thread_with_ring("net_worker_thread", PrivilegeLevel::Ring1, net_worker_thread_fn, dummy_stack_top + 0x30000);
            let _ = pm.register_thread_with_ring("log_flush_thread", PrivilegeLevel::Ring1, log_flush_thread_fn, dummy_stack_top + 0x40000);
            let _ = pm.register_thread_with_ring("ipc_router_thread", PrivilegeLevel::Ring1, ipc_router_thread_fn, dummy_stack_top + 0x70000);

            // Ring 2 User & App Execution Threads
            let _ = pm.register_thread_with_ring("shell_cli_thread", PrivilegeLevel::Ring2, shell_cli_thread_fn, dummy_stack_top + 0x10000);
            let _ = pm.register_thread_with_ring("crypto_worker_thread", PrivilegeLevel::Ring2, crypto_worker_thread_fn, dummy_stack_top + 0x60000);
            let _ = pm.register_thread_with_ring("sensor_poll_thread", PrivilegeLevel::Ring2, sensor_poll_thread_fn, dummy_stack_top + 0x80000);
        }

        // 8. Initialize Preemptive Multi-Threaded Scheduler
        crate::scheduler::SCHEDULER.lock().init();

        // 9. Render AliluOS welcome header and command prompt
        self.boot_banner();
    }

    pub fn run(&mut self) -> ! {
        loop {
            if let Some(event) = self.keyboard.read_key() {
                self.handle_key_event(event);
            }
        }
    }

    fn boot_banner(&mut self) {
        let mut vga = WRITER.lock();
        vga.set_color(Color::LightGreen, Color::Black);

        vga.println("========================================");
        vga.println("          Welcome to AliluOS");
        vga.println("========================================");

        vga.set_color(Color::White, Color::Black);

        vga.println("");
        vga.println("Kernel Initialized Successfully (Ring 0, Ring 1, Ring 2 Segregation).");
        vga.println("");
        vga.write("> ");
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Press(key) => {
                self.handle_key_press(key);
            }
            KeyEvent::Release(_) => {}
        }
    }

    fn handle_key_press(&mut self, key: Key) {
        let is_ctrl = self.keyboard.is_ctrl_pressed();
        match key {
            Key::Character(c) => {
                if is_ctrl {
                    self.shell.handle_ctrl_char(c);
                } else {
                    self.shell.handle_char(c);
                }
            }
            Key::Space => self.shell.handle_space(),
            Key::Enter => self.shell.handle_enter(),
            Key::Backspace => self.shell.handle_backspace(),
            Key::Escape => self.shell.handle_escape(),
            _ => {}
        }
    }
}

// --- Multi-Threaded Kernel Execution Payload Functions ---
fn idle_thread_fn() { loop { core::hint::spin_loop(); } }
fn shell_cli_thread_fn() { loop { core::hint::spin_loop(); } }
fn vat_syncer_thread_fn() { loop { core::hint::spin_loop(); } }
fn net_worker_thread_fn() { loop { core::hint::spin_loop(); } }
fn log_flush_thread_fn() { loop { core::hint::spin_loop(); } }
fn watchdog_thread_fn() { loop { core::hint::spin_loop(); } }
fn crypto_worker_thread_fn() { loop { core::hint::spin_loop(); } }
fn ipc_router_thread_fn() { loop { core::hint::spin_loop(); } }
fn sensor_poll_thread_fn() { loop { core::hint::spin_loop(); } }
fn deadlock_monitor_thread_fn() { loop { core::hint::spin_loop(); } }
fn mutex_manager_thread_fn() { loop { core::hint::spin_loop(); } }
