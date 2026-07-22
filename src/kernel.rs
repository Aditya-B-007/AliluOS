use crate::vga::{Color, VGA};
use crate::keyboard::{Key, KeyEvent, Keyboard};

pub struct Kernel {
    vga: VGA,
    keyboard: Keyboard,
    shell: crate::shell::Shell,
}

impl Kernel {
    pub fn new() -> Self {
        Self {
            vga: VGA::new(),
            keyboard: Keyboard::new(),
            shell: crate::shell::Shell::new(),
        }
    }

    pub fn initialize(&mut self) {
        self.vga.init();
        self.keyboard.init();
        crate::interrupts::init();
        crate::allocator::init_heap();
        self.boot_banner();
    }

    /// Main kernel execution loop.
    ///
    /// For now this simply idles forever.
    /// Later this will become:
    /// - Read Keyboard
    /// - Parse Commands
    /// - Execute Commands
    /// - Update Display

    
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
        self.vga.set_color(Color::LightGreen, Color::Black);

        self.vga.println("========================================");
        self.vga.println("          Welcome to AliluOS");
        self.vga.println("========================================");

        self.vga.set_color(Color::White, Color::Black);

        self.vga.println("");
        self.vga.println("Kernel Initialized Successfully.");
        self.vga.println("");
        self.vga.write("> ");
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
            _ => {}
        }
    }

    
}
