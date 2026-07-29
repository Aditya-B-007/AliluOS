#![allow(dead_code)]

use core::arch::asm;

/// High-level key values returned by the keyboard driver.
///
/// WHAT IT DOES:
/// Maps raw hardware PS/2 scan codes into strongly-typed enumeration values representing specific keyboard keys.
///
/// WHY IT DOES IT:
/// Decouples hardware-specific scan code bytes from high-level kernel logic and shell event handlers.
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

/// Press/Release key event representation.
///
/// WHAT IT DOES:
/// Indicates whether a key action was a key depression (press) or key release.
///
/// WHY IT DOES IT:
/// Allows the shell and kernel to react to key presses (typing) and track modifier state changes accurately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Press(Key),
    Release(Key),
}

/// Hardware PS/2 Keyboard Driver state manager.
///
/// WHAT IT DOES:
/// Manages modifier key state (Shift, Ctrl, Alt, CapsLock) and translates raw scancodes into `KeyEvent` instances.
///
/// WHY IT DOES IT:
/// Provides a clean non-blocking API (`read_key`) for the kernel main loop to consume keystrokes.
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
    /// Creates a new Keyboard driver instance with default modifier states (all unpressed).
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

    /// Initializes keyboard driver state.
    ///
    /// WHAT IT DOES: Resets all modifier toggles to false.
    /// WHY IT DOES IT: Guarantees a clean, predictable keyboard state upon kernel boot.
    pub fn init(&mut self) {
        self.reset();
    }

    /// Resets modifier keys (Shift, Ctrl, Alt, CapsLock).
    pub fn reset(&mut self) {
        self.shift = false;
        self.ctrl = false;
        self.alt = false;
        self.caps_lock = false;
        self.caps_lock_pressed = false;
        self.num_lock = false;
        self.scroll_lock = false;
    }

    pub fn shutdown(&mut self) {}

    pub fn is_shift_pressed(&self) -> bool { self.shift }
    pub fn is_ctrl_pressed(&self) -> bool { self.ctrl }
    pub fn is_alt_pressed(&self) -> bool { self.alt }

    /// Polls for the next available keyboard event.
    ///
    /// WHAT IT DOES: Pops a scancode strictly from the IRQ1 interrupt ring buffer (`pop_scancode()`).
    /// WHY IT DOES IT: Eliminates port 0x60 polling races, avoiding jitter and duplicate keypresses.
    pub fn read_key(&mut self) -> Option<KeyEvent> {
        // Priority 1: Read scancodes collected by IRQ1 interrupt handler
        if let Some(scancode) = crate::interrupts::pop_scancode() {
            return Some(self.translate(scancode));
        }

        // Priority 2: Fallback polling of PS/2 hardware output status
        if self.output_ready() {
            let scancode = self.read_scancode();
            return Some(self.translate(scancode));
        }

        None
    }

    /// Checks if PS/2 Controller output buffer has a pending byte ready.
    ///
    /// WHAT IT DOES: Reads I/O port 0x64 and tests bit 0 (Output Buffer Full).
    /// WHY IT DOES IT: Prevents reading garbage or stalling when no key has been pressed.
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

    /// Reads raw scancode byte directly from PS/2 data port 0x60.
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

    /// Translates raw 1-byte PS/2 Set 1 scancode into a `KeyEvent`.
    ///
    /// WHAT IT DOES:
    /// Evaluates key release bit (0x80), maps scan code to key enumeration, and updates internal shift/caps state.
    ///
    /// WHY IT DOES IT:
    /// Converts hardware-level bit patterns into semantic key events understood by the shell.
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
                    if !self.caps_lock_pressed {
                        self.caps_lock = !self.caps_lock;
                        self.caps_lock_pressed = true;
                    }
                } else {
                    self.caps_lock_pressed = false;
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

            // Punctuation & Special Symbols
            0x0C => Key::Character(if self.shift { '_' } else { '-' }),
            0x0D => Key::Character(if self.shift { '+' } else { '=' }),
            0x1A => Key::Character(if self.shift { '{' } else { '[' }),
            0x1B => Key::Character(if self.shift { '}' } else { ']' }),
            0x27 => Key::Character(if self.shift { ':' } else { ';' }),
            0x28 => Key::Character(if self.shift { '"' } else { '\'' }),
            0x29 => Key::Character(if self.shift { '~' } else { '`' }),
            0x2B => Key::Character(if self.shift { '|' } else { '\\' }),
            0x33 => Key::Character(if self.shift { '<' } else { ',' }),
            0x34 => Key::Character(if self.shift { '>' } else { '.' }),
            0x35 => Key::Character(if self.shift { '?' } else { '/' }),

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

    /// Handles casing transformation for alphabet characters based on Shift and CapsLock state.
    fn letter(&self, c: char) -> char {
        let is_upper = self.caps_lock || self.shift;
        if is_upper {
            c.to_ascii_uppercase()
        } else {
            c
        }
    }
}
