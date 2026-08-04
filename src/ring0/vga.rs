//! # AliluOS VGA Text Mode Display Driver (`ring0/vga.rs`)
//!
//! - **WHAT**: Hardware driver for x86 80x25 VGA text mode display and cursor hardware ports.
//! - **WHY**: Provides text output capability (printing strings, character backspacing, scrolling, line clearing, colors)
//!   before complex graphics drivers are initialized.
//! - **WHEN**: Called whenever the kernel, shell, panic handler, exception handler, or games render text to the screen.
//! - **HOW**: Directly reads and writes volatile memory at physical VGA memory address `0xB8000` and configures CRT controller I/O ports (`0x3D4` / `0x3D5`).

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};
use crate::config::vga::*;
use core::arch::asm;

unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

/// Global synchronized VGA driver instance.
pub static WRITER: Locked<VGA> = Locked::new(VGA::new());

/// 4-bit VGA Color Codes enumeration.
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
pub struct VGA {
    row: usize,
    column: usize,
    foreground: Color,
    background: Color,
    cursor_visible: bool,
}

impl VGA {
    pub const fn new() -> Self {
        Self {
            row: 0,
            column: 0,
            foreground: Color::White,
            background: Color::Black,
            cursor_visible: true,
        }
    }

    pub fn init(&mut self) {
        self.clear();
    }

    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.foreground = foreground;
        self.background = background;
    }

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

    pub fn clear_line(&mut self) {
        let color = self.color_code();
        for col in 0..BUFFER_WIDTH {
            self.write_cell(self.row, col, b' ', color);
        }
        self.column = 0;
        self.update_cursor();
    }

    pub fn write(&mut self, text: &str) {
        for c in text.chars() {
            self.put_char(c);
        }
    }

    pub fn println(&mut self, text: &str) {
        self.write(text);
        self.newline();
    }

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

    pub fn put_char_at(&self, row: usize, col: usize, c: char) {
        if row >= BUFFER_HEIGHT || col >= BUFFER_WIDTH {
            return;
        }

        self.write_cell(row, col, c as u8, self.color_code());
    }

    pub fn backspace(&mut self) {
        if self.column > 0 {
            self.column -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.column = BUFFER_WIDTH - 1;
        }
        self.write_cell(self.row, self.column, b' ', self.color_code());
        self.update_cursor();
    }

    fn newline(&mut self) {
        if self.row < BUFFER_HEIGHT - 1 {
            self.row += 1;
        } else {
            self.scroll();
        }
        self.column = 0;
        self.update_cursor();
    }

    fn scroll(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let (byte, color) = self.read_cell(row, col);
                self.write_cell(row - 1, col, byte, color);
            }
        }

        for col in 0..BUFFER_WIDTH {
            self.write_cell(BUFFER_HEIGHT - 1, col, b' ', self.color_code());
        }

        self.row = BUFFER_HEIGHT - 1;
        self.column = 0;
    }

    fn write_cell(&self, row: usize, col: usize, byte: u8, color: u8) {
        let offset = (row * BUFFER_WIDTH + col) * 2;
        unsafe {
            write_volatile((BUFFER_ADDRESS + offset) as *mut u8, byte);
            write_volatile((BUFFER_ADDRESS + offset + 1) as *mut u8, color);
        }
    }

    fn read_cell(&self, row: usize, col: usize) -> (u8, u8) {
        let offset = (row * BUFFER_WIDTH + col) * 2;
        unsafe {
            let byte = read_volatile((BUFFER_ADDRESS + offset) as *const u8);
            let color = read_volatile((BUFFER_ADDRESS + offset + 1) as *const u8);
            (byte, color)
        }
    }

    fn color_code(&self) -> u8 {
        ((self.background as u8) << 4) | (self.foreground as u8)
    }

    fn update_cursor(&self) {
        if !self.cursor_visible {
            return;
        }

        let pos = (self.row * BUFFER_WIDTH + self.column) as u16;

        unsafe {
            outb(CRTC_ADDR_PORT, 0x0F);
            outb(CRTC_DATA_PORT, (pos & 0xFF) as u8);

            outb(CRTC_ADDR_PORT, 0x0E);
            outb(CRTC_DATA_PORT, ((pos >> 8) & 0xFF) as u8);

            outb(CRTC_ADDR_PORT, 0x0A);
            outb(CRTC_DATA_PORT, 0);
        }
    }
}

/// Global thread-safe spinlock wrapper primitive.
pub struct Locked<T> {
    inner: spin::Mutex<T>,
}

impl<T> Locked<T> {
    pub const fn new(inner: T) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
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
