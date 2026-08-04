//! # AliluOS PS/2 Keyboard Hardware Driver (`ring0/keyboard.rs`)

#![allow(dead_code)]

use core::arch::asm;
use crate::config::keyboard::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Character(char),
    Enter,
    Backspace,
    Tab,
    Escape,
    Space,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Insert,
    Delete,
    PageUp,
    PageDown,
    Function(u8),
    Shift,
    Ctrl,
    Alt,
    CapsLock,
    NumLock,
    ScrollLock,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Press(Key),
    Release(Key),
}

pub struct Keyboard {
    shift: bool,
    ctrl: bool,
    alt: bool,
    caps_lock: bool,
    caps_lock_pressed: bool,
    num_lock: bool,
    scroll_lock: bool,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            shift: false,
            ctrl: false,
            alt: false,
            caps_lock: false,
            caps_lock_pressed: false,
            num_lock: false,
            scroll_lock: false,
        }
    }

    pub fn init(&mut self) {
        self.reset();
    }

    pub fn reset(&mut self) {
        self.shift = false;
        self.ctrl = false;
        self.alt = false;
        self.caps_lock = false;
        self.caps_lock_pressed = false;
        self.num_lock = false;
        self.scroll_lock = false;
    }

    pub fn is_shift_pressed(&self) -> bool { self.shift }
    pub fn is_ctrl_pressed(&self) -> bool { self.ctrl }
    pub fn is_alt_pressed(&self) -> bool { self.alt }

    pub fn read_key(&mut self) -> Option<KeyEvent> {
        if let Some(scancode) = crate::interrupts::pop_scancode() {
            return Some(self.translate(scancode));
        }

        if self.output_ready() {
            let scancode = self.read_scancode();
            return Some(self.translate(scancode));
        }

        None
    }

    fn output_ready(&self) -> bool {
        unsafe {
            let mut status: u8;
            asm!(
                "in al, dx",
                in("dx") STATUS_PORT,
                out("al") status,
                options(nomem, nostack)
            );
            status & 1 != 0
        }
    }

    fn read_scancode(&self) -> u8 {
        unsafe {
            let mut value: u8;
            asm!(
                "in al, dx",
                in("dx") DATA_PORT,
                out("al") value,
                options(nomem, nostack)
            );
            value
        }
    }

    fn translate(&mut self, scancode: u8) -> KeyEvent {
        let release = scancode & 0x80 != 0;
        let code = scancode & 0x7F;

        let key = match code {
            0x01 => Key::Escape,
            0x0E => Key::Backspace,
            0x0F => Key::Tab,
            0x1C => Key::Enter,
            0x1D => {
                self.ctrl = !release;
                Key::Ctrl
            }
            0x2A | 0x36 => {
                self.shift = !release;
                Key::Shift
            }
            0x38 => {
                self.alt = !release;
                Key::Alt
            }
            0x39 => Key::Space,
            0x3A => {
                if !release {
                    if !self.caps_lock_pressed {
                        self.caps_lock = !self.caps_lock;
                        self.caps_lock_pressed = true;
                    }
                } else {
                    self.caps_lock_pressed = false;
                }
                Key::CapsLock
            }
            0x47 => Key::Home,
            0x48 => Key::Up,
            0x49 => Key::PageUp,
            0x4B => Key::Left,
            0x4D => Key::Right,
            0x4F => Key::End,
            0x50 => Key::Down,
            0x51 => Key::PageDown,
            0x52 => Key::Insert,
            0x53 => Key::Delete,
            _ => self.translate_char(code),
        };

        if release {
            KeyEvent::Release(key)
        } else {
            KeyEvent::Press(key)
        }
    }

    fn translate_char(&self, code: u8) -> Key {
        let shift_active = self.shift ^ self.caps_lock;

        let c = match code {
            0x02 => if self.shift { '!' } else { '1' },
            0x03 => if self.shift { '@' } else { '2' },
            0x04 => if self.shift { '#' } else { '3' },
            0x05 => if self.shift { '$' } else { '4' },
            0x06 => if self.shift { '%' } else { '5' },
            0x07 => if self.shift { '^' } else { '6' },
            0x08 => if self.shift { '&' } else { '7' },
            0x09 => if self.shift { '*' } else { '8' },
            0x0A => if self.shift { '(' } else { '9' },
            0x0B => if self.shift { ')' } else { '0' },
            0x0C => if self.shift { '_' } else { '-' },
            0x0D => if self.shift { '+' } else { '=' },

            0x10 => if shift_active { 'Q' } else { 'q' },
            0x11 => if shift_active { 'W' } else { 'w' },
            0x12 => if shift_active { 'E' } else { 'e' },
            0x13 => if shift_active { 'R' } else { 'r' },
            0x14 => if shift_active { 'T' } else { 't' },
            0x15 => if shift_active { 'Y' } else { 'y' },
            0x16 => if shift_active { 'U' } else { 'u' },
            0x17 => if shift_active { 'I' } else { 'i' },
            0x18 => if shift_active { 'O' } else { 'o' },
            0x19 => if shift_active { 'P' } else { 'p' },
            0x1A => if self.shift { '{' } else { '[' },
            0x1B => if self.shift { '}' } else { ']' },

            0x1E => if shift_active { 'A' } else { 'a' },
            0x1F => if shift_active { 'S' } else { 's' },
            0x20 => if shift_active { 'D' } else { 'd' },
            0x21 => if shift_active { 'F' } else { 'f' },
            0x22 => if shift_active { 'G' } else { 'g' },
            0x23 => if shift_active { 'H' } else { 'h' },
            0x24 => if shift_active { 'J' } else { 'j' },
            0x25 => if shift_active { 'K' } else { 'k' },
            0x26 => if shift_active { 'L' } else { 'l' },
            0x27 => if self.shift { ':' } else { ';' },
            0x28 => if self.shift { '"' } else { '\'' },
            0x29 => if self.shift { '~' } else { '`' },
            0x2B => if self.shift { '|' } else { '\\' },

            0x2C => if shift_active { 'Z' } else { 'z' },
            0x2D => if shift_active { 'X' } else { 'x' },
            0x2E => if shift_active { 'C' } else { 'c' },
            0x2F => if shift_active { 'V' } else { 'v' },
            0x30 => if shift_active { 'B' } else { 'b' },
            0x31 => if shift_active { 'N' } else { 'n' },
            0x32 => if shift_active { 'M' } else { 'm' },
            0x33 => if self.shift { '<' } else { ',' },
            0x34 => if self.shift { '>' } else { '.' },
            0x35 => if self.shift { '?' } else { '/' },

            _ => return Key::Unknown,
        };

        Key::Character(c)
    }
}
