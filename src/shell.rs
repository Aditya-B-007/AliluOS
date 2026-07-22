use alloc::string::String;
use alloc::vec::Vec;
use crate::vga::{Color, VGA};
use crate::fs::{FS, Node};

/// State representing if the shell is in command entry mode or file editing mode.
#[derive(PartialEq, Eq)]
enum ShellMode {
    Command,
    Editor,
}

pub struct Shell {
    buffer: String,             // Buffer storing current command line input
    mode: ShellMode,            // Active mode (Command vs. Editor)
    editor_filename: String,    // Name of file currently being edited
    editor_buffer: String,      // Buffer storing accumulating text in editor mode
    cwd: Vec<String>,           // Current Working Directory path segments
}

impl Shell {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            mode: ShellMode::Command,
            editor_filename: String::new(),
            editor_buffer: String::new(),
            cwd: Vec::new(), // Empty segments list represents root "/"
        }
    }

    /// Handles a character input, routing depending on the current active shell mode.
    pub fn handle_char(&mut self, c: char) {
        let mut vga = VGA::new();
        match self.mode {
            ShellMode::Command => {
                self.buffer.push(c);
                vga.put_char(c);
            }
            ShellMode::Editor => {
                self.editor_buffer.push(c);
                vga.put_char(c);
            }
        }
    }

    /// Handles a backspace input, adjusting internal buffers and screen visuals.
    pub fn handle_backspace(&mut self) {
        let mut vga = VGA::new();
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
        }
    }

    /// Handles space key inputs.
    pub fn handle_space(&mut self) {
        let mut vga = VGA::new();
        match self.mode {
            ShellMode::Command => {
                self.buffer.push(' ');
                vga.put_char(' ');
            }
            ShellMode::Editor => {
                self.editor_buffer.push(' ');
                vga.put_char(' ');
            }
        }
    }

    /// Handles when the Enter key is pressed.
    pub fn handle_enter(&mut self) {
        let mut vga = VGA::new();
        vga.put_char('\n');

        match self.mode {
            ShellMode::Command => {
                let cmd_str = self.buffer.clone();
                self.buffer.clear();
                self.execute_command(&cmd_str);
                if self.mode == ShellMode::Command {
                    vga.write("> ");
                }
            }
            ShellMode::Editor => {
                let lines: Vec<&str> = self.editor_buffer.lines().collect();
                if let Some(&last_line) = lines.last() {
                    let trimmed = last_line.trim();
                    if trimmed == ":wq" {
                        // Remove the command line from the buffer
                        let command_len = last_line.len();
                        for _ in 0..command_len {
                            self.editor_buffer.pop();
                        }
                        
                        // Parse target path and name from filename
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
        }
    }

    /// Helper to print current working directory absolute path.
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

    /// Parses and executes plain English shell commands.
    fn execute_command(&mut self, cmd_line: &str) {
        let trimmed = cmd_line.trim();
        if trimmed.is_empty() {
            return;
        }

        let mut parts = trimmed.split_whitespace();
        let command = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();

        let mut vga = VGA::new();

        match command {
            "help" => {
                vga.set_color(Color::LightCyan, Color::Black);
                vga.println("Available Plain-English Commands:");
                vga.set_color(Color::White, Color::Black);
                vga.println("  help                       - Show this guide");
                vga.println("  clear                      - Clear display screen");
                vga.println("  system                     - Show system specifications & stats");
                vga.println("  tasks                      - List current running CPU tasks");
                vga.println("  list                       - List files/folders in current directory");
                vga.println("  directory                  - Show current working directory path");
                vga.println("  enter [path]               - Enter a subfolder or path (CD)");
                vga.println("  folder [name]              - Create a new directory folder");
                vga.println("  create [file]              - Create a text/source code file");
                vga.println("  write [file] [text]        - Write or overwrite text in a file");
                vga.println("  read [file]                - Print contents of a file");
                vga.println("  delete [file/folder]       - Delete a file or directory");
                vga.println("  edit [file]                - Enter interactive text editor");
                vga.println("  play                       - Launch built-in game");
                vga.println("  echo [text]                - Print text back to screen");
            }
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
                let name = args[0];
                let ticks = unsafe { crate::interrupts::timer_ticks() };
                match FS.lock().create_directory(&self.cwd, name, ticks) {
                    Ok(_) => vga.println("Directory created successfully."),
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
                let target_name = file_name[0].as_str();

                if !is_text_or_code_file(target_name) {
                    vga.set_color(Color::LightRed, Color::Black);
                    vga.println("Error: Only text (.txt) and source code (.rs, .py, .c, etc.) files can be created.");
                    vga.set_color(Color::White, Color::Black);
                    return;
                }

                let ticks = unsafe { crate::interrupts::timer_ticks() };
                match fs.create_file(parent_segments, target_name, ticks) {
                    Ok(_) => vga.println("File created successfully."),
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
                    Ok(_) => vga.println("Text written to file."),
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
                    Ok(_) => vga.println("Target deleted successfully."),
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
                let target_name = file_name[0].as_str();

                if !is_text_or_code_file(target_name) {
                    vga.set_color(Color::LightRed, Color::Black);
                    vga.println("Error: Only text (.txt) and source code (.rs, .py, .c, etc.) files can be edited.");
                    vga.set_color(Color::White, Color::Black);
                    return;
                }
                
                // Automatically create file if it doesn't exist
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
                match args[0] {
                    "atari" => crate::game::start_atari(),
                    "chess" => crate::game::start_chess(),
                    _ => vga.println("Unknown game. Choose 'atari' or 'chess'."),
                }
            }
            "draw" => {
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
                vga.println("Type 'help' to see all commands.");
            }
        }
    }
}

/// Utility check for text files or programming source code extensions.
fn is_text_or_code_file(filename: &str) -> bool {
    let extensions = &[
        ".txt", ".rs", ".py", ".c", ".h", ".cpp", ".hpp", 
        ".js", ".ts", ".html", ".css", ".go", ".java", ".sh"
    ];
    extensions.iter().any(|ext| filename.ends_with(ext))
}

/// Formats a seconds integer to string without standard formatting macros.
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
