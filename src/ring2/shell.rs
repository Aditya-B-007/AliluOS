//! # AliluOS Interactive Shell Engine (`ring2/shell.rs`)
//!
//! - **WHAT**: Command processor, line buffer manager, interactive text editor, and tasks monitor.
//! - **WHY**: Provides human-readable plain English command input, file operations, system stats, and privilege ring thread stats.
//! - **WHEN**: Executed by `shell_cli_thread` (TID 1, Ring 2).

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use crate::vga::{Color, WRITER, VGA};
use crate::fs::{FS, Node};

#[derive(PartialEq, Eq)]
enum ShellMode {
    Command,
    Editor,
    HelpViewer,
}

pub struct Shell {
    buffer: String,
    mode: ShellMode,
    editor_filename: String,
    editor_buffer: String,
    save_pending: bool,
    cwd: Vec<String>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            mode: ShellMode::Command,
            editor_filename: String::new(),
            editor_buffer: String::new(),
            save_pending: false,
            cwd: Vec::new(),
        }
    }

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
            ShellMode::Editor => {
                self.mode = ShellMode::Command;
                self.save_pending = false;
                self.editor_buffer.clear();
                self.editor_filename.clear();
                let mut vga = WRITER.lock();
                vga.set_color(Color::Yellow, Color::Black);
                vga.println("\nExited editor without saving.");
                vga.set_color(Color::White, Color::Black);
                vga.write("> ");
            }
        }
    }

    pub fn handle_ctrl_char(&mut self, c: char) {
        let lower = c.to_ascii_lowercase();
        if self.mode == ShellMode::Editor {
            let mut vga = WRITER.lock();
            if lower == 's' {
                self.save_pending = true;
                vga.set_color(Color::Yellow, Color::Black);
                vga.println("\n[Ctrl+S detected! Press Ctrl+K to save and exit]");
                vga.set_color(Color::White, Color::Black);
            } else if lower == 'k' {
                if self.save_pending {
                    let target_path = self.editor_filename.clone();
                    let mut fs = FS.lock();
                    let resolved = fs.resolve_path(&self.cwd, &target_path);
                    let (parent_segments, target_name) = fs.split_parent_and_name(&resolved);
                    if target_name.is_empty() {
                        vga.println("\nError: Invalid filename");
                    } else {
                        let content = self.editor_buffer.clone();
                        match fs.write_file(parent_segments, target_name, &content) {
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
                    self.save_pending = false;
                    self.editor_buffer.clear();
                    self.editor_filename.clear();
                    vga.write("> ");
                } else {
                    vga.set_color(Color::Yellow, Color::Black);
                    vga.println("\n[Press Ctrl+S first, then Ctrl+K to save and exit]");
                    vga.set_color(Color::White, Color::Black);
                }
            }
        }
    }

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
            ShellMode::HelpViewer => {}
        }
    }

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
                        let (parent_segments, target_name) = fs.split_parent_and_name(&resolved);
                        if target_name.is_empty() {
                            vga.println("Error: Invalid filename");
                        } else {
                            let content = self.editor_buffer.clone();
                            match fs.write_file(parent_segments, target_name, &content) {
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

    fn execute_command(&mut self, cmd_line: &str) {
        let trimmed = cmd_line.trim();
        if trimmed.is_empty() {
            return;
        }

        let mut parts = trimmed.split_whitespace();
        let command = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();

        if command == "help" || args.contains(&"--help") || args.contains(&"-h") {
            let mut vga = WRITER.lock();
            vga.set_color(Color::LightCyan, Color::Black);
            vga.println("========================================================================");
            vga.println("                    AliluOS COMMAND MANUAL                              ");
            vga.println("========================================================================");
            vga.set_color(Color::White, Color::Black);
            vga.println("  help                       - Display this command manual");
            vga.println("  clear                      - Clear display screen");
            vga.println("  system                     - Display system specs, memory & timer stats");
            vga.println("  tasks                      - List single-process resources & privilege ring threads");
            vga.println("  list                       - List files/folders (B-Tree sorted)");
            vga.println("  directory                  - Print current working directory path");
            vga.println("  enter [path]               - Enter directory folder");
            vga.println("  back / leave / up / cd ..  - Return back to parent directory");
            vga.println("  folder [name]              - Create a new directory folder in B-Tree");
            vga.println("  create [file]              - Create a new file in B-Tree index");
            vga.println("  write [file] [text]        - Write text content to a file");
            vga.println("  read [file]                - View file contents from B-Tree index");
            vga.println("  delete [file/folder]       - Delete a file or folder from B-Tree index");
            vga.println("  edit [file]                - Open interactive text editor");
            vga.println("  play [atari / chess]       - Launch built-in text game");
            vga.println("  browse [query]             - Open DuckDuckGo search browser");
            vga.println("  echo [text]                - Print text back to screen");
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
                vga.println("Architecture: x86_64 Privilege Ring Bifurcation (Ring 0, Ring 1, Ring 2)");
                vga.println("Process Model: Single-Process Resource Manager (PID 1)");
                vga.println("Heap Status: 5 MiB (5120 KiB) initialized");
                vga.println("Storage Engine: Persistent ATA Disk Blocks (1024 bytes/block)");
                vga.println("Filesystem Index: On-Disk B+ Tree Data Structure");
                vga.println("Memory Manager: RAM Virtual Address Table (VAT) Demand Paging");
                let ticks = crate::interrupts::timer_ticks();
                let seconds = ticks / 100;
                vga.write("Uptime: ");
                vga.write(&seconds_to_str(seconds));
                vga.println(" seconds");
            }
            "tasks" => {
                let mut pm = crate::process::PROCESS_MANAGER.lock();
                if let crate::process::ProcessResourceResponse::Stats(summary) = pm.handle(crate::process::ProcessResourceRequest::GetSystemResourceStats) {
                    vga.set_color(Color::LightGreen, Color::Black);
                    vga.println("=== SINGLE-PROCESS RESOURCE MANAGER STATS ===");
                    vga.set_color(Color::White, Color::Black);
                    vga.write("PID: ");
                    vga.println(&seconds_to_str(summary.process_id as u64));
                    vga.write("Primary Memory Quota: ");
                    vga.write(&seconds_to_str((summary.ram_allocated / 1024) as u64));
                    vga.write(" / ");
                    vga.write(&seconds_to_str((summary.ram_quota / 1024) as u64));
                    vga.println(" KiB");

                    vga.write("Secondary Disk Quota: ");
                    vga.write(&seconds_to_str((summary.disk_allocated / 1024) as u64));
                    vga.write(" / ");
                    vga.write(&seconds_to_str((summary.disk_quota / 1024) as u64));
                    vga.println(" KiB");

                    vga.write("Network Bandwidth Rate: ");
                    vga.write(&seconds_to_str(summary.net_bandwidth_limit / 1000));
                    vga.println(" KB/s");

                    vga.set_color(Color::LightCyan, Color::Black);
                    vga.println("\n=== KERNEL EXECUTION THREADS (TID 0..10) ===");
                    vga.println("TID  NAME                    RING           STATE     STACK");
                    vga.set_color(Color::White, Color::Black);

                    for t in summary.thread_stats {
                        vga.write(&seconds_to_str(t.tid as u64));
                        vga.write("    ");
                        let pad = 24 - t.name.len().min(23);
                        vga.write(&t.name);
                        for _ in 0..pad { vga.write(" "); }
                        
                        match t.ring {
                            crate::thread::PrivilegeLevel::Ring0 => {
                                vga.set_color(Color::LightRed, Color::Black);
                                vga.write("Ring0/Kernel  ");
                            }
                            crate::thread::PrivilegeLevel::Ring1 => {
                                vga.set_color(Color::Yellow, Color::Black);
                                vga.write("Ring1/Driver  ");
                            }
                            crate::thread::PrivilegeLevel::Ring2 => {
                                vga.set_color(Color::LightCyan, Color::Black);
                                vga.write("Ring2/UserApp ");
                            }
                        }

                        match t.state {
                            crate::thread::ThreadState::Running => {
                                vga.set_color(Color::LightGreen, Color::Black);
                                vga.write("RUNNING   ");
                            }
                            crate::thread::ThreadState::Ready => {
                                vga.set_color(Color::Yellow, Color::Black);
                                vga.write("READY     ");
                            }
                            crate::thread::ThreadState::Blocked => {
                                vga.set_color(Color::LightRed, Color::Black);
                                vga.write("BLOCKED   ");
                            }
                            crate::thread::ThreadState::Terminated => {
                                vga.set_color(Color::DarkGray, Color::Black);
                                vga.write("TERMINATED");
                            }
                        }
                        vga.set_color(Color::White, Color::Black);
                        vga.write(&seconds_to_str((t.stack_size / 1024) as u64));
                        vga.println(" KiB");
                    }
                }
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
                    vga.write("Entered directory: ");
                    self.print_cwd_path(&mut vga);
                } else {
                    vga.set_color(Color::LightRed, Color::Black);
                    vga.println("Error: Directory not found.");
                    vga.set_color(Color::White, Color::Black);
                }
            }
            "back" | "leave" | "up" => {
                if !self.cwd.is_empty() {
                    self.cwd.pop();
                    vga.write("Returned to: ");
                    self.print_cwd_path(&mut vga);
                } else {
                    vga.println("Already at root directory (/).");
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
                let (parent_segments, dir_name) = fs.split_parent_and_name(&resolved);
                if dir_name.is_empty() {
                    vga.println("Error: Invalid directory name");
                    return;
                }
                let ticks = crate::interrupts::timer_ticks();
                match fs.create_directory(parent_segments, dir_name, ticks) {
                    Ok(_) => vga.println("Directory folder created successfully in B+ Tree index."),
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
                                for _ in 0..padding { vga.write(" "); }
                                if is_dir {
                                    vga.set_color(Color::LightBlue, Color::Black);
                                    vga.println("<DIR>");
                                    vga.set_color(Color::White, Color::Black);
                                } else {
                                    vga.println("<FILE>");
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
                let (parent_segments, file_name) = fs.split_parent_and_name(&resolved);
                if file_name.is_empty() {
                    vga.println("Error: Invalid filename");
                    return;
                }
                let ticks = crate::interrupts::timer_ticks();
                match fs.create_file(parent_segments, file_name, ticks) {
                    Ok(_) => vga.println("File created successfully in B+ Tree index."),
                    Err(e) => {
                        vga.set_color(Color::LightRed, Color::Black);
                        vga.println(e);
                        vga.set_color(Color::White, Color::Black);
                    }
                }
            }
            "write" => {
                if args.len() < 2 {
                    vga.println("Usage: write [filename] [text...]");
                    return;
                }
                let path_str = args[0];
                let content = args[1..].join(" ");
                let mut fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                let (parent_segments, file_name) = fs.split_parent_and_name(&resolved);
                if file_name.is_empty() {
                    vga.println("Error: Invalid filename");
                    return;
                }
                match fs.write_file(parent_segments, file_name, &content) {
                    Ok(_) => vga.println("Written to file successfully."),
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
                let (parent_segments, file_name) = fs.split_parent_and_name(&resolved);
                if file_name.is_empty() {
                    vga.println("Error: Invalid filename");
                    return;
                }
                match fs.read_file(parent_segments, file_name) {
                    Ok(content) => {
                        vga.set_color(Color::LightGreen, Color::Black);
                        vga.println("--- File Content ---");
                        vga.set_color(Color::White, Color::Black);
                        vga.println(&content);
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
                    vga.println("Usage: delete [name]");
                    return;
                }
                let path_str = args[0];
                let mut fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                let (parent_segments, file_name) = fs.split_parent_and_name(&resolved);
                if file_name.is_empty() {
                    vga.println("Error: Invalid name");
                    return;
                }
                match fs.delete_node(parent_segments, file_name) {
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
                self.editor_filename = String::from(path_str);
                let fs = FS.lock();
                let resolved = fs.resolve_path(&self.cwd, path_str);
                let (parent_segments, file_name) = fs.split_parent_and_name(&resolved);
                
                if let Ok(existing) = fs.read_file(parent_segments, file_name) {
                    self.editor_buffer = existing;
                } else {
                    self.editor_buffer = String::new();
                }

                self.mode = ShellMode::Editor;
                vga.clear();
                vga.set_color(Color::LightCyan, Color::Black);
                vga.write("=== AliluOS Interactive Editor: ");
                vga.write(path_str);
                vga.println(" ===");
                vga.set_color(Color::White, Color::Black);
                vga.println("Type content below. Commands: Type ':wq' on new line to save & exit, ':q' to exit.\n");
                vga.write(&self.editor_buffer.clone());
            }
            "play" => {
                if args.is_empty() {
                    vga.println("Usage: play [atari / chess]");
                    return;
                }
                if args[0] == "atari" {
                    crate::game::GAME.lock().start_atari();
                } else if args[0] == "chess" {
                    crate::game::GAME.lock().start_chess();
                } else {
                    vga.println("Unknown game. Choose: atari or chess");
                }
            }
            "echo" => {
                vga.println(&args.join(" "));
            }
            _ => {
                vga.set_color(Color::LightRed, Color::Black);
                vga.write("Unknown command: '");
                vga.write(command);
                vga.println("'. Type 'help' for manual.");
                vga.set_color(Color::White, Color::Black);
            }
        }
    }
}

fn seconds_to_str(mut sec: u64) -> String {
    if sec == 0 { return String::from("0"); }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while sec > 0 {
        buf[i] = b'0' + (sec % 10) as u8;
        sec /= 10;
        i += 1;
    }
    let mut s = String::new();
    for j in (0..i).rev() {
        s.push(buf[j] as char);
    }
    s
}
