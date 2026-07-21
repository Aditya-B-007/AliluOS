#![allow(dead_code)]

use core::arch::asm;
///Remember to switch to IRQ1 when interrupt is introduced. Uses PS/2 controller.
/// High-level key values returned by the keyboard driver.
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

/// Press/Release event.
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
        self.num_lock = false;
        self.scroll_lock = false;
    }

    pub fn shutdown(&mut self) {}

    pub fn is_shift_pressed(&self) -> bool { self.shift }
    pub fn is_ctrl_pressed(&self) -> bool { self.ctrl }
    pub fn is_alt_pressed(&self) -> bool { self.alt }

    /// Polls the keyboard controller.
    /// Returns None when no key is available.
    pub fn read_key(&mut self) -> Option<KeyEvent> {
        if !self.output_ready() {
            return None;
        }

        let scancode = self.read_scancode();
        Some(self.translate(scancode))
    }

    fn output_ready(&self) -> bool {
        unsafe {
            let mut status: u8;
            asm!(
                "in al, dx",
                in("dx") 0x64u16,
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
                in("dx") 0x60u16,
                out("al") value,
                options(nomem, nostack)
            );
            value
        }
    }

    fn translate(&mut self, scancode: u8) -> KeyEvent {
        let released = scancode & 0x80 != 0;
        let code = scancode & 0x7F;

        let key = match code {
            0x01 => Key::Escape,
            0x0E => Key::Backspace,
            0x0F => Key::Tab,
            0x1C => Key::Enter,
            0x39 => Key::Space,

            0x2A | 0x36 => {
                self.shift = !released;
                Key::Shift
            }

            0x1D => {
                self.ctrl = !released;
                Key::Ctrl
            }

            0x38 => {
                self.alt = !released;
                Key::Alt
            }

            0x3A => {
                if !released {
                    self.caps_lock = !self.caps_lock;
                }
                Key::CapsLock
            }

            // Numbers
            0x02 => Key::Character(if self.shift {'!'} else {'1'}),
            0x03 => Key::Character(if self.shift {'@'} else {'2'}),
            0x04 => Key::Character(if self.shift {'#'} else {'3'}),
            0x05 => Key::Character(if self.shift {'$'} else {'4'}),
            0x06 => Key::Character(if self.shift {'%'} else {'5'}),
            0x07 => Key::Character(if self.shift {'^'} else {'6'}),
            0x08 => Key::Character(if self.shift {'&'} else {'7'}),
            0x09 => Key::Character(if self.shift {'*'} else {'8'}),
            0x0A => Key::Character(if self.shift {'('} else {'9'}),
            0x0B => Key::Character(if self.shift {')'} else {'0'}),

            // Letters (set 1)
            0x10 => Key::Character(self.letter('q')),
            0x11 => Key::Character(self.letter('w')),
            0x12 => Key::Character(self.letter('e')),
            0x13 => Key::Character(self.letter('r')),
            0x14 => Key::Character(self.letter('t')),
            0x15 => Key::Character(self.letter('y')),
            0x16 => Key::Character(self.letter('u')),
            0x17 => Key::Character(self.letter('i')),
            0x18 => Key::Character(self.letter('o')),
            0x19 => Key::Character(self.letter('p')),

            0x1E => Key::Character(self.letter('a')),
            0x1F => Key::Character(self.letter('s')),
            0x20 => Key::Character(self.letter('d')),
            0x21 => Key::Character(self.letter('f')),
            0x22 => Key::Character(self.letter('g')),
            0x23 => Key::Character(self.letter('h')),
            0x24 => Key::Character(self.letter('j')),
            0x25 => Key::Character(self.letter('k')),
            0x26 => Key::Character(self.letter('l')),

            0x2C => Key::Character(self.letter('z')),
            0x2D => Key::Character(self.letter('x')),
            0x2E => Key::Character(self.letter('c')),
            0x2F => Key::Character(self.letter('v')),
            0x30 => Key::Character(self.letter('b')),
            0x31 => Key::Character(self.letter('n')),
            0x32 => Key::Character(self.letter('m')),

            // F1-F12
            0x3B..=0x44 => Key::Function(code - 0x3A),
            0x57 => Key::Function(11),
            0x58 => Key::Function(12),

            _ => Key::Unknown,
        };

        if released {
            KeyEvent::Release(key)
        } else {
            KeyEvent::Press(key)
        }
    }

    fn letter(&self, c: char) -> char {
        let upper = self.shift ^ self.caps_lock;
        if upper {
            c.to_ascii_uppercase()
        } else {
            c
        }
    }
}
