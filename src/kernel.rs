//! # AliluOS Main Kernel Module (`kernel.rs`)
//!
//! - **WHAT**: Encapsulates overall kernel initialization, main execution loop, keyboard polling, and event dispatching to the shell.
//! - **WHY**: Centralizes system startup sequence and coordinates sub-components (keyboard driver, interrupt controller, heap allocator, interactive shell).
//! - **WHEN**: Instantiated and executed during system boot by `_start()` in `main.rs`.
//! - **HOW**: Initializes hardware subsystems sequentially, renders the boot banner on the global VGA console, and loops infinitely polling for keyboard events.

use crate::vga::{Color, WRITER};
use crate::keyboard::{Key, KeyEvent, Keyboard};

/// Core Kernel Structure.
///
/// - **WHAT**: Holds state for system hardware drivers (Keyboard) and high-level subsystems (Shell).
/// - **WHY**: Provides a single owner and namespace for managing operating system execution lifecycle.
/// - **WHEN**: Lives for the entire duration of the OS lifetime (never dropped).
/// - **HOW**: Instantiated via `Kernel::new()`, initialized via `initialize()`, and driven via `run()`.
pub struct Kernel {
    keyboard: Keyboard,
    shell: crate::shell::Shell,
}

impl Kernel {
    /// Constructs a new `Kernel` instance with default driver and shell states.
    ///
    /// - **WHAT**: Initializes memory structures for the `Keyboard` driver and `Shell` engine.
    /// - **WHY**: Separates allocation/construction from hardware initialization.
    /// - **WHEN**: Called at the beginning of `_start()` in `main.rs`.
    /// - **HOW**: Returns a `Kernel` struct populated with initialized sub-structs.
    pub fn new() -> Self {
        Self {
            keyboard: Keyboard::new(),
            shell: crate::shell::Shell::new(),
        }
    }

    /// Orchestrates hardware driver and memory allocator initialization sequence.
    ///
    /// - **WHAT**: Sequentially initializes VGA display, Keyboard driver, GDT/IDT/PIC/PIT interrupt controller, Heap Allocator, and boot banner.
    /// - **WHY**: Subsystems have strict initialization dependencies (e.g. heap allocator depends on GDT/IDT, shell depends on heap and VGA).
    /// - **WHEN**: Invoked once during system boot in `_start()`.
    /// - **HOW**: Calls `WRITER.lock().init()`, `keyboard.init()`, `interrupts::init()`, `allocator::init_heap()`, and `boot_banner()`.
    pub fn initialize(&mut self) {
        // 1. Initialize VGA text mode screen and clear buffer
        WRITER.lock().init();

        // 2. Initialize PS/2 Keyboard modifier state
        self.keyboard.init();

        // 3. Configure GDT, IDT, dual 8259 PICs, and PIT timer (100 Hz)
        crate::interrupts::init();

        // 4. Initialize global heap memory allocator (100 KiB heap)
        crate::allocator::init_heap();

        // 5. Initialize PCI Network Interface Card driver
        crate::network::NIC_DRIVER.lock().init();

        // 6. Initialize Secondary Storage Disk & B+ Tree Filesystem
        crate::fs::FS.lock().init();

        // 7. Render AliluOS welcome header and command prompt
        self.boot_banner();

    }

    /// Main Kernel Execution Loop.
    ///
    /// - **WHAT**: The infinite CPU loop that drives the operating system after boot initialization completes.
    /// - **WHY**: Bare-metal operating systems must never return from `_start()`; they run continuously processing interrupts and user input.
    /// - **WHEN**: Called from `_start()` after hardware interrupts are enabled (`sti`).
    /// - **HOW**: Continuously polls `self.keyboard.read_key()` for hardware keystrokes and routes them to `handle_key_event()`.
    pub fn run(&mut self) -> ! {
        loop {
            // Poll for pending keyboard input events from IRQ1 ring-buffer or PS/2 port
            if let Some(event) = self.keyboard.read_key() {
                self.handle_key_event(event);
            }
        }
    }

    // ----------------------------------------------------
    // Private Helper Methods
    // ----------------------------------------------------

    /// Renders the AliluOS Startup Boot Banner on VGA Display.
    ///
    /// - **WHAT**: Displays kernel welcome header, branding, initialization status, and initial `> ` shell prompt.
    /// - **WHY**: Provides clear visual feedback to the user that the kernel booted cleanly and is ready for commands.
    /// - **WHEN**: Called at the end of `initialize()`.
    /// - **HOW**: Uses the synchronized global `WRITER` instance to write formatted text to VGA memory at 0xB8000.
    fn boot_banner(&mut self) {
        let mut vga = WRITER.lock();
        vga.set_color(Color::LightGreen, Color::Black);

        vga.println("========================================");
        vga.println("          Welcome to AliluOS");
        vga.println("========================================");

        vga.set_color(Color::White, Color::Black);

        vga.println("");
        vga.println("Kernel Initialized Successfully.");
        vga.println("");
        vga.write("> ");
    }

    /// Filters key event press/release actions.
    ///
    /// - **WHAT**: Receives `KeyEvent` (Press or Release) and filters for `Press` events.
    /// - **WHY**: Shell character entry and command triggers occur on key depression (Press), avoiding duplicate input on release.
    /// - **WHEN**: Triggered by `run()` whenever a `KeyEvent` is returned by `keyboard.read_key()`.
    /// - **HOW**: Pattern matches `KeyEvent::Press(key)` and forwards the key enum to `handle_key_press()`.
    fn handle_key_event(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Press(key) => {
                self.handle_key_press(key);
            }
            KeyEvent::Release(_) => {
                // Key release events are ignored for shell typing, but tracked internally by Keyboard for Shift/Ctrl flags
            }
        }
    }

    /// Routes pressed keys to appropriate Shell event handlers.
    ///
    /// - **WHAT**: Translates key types (`Character`, `Space`, `Enter`, `Backspace`, `Escape`) into Shell actions.
    /// - **WHY**: Decouples key enumeration values from shell mode logic and input buffer manipulation.
    /// - **WHEN**: Invoked by `handle_key_event()` for every key press.
    /// - **HOW**: Calls matching shell methods (`handle_char`, `handle_space`, `handle_enter`, `handle_backspace`, `handle_escape`).
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
            Key::Space => {
                self.shell.handle_space();
            }
            Key::Enter => {
                self.shell.handle_enter();
            }
            Key::Backspace => {
                self.shell.handle_backspace();
            }
            Key::Escape => {
                self.shell.handle_escape();
            }
            _ => {
                // Unhandled function keys or modifier standalone presses
            }
        }
    }
}
