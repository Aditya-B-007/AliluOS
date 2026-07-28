use crate::vga::{Color, WRITER};
use crate::keyboard::{Key, KeyEvent, Keyboard};

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
        WRITER.lock().init();
        self.keyboard.init();
        crate::interrupts::init();
        crate::allocator::init_heap();
        self.boot_banner();
    }

    /// Main kernel execution loop.
    pub fn run(&mut self) -> ! {
        loop {
            if let Some(event) = self.keyboard.read_key() {
                self.handle_key_event(event);
            }
        }
    }

    // ----------------------------------------------------
    // Private Methods
    // ----------------------------------------------------

    /// Displays the boot banner.
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

    fn handle_key_event(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Press(key) => {
                self.handle_key_press(key);
            }
            KeyEvent::Release(_) => {}
        }
    }

    fn handle_key_press(&mut self, key: Key) {
        match key {
            Key::Character(c) => {
                self.shell.handle_char(c);
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
            _ => {}
        }
    }
}
