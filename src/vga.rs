#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};

const BUFFER_ADDRESS: usize = 0xB8000;
const BUFFER_WIDTH: usize = 80;
const BUFFER_HEIGHT: usize = 25;

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
        self.update_cursor();
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
            self.write_cell(self.row, self.column, b' ', self.color_code());
        } else if self.row > 0 {
            self.row -= 1;
            self.column = BUFFER_WIDTH - 1;
            self.write_cell(self.row, self.column, b' ', self.color_code());
        }
        self.update_cursor();
    }

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


    //Commands used by the methods above, and are not exposed to the external world.
    
    fn newline(&mut self) {
        self.column = 0;
        if self.row < BUFFER_HEIGHT - 1 {
            self.row += 1;
        } else {
            self.scroll();
        }
        self.update_cursor();
    }

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

    fn write_cell(&self, row: usize, col: usize, byte: u8, color: u8) {
        let offset = (row * BUFFER_WIDTH + col) * 2;
        unsafe {
            write_volatile((BUFFER_ADDRESS + offset) as *mut u8, byte);
            write_volatile((BUFFER_ADDRESS + offset + 1) as *mut u8, color);
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
            outb(0x3D4, 0x0F);
            outb(0x3D5, (pos & 0xFF) as u8);

            outb(0x3D4, 0x0E);
            outb(0x3D5, ((pos >> 8) & 0xFF) as u8);

            outb(0x3D4, 0x0A);
            outb(0x3D5, 0);
        }
    }
}

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nostack, nomem)
    );
}
