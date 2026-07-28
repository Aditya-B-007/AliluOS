//! # AliluOS VGA Text Mode Display Driver (`vga.rs`)
//!
//! - **WHAT**: Hardware driver for x86 80x25 VGA text mode display and cursor hardware ports.
//! - **WHY**: Provides text output capability (printing strings, character backspacing, scrolling, line clearing, colors)
//!   before complex graphics drivers are initialized.
//! - **WHEN**: Called whenever the kernel, shell, panic handler, exception handler, or games render text to the screen.
//! - **HOW**: Directly reads and writes volatile memory at physical VGA memory address `0xB8000` and configures CRT controller I/O ports (`0x3D4` / `0x3D5`).

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};

/// Base physical memory address of the x86 VGA text buffer (MMIO).
const BUFFER_ADDRESS: usize = 0xB8000;
/// Width of standard VGA text mode grid in character columns.
const BUFFER_WIDTH: usize = 80;
/// Height of standard VGA text mode grid in character rows.
const BUFFER_HEIGHT: usize = 25;

/// Global synchronized VGA driver instance.
///
/// - **WHAT**: Thread-safe spinlock wrapper guarding the single `VGA` hardware writer instance.
/// - **WHY**: Prevents concurrent print corruption and guarantees that cursor `(row, column)` positioning persists continuously across all kernel functions.
/// - **WHEN**: Locked by any kernel thread (`WRITER.lock()`) whenever text output is written.
/// - **HOW**: Wrapped in `Locked<VGA>`, which uses atomic CAS spin loop (`compare_exchange_weak`) for synchronization.
pub static WRITER: Locked<VGA> = Locked::new(VGA::new());

/// 4-bit VGA Color Codes enumeration.
///
/// - **WHAT**: Standard 16-color VGA palette representation.
/// - **WHY**: Allows foreground text and background cells to be styled using human-readable names.
/// - **WHEN**: Specified when calling `set_color()`.
/// - **HOW**: Packed into an 8-bit attribute byte `(background << 4) | foreground`.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Color {
    Black = 0,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    LightGray,
    DarkGray,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    Pink,
    Yellow,
    White,
}

/// VGA Text Mode Controller State.
///
/// - **WHAT**: Tracks hardware cursor coordinates, text attributes, and hardware visibility.
/// - **WHY**: Maintains active cursor position across calls so character printing advances sequentially.
/// - **WHEN**: Persists globally in `WRITER`.
/// - **HOW**: Updates internal `row` and `column` fields, flushing positions to CRT hardware ports via `update_cursor()`.
pub struct VGA {
    row: usize,
    column: usize,
    foreground: Color,
    background: Color,
    cursor_visible: bool,
}

impl VGA {
    /// Constructs a new VGA state struct with default settings.
    pub const fn new() -> Self {
        Self {
            row: 0,
            column: 0,
            foreground: Color::White,
            background: Color::Black,
            cursor_visible: true,
        }
    }

    /// Initializes VGA display state upon system boot.
    ///
    /// - **WHAT**: Clears all 80x25 cells on screen to blank spaces and updates cursor location to (0,0).
    /// - **WHY**: Erases residual BIOS text and prepares a clean canvas for kernel boot.
    /// - **WHEN**: Called during `Kernel::initialize()`.
    /// - **HOW**: Calls `clear()` and flushes state via `update_cursor()`.
    pub fn init(&mut self) {
        self.clear();
        self.update_cursor();
    }

    /// Clears the entire 80x25 display grid.
    ///
    /// - **WHAT**: Fills all 2000 character cells with space bytes (`0x20`) and active background color.
    /// - **WHY**: Resets display for `clear` command or interactive applications.
    /// - **WHEN**: Executed on boot, shell `clear` command, or application exit.
    /// - **HOW**: Iterates rows 0..25 and cols 0..80 writing volatile cell bytes to memory address 0xB8000.
    pub fn clear(&mut self) {
        let color = self.color_code();
        for row in 0..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                self.write_cell(row, col, b' ', color);
            }
        }
        self.row = 0;
        self.column = 0;
        self.update_cursor();
    }

    /// Clears current line content from active column to end of row.
    pub fn clear_line(&mut self) {
        let color = self.color_code();
        for col in 0..BUFFER_WIDTH {
            self.write_cell(self.row, col, b' ', color);
        }
        self.column = 0;
        self.update_cursor();
    }

    /// Writes a UTF-8 string slice to VGA memory.
    ///
    /// - **WHAT**: Iterates string characters and outputs each character.
    /// - **WHY**: Primary helper for text output across kernel loggers and shell commands.
    /// - **WHEN**: Called whenever string text is printed.
    /// - **HOW**: Loops over `text.chars()`, dispatching each `char` to `put_char()`.
    pub fn write(&mut self, text: &str) {
        for c in text.chars() {
            self.put_char(c);
        }
    }

    /// Writes a UTF-8 string slice and appends a newline character.
    pub fn println(&mut self, text: &str) {
        self.write(text);
        self.newline();
    }

    /// Outputs a single character to the current cursor position.
    ///
    /// - **WHAT**: Handles newline logic or places ASCII character bytes at active `(row, column)`.
    /// - **WHY**: Low-level building block for all VGA output operations.
    /// - **WHEN**: Called by `write()`, `println()`, or shell typing handlers.
    /// - **HOW**:
    ///   - If `c == '\n'`, triggers `newline()`.
    ///   - If `column >= 80`, wraps to next line via `newline()`.
    ///   - Writes byte to memory offset, increments `column`, and updates hardware CRT cursor.
    pub fn put_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            _ => {
                if self.column >= BUFFER_WIDTH {
                    self.newline();
                }

                self.put_char_at(self.row, self.column, c);
                self.column += 1;
                self.update_cursor();
            }
        }
    }

    /// Places a character byte directly at absolute `(row, col)` coordinates without advancing internal cursor state.
    ///
    /// - **WHAT**: Writes character and color byte to designated coordinate cell.
    /// - **WHY**: Used by full-screen applications (like games or interactive chess) that draw UI elements at fixed grid locations.
    /// - **WHEN**: Called by applications rendering grid-based UI layouts.
    /// - **HOW**: Performs bounds check `row < 25 && col < 80` and writes cell to 0xB8000 address offset.
    pub fn put_char_at(&self, row: usize, col: usize, c: char) {
        if row >= BUFFER_HEIGHT || col >= BUFFER_WIDTH {
            return;
        }

        self.write_cell(row, col, c as u8, self.color_code());
    }

    /// Deletes character behind active cursor position.
    ///
    /// - **WHAT**: Moves cursor back one column and overwrites cell with space character.
    /// - **WHY**: Supports keyboard backspace input in interactive command prompt.
    /// - **WHEN**: Triggered by shell when user presses `Backspace`.
    /// - **HOW**: Decrements `column` (or moves up to end of previous line if column is 0), writes `b' '`, and updates hardware cursor.
    pub fn backspace(&mut self) {
        if self.column > 0 {
            self.column -= 1;
            self.write_cell(self.row, self.column, b' ', self.color_code());
        } else if self.row > 0 {
            self.row -= 1;
            self.column = BUFFER_WIDTH - 1;
            self.write_cell(self.row, self.column, b' ', self.color_code());
        }
        self.update_cursor();
    }

    /// Configures active text foreground and background color styling.
    pub fn set_color(&mut self, fg: Color, bg: Color) {
        self.foreground = fg;
        self.background = bg;
    }

    pub fn get_cursor_position(&self) -> (usize, usize) {
        (self.row, self.column)
    }

    pub fn set_cursor_position(&mut self, row: usize, col: usize) {
        self.row = row.min(BUFFER_HEIGHT - 1);
        self.column = col.min(BUFFER_WIDTH - 1);
        self.update_cursor();
    }

    pub fn show_cursor(&mut self) {
        self.cursor_visible = true;
        self.update_cursor();
    }

    pub fn hide_cursor(&mut self) {
        self.cursor_visible = false;
        unsafe {
            outb(0x3D4, 0x0A);
            outb(0x3D5, 0x20);
        }
    }

    /// Advances cursor to start of next line or triggers full screen scroll.
    fn newline(&mut self) {
        self.column = 0;
        if self.row < BUFFER_HEIGHT - 1 {
            self.row += 1;
        } else {
            self.scroll();
        }
        self.update_cursor();
    }

    /// Scrolls display contents up by one line.
    ///
    /// - **WHAT**: Copies rows 1..25 up to rows 0..24 and clears row 24 with space bytes.
    /// - **WHY**: Prevents output from overflowing bottom of VGA hardware buffer.
    /// - **WHEN**: Triggered by `newline()` when `row == 24`.
    /// - **HOW**: Performs volatile 16-bit word copies from `(row * 80 + col)` to `((row - 1) * 80 + col)`.
    fn scroll(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                unsafe {
                    let from = (BUFFER_ADDRESS as *mut u16).add(row * BUFFER_WIDTH + col);
                    let to = (BUFFER_ADDRESS as *mut u16).add((row - 1) * BUFFER_WIDTH + col);
                    let value = read_volatile(from);
                    write_volatile(to, value);
                }
            }
        }

        for col in 0..BUFFER_WIDTH {
            self.write_cell(BUFFER_HEIGHT - 1, col, b' ', self.color_code());
        }

        self.row = BUFFER_HEIGHT - 1;
        self.column = 0;
    }

    /// Writes raw ASCII byte and attribute color byte to hardware VGA MMIO offset.
    fn write_cell(&self, row: usize, col: usize, byte: u8, color: u8) {
        let offset = (row * BUFFER_WIDTH + col) * 2;
        unsafe {
            write_volatile((BUFFER_ADDRESS + offset) as *mut u8, byte);
            write_volatile((BUFFER_ADDRESS + offset + 1) as *mut u8, color);
        }
    }

    /// Packs background and foreground colors into single 8-bit attribute byte.
    fn color_code(&self) -> u8 {
        ((self.background as u8) << 4) | (self.foreground as u8)
    }

    /// Flushes current `(row, column)` coordinates to hardware VGA CRT controller registers.
    ///
    /// - **WHAT**: Writes 16-bit offset `(row * 80 + column)` to I/O ports 0x3D4 / 0x3D5.
    /// - **WHY**: Moves hardware flashing underline cursor on screen to match driver state.
    /// - **WHEN**: Called after any character write, newline, backspace, or clear operation.
    /// - **HOW**:
    ///   - Writes register 0x0F to port 0x3D4, sends low byte offset to 0x3D5.
    ///   - Writes register 0x0E to port 0x3D4, sends high byte offset to 0x3D5.
    fn update_cursor(&self) {
        if !self.cursor_visible {
            return;
        }

        let pos = (self.row * BUFFER_WIDTH + self.column) as u16;

        unsafe {
            outb(0x3D4, 0x0F);
            outb(0x3D5, (pos & 0xFF) as u8);

            outb(0x3D4, 0x0E);
            outb(0x3D5, ((pos >> 8) & 0xFF) as u8);

            outb(0x3D4, 0x0A);
            outb(0x3D5, 0);
        }
    }
}

/// Assembly wrapper for writing a byte to an x86 I/O port.
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nostack, nomem)
    );
}

/// Generic thread-safe spinlock wrapper primitive.
pub struct Locked<T> {
    inner: spin::Mutex<T>,
}

impl<T> Locked<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: spin::Mutex::new(value),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<T> {
        self.inner.lock()
    }
}

mod spin {
    use core::sync::atomic::{AtomicBool, Ordering};
    use core::cell::UnsafeCell;
    use core::ops::{Deref, DerefMut};

    pub struct Mutex<T> {
        locked: AtomicBool,
        value: UnsafeCell<T>,
    }

    unsafe impl<T: Send> Sync for Mutex<T> {}

    impl<T> Mutex<T> {
        pub const fn new(value: T) -> Self {
            Self {
                locked: AtomicBool::new(false),
                value: UnsafeCell::new(value),
            }
        }

        pub fn lock(&self) -> MutexGuard<T> {
            while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
                core::hint::spin_loop();
            }
            MutexGuard { mutex: self }
        }
    }

    pub struct MutexGuard<'a, T> {
        mutex: &'a Mutex<T>,
    }

    impl<'a, T> Deref for MutexGuard<'a, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            unsafe { &*self.mutex.value.get() }
        }
    }

    impl<'a, T> DerefMut for MutexGuard<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { &mut *self.mutex.value.get() }
        }
    }

    impl<'a, T> Drop for MutexGuard<'a, T> {
        fn drop(&mut self) {
            self.mutex.locked.store(false, Ordering::Release);
        }
    }
}
