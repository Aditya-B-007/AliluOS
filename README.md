<div align="center">

# 🐿️ AliluOS
## ಅಳಿಲು Operating System

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
