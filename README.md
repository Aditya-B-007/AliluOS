<div align="center">

# 🐿️ AliluOS
## ಅಳಿಲು Operating System
<img width="3000" height="2000" alt="image" src="https://github.com/user-attachments/assets/71a679d6-0ad7-478e-9ee6-825ac2408826" />

*A lightweight, command-line operating system written in Rust.*

---

![Rust](https://img.shields.io/badge/Language-Rust-orange?style=for-the-badge&logo=rust)
![Platform](https://img.shields.io/badge/Platform-x86__64-blue?style=for-the-badge)
![Interface](https://img.shields.io/badge/Interface-CLI-green?style=for-the-badge)
![Kernel](https://img.shields.io/badge/Kernel-Monolithic-red?style=for-the-badge)
![License](https://img.shields.io/badge/License-MIT-purple?style=for-the-badge)
![Status](https://img.shields.io/badge/Status-In_Development-yellow?style=for-the-badge)

</div>

---

# About

**AliluOS** is a minimal command-line operating system written entirely in **Rust**.

Inspired by the agility and efficiency of the squirrel (*ಅಳಿಲು*), AliluOS focuses on simplicity, reliability, and learning operating system internals from the ground up.

The project intentionally starts small and grows incrementally, implementing each subsystem one at a time—from the bootloader to memory management, process scheduling, filesystems, and beyond.

> **Small. Fast. Built from Scratch.**

**ಅಳಿಲುOS** ಸಂಪೂರ್ಣವಾಗಿ **Rust** ಭಾಷೆಯಲ್ಲಿ ನಿರ್ಮಿಸಲಾಗುತ್ತಿರುವ ಕನಿಷ್ಠ (Minimal) ಕಮಾಂಡ್-ಲೈನ್ ಆಪರೇಟಿಂಗ್ ಸಿಸ್ಟಮ್ ಆಗಿದೆ.

**ಅಳಿಲು** ಹೇಗೆ ಚುರುಕಾಗಿ, ಕಡಿಮೆ ಸಂಪನ್ಮೂಲಗಳನ್ನು ಬಳಸಿಕೊಂಡು ಕಾರ್ಯನಿರ್ವಹಿಸುತ್ತದೆಯೋ, ಅದೇ ತತ್ವದ ಮೇಲೆ ಈ ಕಾರ್ಯಾಚರಣಾ ವ್ಯವಸ್ಥೆಯನ್ನು ವಿನ್ಯಾಸಗೊಳಿಸಲಾಗಿದೆ.

ಈ ಯೋಜನೆಯ ಉದ್ದೇಶ ಕಾರ್ಯಾಚರಣಾ ವ್ಯವಸ್ಥೆಯ ಪ್ರತಿಯೊಂದು ಭಾಗವನ್ನು ಶೂನ್ಯದಿಂದ ಹಂತ ಹಂತವಾಗಿ ನಿರ್ಮಿಸುವುದು—ಬೂಟ್‌ಲೋಡರ್‌ನಿಂದ ಪ್ರಾರಂಭಿಸಿ ಮೆಮೊರಿ ನಿರ್ವಹಣೆ, ಪ್ರಕ್ರಿಯೆ ನಿರ್ವಹಣೆ, ಕಡತ ವ್ಯವಸ್ಥೆ ಹಾಗೂ ಇತರೆ ಘಟಕಗಳವರೆಗೆ.

> **ಚಿಕ್ಕದು. ವೇಗವಾದದು. ಶೂನ್ಯದಿಂದ ನಿರ್ಮಿತವಾದುದು.**

# Philosophy

- Minimal by Design
- CLI First
- Memory Safe with Rust
- Educational
- Built Incrementally
- No Unnecessary Features

---

# Project Goals

- Build a bootable operating system from scratch
- Understand low-level computer architecture
- Learn operating system internals
- Maintain clean and readable Rust code
- Keep the kernel lightweight


# Building

## Folder structure
```
rust-cli-os/
│
├── .cargo/
│   └── config.toml               # Configures the build target and compiler flags
│
├── src/                          # The Kernel Source Code
│   ├── main.rs                   # Kernel entry point, panic handler, and main shell loop
│   ├── vga.rs                    # Text-mode video driver (handles printing and println!)
│   ├── keyboard.rs               # Keyboard interrupt handling and key translation
│   ├── interrupts.rs             # IDT, GDT, and Hardware Interrupt configurations
│   ├── allocator.rs              # Heap allocation initialization (for dynamic strings/vecs)
│   ├── fs.rs                     # The in-memory or basic file system (create, delete, edit)
│   └── game.rs                   # The single built-in text game logic
│
├── x86_64-bare_metal.json        # Custom JSON target specification file for bare-metal x86_64
├── Cargo.toml                    # Project dependencies (x86_64, bootloader, pc-keyboard, etc.)
└── build.rs                      # Optional build script to link files or automate image creation

```
## Requirements

- Rust Nightly
- cargo
- rust-src
- llvm-tools
- QEMU
- NASM

Install targets

```bash
rustup default nightly
rustup component add rust-src
rustup component add llvm-tools-preview
```

Build

```bash
cargo build
```

Run

```bash
cargo run
```

---

# Why Rust?

Rust provides

- Memory safety
- Zero-cost abstractions
- Fearless concurrency
- Excellent tooling
- Modern systems programming

making it an excellent language for operating system development.

---

# Project Status

🚧 **This operating system is under active development.**

Features will be added gradually as each subsystem is implemented and tested.

---

# Packaging a Bootable Image (.bin / .iso)

This section describes how to compile the AliluOS kernel, bundle it with the bootloader, and package it into a virtual-machine-compatible `.bin` or `.iso` file.

## 1. Prerequisites
You must have the Rust nightly toolchain and llvm tools installed:
```bash
rustup default nightly
rustup component add rust-src
rustup component add llvm-tools-preview
```

Additionally, you need the `bootimage` cargo extension to automate bootloader injection and disk mapping:
```bash
cargo install bootimage
```

## 2. Generating a Bootable Raw Disk Image (.bin)
To bundle the kernel ELF binary and compile the bootloader into a unified sector-mapped raw disk image, run:
```bash
cargo bootimage
```
* **Output Location**: `target/x86_64-bare_metal/debug/bootimage-alilu_os.bin`
* **VM Loading**: This `.bin` file is a hybrid BIOS bootable image. You can load this file directly in **QEMU** or mount it as a **Virtual Hard Disk (IDE/SATA controller)** inside **VirtualBox** or **VMware**.

## 3. Converting to a VirtualBox VDI Hard Disk (.vdi)
To run the OS inside Oracle VM VirtualBox, it is recommended to convert the raw `.bin` image into a Virtual Disk Image (`.vdi`):

```bash
VBoxManage convertfromraw target/x86_64-bare_metal/debug/bootimage-alilu_os.bin target/alilu_os.vdi --format VDI
```
* **Output Location**: `target/alilu_os.vdi`
* **VirtualBox Setup**: 
  1. Create a new virtual machine of type **Other** -> **Other/Unknown (64-bit)**.
  2. Select **"Use an existing virtual hard disk file"** in the storage/hard disk wizard and browse to choose `alilu_os.vdi`.
  3. Start the virtual machine.

## 4. Converting to an ISO File (.iso)
If your virtual machine manager requires a standard ISO format (optical CD-ROM mount), package the bootable raw binary:

### On macOS / Linux
Install `xorriso` (e.g. `brew install xorriso` or `sudo apt install xorriso`) and run:
```bash
# Create directory structure
mkdir -p target/iso/boot/grub
# Copy bootable bin
cp target/x86_64-bare_metal/debug/bootimage-alilu_os.bin target/iso/boot/kernel.bin
# Generate the ISO image
xorriso -as mkisofs -R -b boot/kernel.bin -no-emul-boot -boot-load-size 4 -boot-info-table -o target/alilu_os.iso target/iso
```
* **Output Location**: `target/alilu_os.iso`
* **VM Loading**: Mount this `.iso` file directly into your virtual machine's virtual optical (CD/DVD) drive.

---

# Contributing

Contributions, suggestions, and discussions are welcome.

If you find bugs or have ideas for improvements, feel free to open an issue or submit a pull request.

---

# License

This project is licensed under the **MIT License**.

---

<div align="center">

## 🐿️ AliluOS

### ಅಳಿಲು Operating System

**"Small. Fast. Built from Scratch."**

**"ಚಿಕ್ಕದು. ವೇಗವಾದದು. ಶೂನ್ಯದಿಂದ ನಿರ್ಮಿತವಾದುದು."**

</div>
