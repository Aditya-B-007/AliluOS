//! # AliluOS Interactive Shell Engine (`shell.rs`)
//!
//! - **WHAT**: Command processor, line buffer manager, interactive text editor, and Help Viewer for AliluOS.
//! - **WHY**: Provides human-readable plain English command input, file operations, system stats, and interactive documentation.
//! - **WHEN**: Triggered on every user keystroke dispatched from `Kernel::handle_key_press()`.
//! - **HOW**: Maintains state for active `ShellMode` (`Command`, `Editor`, `HelpViewer`), parses input strings on `Enter`,
//!   and interacts with the global B-Tree filesystem (`FS`) and synchronized VGA console (`WRITER`).

use alloc::string::String;
use alloc::vec::Vec;
use crate::vga::{Color, WRITER, VGA};
use crate::fs::{FS, Node};

/// Shell Operating Modes enumeration.
///
/// - **WHAT**: Tracks whether the user is typing standard commands (`Command`), editing a file (`Editor`), or viewing help (`HelpViewer`).
/// - **WHY**: Input keys (`Enter`, `Backspace`, `Escape`, characters) behave differently depending on active mode.
#[derive(PartialEq, Eq)]
enum ShellMode {
    Command,
    Editor,
    HelpViewer,
}

/// Main Shell Engine State.
///
/// - **WHAT**: Stores active line buffer, active mode, current working directory (CWD) path segments, and editor state.
/// - **WHY**: Retains user context across keystrokes.
pub struct Shell {
    buffer: String,             // Buffer storing current command line input
    mode: ShellMode,            // Active mode (Command vs. Editor vs. HelpViewer)
    editor_filename: String,    // Name of file currently being edited
    editor_buffer: String,      // Buffer storing accumulating text in editor mode
    cwd: Vec<String>,           // Current Working Directory path segments
}

impl Shell {
    /// Constructs a new Shell instance with root directory `CWD = []`.
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            mode: ShellMode::Command,
            editor_filename: String::new(),
            editor_buffer: String::new(),
            cwd: Vec::new(),
        }
    }

    /// Handles Escape key press.
    ///
    /// - **WHAT**:
    ///   - In `HelpViewer` mode: exits help view, clears screen, and restores command prompt `> `.
    ///   - In `Command` mode: clears active command input line (`^C`).
    /// - **WHY**: Provides a dedicated, predictable key action to close modal overlays or reset command line input.
    /// - **WHEN**: Triggered whenever the user presses `Escape`.
    pub fn handle_escape(&mut self) {
        match self.mode {
            ShellMode::HelpViewer => {
                self.mode = ShellMode::Command;
                let mut vga = WRITER.lock();
                vga.clear();
                vga.set_color(Color::LightGreen, Color::Black);
                vga.println("=== AliluOS Interactive Shell ===");
                vga.set_color(Color::White, Color::Black);
                vga.println("Type 'help' to reopen help viewer, or enter commands below.\n");
                vga.write("> ");
            }
            ShellMode::Command => {
                if !self.buffer.is_empty() {
                    self.buffer.clear();
                    let mut vga = WRITER.lock();
                    vga.println("^C");
                    vga.write("> ");
                }
            }
            ShellMode::Editor => {}
        }
    }

    /// Handles character input keypresses.
    ///
    /// - **WHAT**: Appends character to active buffer (`buffer` or `editor_buffer`) and echoes to VGA.
    /// - **WHY**: Accumulates text for command execution or file editing.
    /// - **WHEN**: Called for character keystrokes.
    pub fn handle_char(&mut self, c: char) {
        let mut vga = WRITER.lock();
        match self.mode {
            ShellMode::Command => {
                self.buffer.push(c);
                vga.put_char(c);
            }
            ShellMode::Editor => {
                self.editor_buffer.push(c);
                vga.put_char(c);
            }
            ShellMode::HelpViewer => {
                // Keystrokes ignored in HelpViewer mode until Escape is pressed
            }
        }
    }

    /// Handles Backspace keypresses.
    ///
    /// - **WHAT**: Removes last character from buffer and erases character on VGA display.
    /// - **WHY**: Corrects typing mistakes in command line or text editor.
    /// - **WHEN**: Called when user presses `Backspace`.
    pub fn handle_backspace(&mut self) {
        let mut vga = WRITER.lock();
        match self.mode {
            ShellMode::Command => {
                if !self.buffer.is_empty() {
                    self.buffer.pop();
                    vga.backspace();
                }
            }
            ShellMode::Editor => {
                if !self.editor_buffer.is_empty() {
                    self.editor_buffer.pop();
                    vga.backspace();
                }
            }
            ShellMode::HelpViewer => {}
        }
    }

    /// Handles Space keypresses.
    pub fn handle_space(&mut self) {
        let mut vga = WRITER.lock();
        match self.mode {
            ShellMode::Command => {
                self.buffer.push(' ');
                vga.put_char(' ');
            }
            ShellMode::Editor => {
                self.editor_buffer.push(' ');
                vga.put_char(' ');
            }
            ShellMode::HelpViewer => {}
        }
    }

    /// Handles Enter keypresses.
    ///
    /// - **WHAT**:
    ///   - In `Command` mode: executes command stored in `buffer` and re-prints `> `.
    ///   - In `Editor` mode: inserts newline or evaluates `:wq` / `:q` exit commands.
    /// - **WHY**: Commits user input for execution or text persistence.
    /// - **WHEN**: Called when user presses `Enter`.
    pub fn handle_enter(&mut self) {
        let mut vga = WRITER.lock();
        vga.put_char('\n');

        match self.mode {
            ShellMode::Command => {
                let cmd_str = self.buffer.clone();
                self.buffer.clear();
                drop(vga);
                self.execute_command(&cmd_str);
                if self.mode == ShellMode::Command {
                    WRITER.lock().write("> ");
                }
            }
            ShellMode::Editor => {
                let lines: Vec<&str> = self.editor_buffer.lines().collect();
                if let Some(&last_line) = lines.last() {
                    let trimmed = last_line.trim();
                    if trimmed == ":wq" {
                        let command_len = last_line.len();
                        for _ in 0..command_len {
                            self.editor_buffer.pop();
                        }
                        
                        let target_path = self.editor_filename.clone();
                        let mut fs = FS.lock();
                        let resolved = fs.resolve_path(&self.cwd, &target_path);
                        if resolved.is_empty() {
                            vga.println("Error: Cannot write to root");
                        } else {
                            let (parent_segments, file_name) = resolved.split_at(resolved.len() - 1);
                            let content = self.editor_buffer.clone();
                            match fs.write_file(parent_segments, &file_name[0], &content) {
                                Ok(_) => {
                                    vga.set_color(Color::LightGreen, Color::Black);
                                    vga.println("\nFile saved successfully.");
                                }
                                Err(e) => {
                                    vga.set_color(Color::LightRed, Color::Black);
                                    vga.println(e);
                                }
                            }
                        }
                        vga.set_color(Color::White, Color::Black);
                        self.mode = ShellMode::Command;
                        self.editor_buffer.clear();
                        self.editor_filename.clear();
                        vga.write("> ");
                    } else if trimmed == ":q" {
                        vga.set_color(Color::Yellow, Color::Black);
                        vga.println("\nExited without saving.");
                        vga.set_color(Color::White, Color::Black);
                        self.mode = ShellMode::Command;
                        self.editor_buffer.clear();
                        self.editor_filename.clear();
                        vga.write("> ");
                    } else {
                        self.editor_buffer.push('\n');
                    }
                } else {
                    self.editor_buffer.push('\n');
                }
            }
            ShellMode::HelpViewer => {}
        }
    }

    /// Helper printing current working directory path.
    fn print_cwd_path(&self, vga: &mut VGA) {
        if self.cwd.is_empty() {
            vga.println("/");
        } else {
            for segment in &self.cwd {
                vga.write("/");
                vga.write(segment);
            }
            vga.println("");
        }
    }

    /// Command Line Parser and Executor.
    ///
    /// - **WHAT**: Parses command string into command name and arguments, then dispatches to subsystem handlers.
    /// - **WHY**: Core CLI interface for AliluOS.
    /// - **WHEN**: Triggered by `handle_enter()` in `Command` mode.
    /// - **HOW**: Matches command strings (`help`, `clear`, `system`, `tasks`, `directory`, `enter`, `folder`, `list`, `create`, `write`, `read`, `delete`, `edit`, `play`, `draw`, `echo`).
    fn execute_command(&mut self, cmd_line: &str) {
        let trimmed = cmd_line.trim();
        if trimmed.is_empty() {
            return;
        }

        let mut parts = trimmed.split_whitespace();
        let command = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();

        // Launch full-screen Interactive Help Viewer on `help` or `--help`
        if command == "help" || args.contains(&"--help") || args.contains(&"-h") {
            self.mode = ShellMode::HelpViewer;
            let mut vga = WRITER.lock();
            vga.clear();
            vga.set_color(Color::LightCyan, Color::Black);
            vga.println("========================================================================");
            vga.println("                    AliluOS INTERACTIVE HELP VIEWER                     ");
            vga.println("========================================================================");
            vga.set_color(Color::White, Color::Black);
            vga.println("");
            vga.println("  help                       - Launch this interactive help viewer");
            vga.println("  clear                      - Clear display screen");
            vga.println("  system                     - Display system specs, memory & timer stats");
            vga.println("  tasks                      - List active kernel CPU tasks");
            vga.println("  list                       - List files/folders (B-Tree sorted)");
            vga.println("  directory                  - Print current working directory path");
            vga.println("  enter [path]               - Change current working directory");
            vga.println("  folder [name]              - Create a new directory folder in B-Tree");
            vga.println("  create [file]              - Create a new file in B-Tree index");
            vga.println("  write [file] [text]        - Write text content to a file");
            vga.println("  read [file]                - View file contents from B-Tree index");
            vga.println("  delete [file/folder]       - Delete a file or folder from B-Tree index");
            vga.println("  edit [file]                - Open interactive text editor");
            vga.println("  play [atari / chess]       - Launch built-in text game");
            vga.println("  draw                       - Launch drawing canvas tool");
            vga.println("  echo [text]                - Print text back to screen");
            vga.println("");
            vga.set_color(Color::Yellow, Color::Black);
            vga.println("------------------------------------------------------------------------");
            vga.println("  Press [Escape] to close Help Viewer and return to Command Line Prompt");
            vga.println("------------------------------------------------------------------------");
            vga.set_color(Color::White, Color::Black);
            return;
        }

        let mut vga = WRITER.lock();

        match command {
            "clear" => {
                vga.clear();
            }
            "system" => {
                vga.set_color(Color::LightGreen, Color::Black);
                vga.println("--- AliluOS System Info ---");
                vga.set_color(Color::White, Color::Black);
                vga.println("OS Name: AliluOS (ಅಳಿಲು)");
                vga.println("Architecture: x86_64 Bare-Metal");
                vga.println("Platform: Standard PC compatible");
                vga.println("Heap Status: 100 KiB initialized");
                vga.println("Filesystem: B-Tree Indexed Hierarchy");
                vga.write("Uptime: ");
                let ticks = unsafe { crate::interrupts::timer_ticks() };
                let seconds = ticks / 100;
                vga.write("seconds: ");
                vga.println(seconds_to_str(seconds));
            }
            "tasks" => {
                vga.set_color(Color::LightGreen, Color::Black);
                vga.println("--- Active Kernel Tasks ---");
                vga.set_color(Color::White, Color::Black);
                vga.println("PID   NAME         STATUS");
                vga.println("0     idle_loop    RUNNING");
                vga.println("1     shell_cli    RUNNING");
            }
            "directory" => {
                self.print_cwd_path(&mut vga);
            }
            "enter" => {
                if args.is_empty() {
                    vga.println("Usage: enter [path]");
                    return;
                }
                let path = args[0];
                let mut fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path);
                if fs.find_directory(&resolved).is_some() {
                    self.cwd = resolved;
                } else {
                    vga.set_color(Color::LightRed, Color::Black);
                    vga.println("Error: Directory not found.");
                    vga.set_color(Color::White, Color::Black);
                }
            }
            "folder" => {
                if args.is_empty() {
                    vga.println("Usage: folder [name]");
                    return;
                }
                let path_str = args[0];
                let mut fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                if resolved.is_empty() {
                    vga.println("Error: Invalid directory name");
                    return;
                }
                let (parent_segments, dir_name) = resolved.split_at(resolved.len() - 1);
                let ticks = unsafe { crate::interrupts::timer_ticks() };
                match fs.create_directory(parent_segments, &dir_name[0], ticks) {
                    Ok(_) => vga.println("Directory folder created successfully in B-Tree index."),
                    Err(e) => {
                        vga.set_color(Color::LightRed, Color::Black);
                        vga.println(e);
                        vga.set_color(Color::White, Color::Black);
                    }
                }
            }
            "list" => {
                match FS.lock().list_directory(&self.cwd) {
                    Ok(items) => {
                        if items.is_empty() {
                            vga.println("Directory is empty.");
                        } else {
                            vga.set_color(Color::LightCyan, Color::Black);
                            vga.println("Name                 Type");
                            vga.set_color(Color::White, Color::Black);
                            for (name, is_dir) in items {
                                let padding = 20 - name.len().min(19);
                                vga.write(&name);
                                for _ in 0..padding {
                                    vga.write(" ");
                                }
                                if is_dir {
                                    vga.set_color(Color::LightBlue, Color::Black);
                                    vga.println("<DIR>");
                                    vga.set_color(Color::White, Color::Black);
                                } else {
                                    vga.println("File");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        vga.set_color(Color::LightRed, Color::Black);
                        vga.println(e);
                        vga.set_color(Color::White, Color::Black);
                    }
                }
            }
            "create" => {
                if args.is_empty() {
                    vga.println("Usage: create [filename]");
                    return;
                }
                let path_str = args[0];
                let mut fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                if resolved.is_empty() {
                    vga.println("Error: Invalid filename");
                    return;
                }
                let (parent_segments, file_name) = resolved.split_at(resolved.len() - 1);
                let target_name = &file_name[0];

                let ticks = unsafe { crate::interrupts::timer_ticks() };
                match fs.create_file(parent_segments, target_name, ticks) {
                    Ok(_) => vga.println("File created successfully in B-Tree index."),
                    Err(e) => {
                        vga.set_color(Color::LightRed, Color::Black);
                        vga.println(e);
                        vga.set_color(Color::White, Color::Black);
                    }
                }
            }
            "write" => {
                if args.len() < 2 {
                    vga.println("Usage: write [filename] [text]");
                    return;
                }
                let path_str = args[0];
                let text = args[1..].join(" ");
                let mut fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                if resolved.is_empty() {
                    vga.println("Error: Invalid filename");
                    return;
                }
                let (parent_segments, file_name) = resolved.split_at(resolved.len() - 1);
                match fs.write_file(parent_segments, &file_name[0], &text) {
                    Ok(_) => vga.println("Text written to file in B-Tree index."),
                    Err(e) => {
                        vga.set_color(Color::LightRed, Color::Black);
                        vga.println(e);
                        vga.set_color(Color::White, Color::Black);
                    }
                }
            }
            "read" => {
                if args.is_empty() {
                    vga.println("Usage: read [filename]");
                    return;
                }
                let path_str = args[0];
                let fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                if resolved.is_empty() {
                    vga.println("Error: Invalid filename");
                    return;
                }
                let (parent_segments, file_name) = resolved.split_at(resolved.len() - 1);
                match fs.read_file(parent_segments, &file_name[0]) {
                    Ok(content) => {
                        vga.println("--- Content ---");
                        vga.println(&content);
                        vga.println("---------------");
                    }
                    Err(e) => {
                        vga.set_color(Color::LightRed, Color::Black);
                        vga.println(e);
                        vga.set_color(Color::White, Color::Black);
                    }
                }
            }
            "delete" => {
                if args.is_empty() {
                    vga.println("Usage: delete [filename/folder]");
                    return;
                }
                let path_str = args[0];
                let mut fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                if resolved.is_empty() {
                    vga.println("Error: Invalid target");
                    return;
                }
                let (parent_segments, target_name) = resolved.split_at(resolved.len() - 1);
                match fs.delete_node(parent_segments, &target_name[0]) {
                    Ok(_) => vga.println("Target deleted successfully from B-Tree index."),
                    Err(e) => {
                        vga.set_color(Color::LightRed, Color::Black);
                        vga.println(e);
                        vga.set_color(Color::White, Color::Black);
                    }
                }
            }
            "edit" => {
                if args.is_empty() {
                    vga.println("Usage: edit [filename]");
                    return;
                }
                let path_str = args[0];
                
                let mut fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                if resolved.is_empty() {
                    vga.println("Error: Invalid filename");
                    return;
                }
                let (parent_segments, file_name) = resolved.split_at(resolved.len() - 1);
                let target_name = &file_name[0];
                
                // Automatically create file in B-Tree index if it doesn't exist
                let dir_items = fs.list_directory(parent_segments).unwrap_or_else(|_| Vec::new());
                let file_exists = dir_items.iter().any(|(name, is_dir)| name == target_name && !is_dir);

                if !file_exists {
                    let ticks = unsafe { crate::interrupts::timer_ticks() };
                    let _ = fs.create_file(parent_segments, target_name, ticks);
                }

                let current_content = fs.read_file(parent_segments, target_name).unwrap_or_else(|_| String::new());
                self.editor_filename = String::from(path_str);
                self.editor_buffer = current_content.clone();
                self.mode = ShellMode::Editor;

                vga.clear();
                vga.set_color(Color::LightCyan, Color::Black);
                vga.write("--- Editing File: ");
                vga.write(target_name);
                vga.println(" ---");
                vga.println("Type your text below. Type ':wq' on a new line and press Enter to save and exit, or ':q' to exit without saving.");
                vga.println("------------------------------------------------------------------------");
                vga.set_color(Color::White, Color::Black);
                vga.write(&current_content);
            }
            "play" => {
                if args.is_empty() {
                    vga.println("Usage: play [atari / chess]");
                    return;
                }
                drop(vga);
                match args[0] {
                    "atari" => crate::game::start_atari(),
                    "chess" => crate::game::start_chess(),
                    _ => WRITER.lock().println("Unknown game. Choose 'atari' or 'chess'."),
                }
            }
            "draw" => {
                drop(vga);
                crate::game::start_canvas();
            }
            "echo" => {
                let text = args.join(" ");
                vga.println(&text);
            }
            _ => {
                vga.set_color(Color::LightRed, Color::Black);
                vga.write("Command not recognized: ");
                vga.println(command);
                vga.set_color(Color::White, Color::Black);
                vga.println("Type 'help' to launch the Interactive Help Viewer.");
            }
        }
    }
}

/// Formats seconds integer into a string.
fn seconds_to_str(secs: u64) -> String {
    let mut s = String::new();
    let mut temp = secs;
    if temp == 0 {
        s.push('0');
        return s;
    }
    let mut digits = Vec::new();
    while temp > 0 {
        digits.push((b'0' + (temp % 10) as u8) as char);
        temp /= 10;
    }
    for &c in digits.iter().rev() {
        s.push(c);
    }
    s
}
