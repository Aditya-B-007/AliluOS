//! # AliluOS GDT, IDT, TSS & Hardware Interrupt Subsystem (`ring0/interrupts.rs`)
//!
//! - **WHAT**: Instantiates GDT descriptors for Ring 0, Ring 1, and Ring 2 privileges, configures TSS interrupt stacks, dual 8259 PICs, PIT 100 Hz timer, and IDT gates.
//! - **WHY**: Enforces privilege ring segregation across Ring 0, Ring 1, and Ring 2, and handles hardware IRQ interrupts and system calls (`0x80`).

#![allow(dead_code)]

use core::arch::asm;
use crate::vga::{Color, VGA, WRITER};
use crate::config::interrupts::*;
use crate::config::gdt::*;

pub struct ScancodeBuffer {
    buffer: [u8; 256],
    head: usize,
    tail: usize,
}

impl ScancodeBuffer {
    pub const fn new() -> Self {
        Self {
            buffer: [0; 256],
            head: 0,
            tail: 0,
        }
    }

    pub fn push(&mut self, scancode: u8) {
        let next_head = (self.head + 1) % 256;
        if next_head != self.tail {
            self.buffer[self.head] = scancode;
            self.head = next_head;
        }
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            None
        } else {
            let scancode = self.buffer[self.tail];
            self.tail = (self.tail + 1) % 256;
            Some(scancode)
        }
    }
}

pub static SCANCODES: crate::vga::Locked<ScancodeBuffer> = crate::vga::Locked::new(ScancodeBuffer::new());

pub fn pop_scancode() -> Option<u8> {
    SCANCODES.lock().pop()
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access_byte: u8,
    flags_limit_high: u8,
    base_high: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct GdtSystemEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access_byte: u8,
    flags_limit_high: u8,
    base_high_middle: u8,
    base_high: u32,
    reserved: u32,
}

#[repr(C, align(16))]
struct Gdt {
    null: GdtEntry,
    code: GdtEntry,        // Ring 0 Code (0x08)
    data: GdtEntry,        // Ring 0 Data (0x10)
    driver_code: GdtEntry, // Ring 1 Code (0x18 | 3 = 0x1B)
    driver_data: GdtEntry, // Ring 1 Data (0x20 | 3 = 0x23)
    user_code: GdtEntry,   // Ring 2 Code (0x28 | 3 = 0x2B)
    user_data: GdtEntry,   // Ring 2 Data (0x30 | 3 = 0x33)
    tss: GdtSystemEntry,   // TSS (0x38 | 3 = 0x3B)
}

#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

#[repr(C, packed)]
struct TaskStateSegment {
    reserved_1: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved_2: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved_3: u64,
    reserved_4: u16,
    iomap_base: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    pointer_low: u16,
    gdt_selector: u16,
    options: u16,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl IdtEntry {
    pub const fn missing() -> Self {
        Self {
            pointer_low: 0,
            gdt_selector: 0,
            options: 0,
            pointer_middle: 0,
            pointer_high: 0,
            reserved: 0,
        }
    }

    fn set_handler(&mut self, handler: u64) {
        self.pointer_low = (handler & 0xFFFF) as u16;
        self.gdt_selector = KERNEL_CODE_SEL; // Code segment selector at 0x08
        self.options = 0x8E00; // Present, Ring 0 Interrupt Gate
        self.pointer_middle = ((handler >> 16) & 0xFFFF) as u16;
        self.pointer_high = ((handler >> 32) & 0xFFFFFFFF) as u32;
        self.reserved = 0;
    }

    fn set_user_syscall_handler(&mut self, handler: u64) {
        self.pointer_low = (handler & 0xFFFF) as u16;
        self.gdt_selector = KERNEL_CODE_SEL;
        self.options = 0xEE00; // Present, Ring 2 (DPL=2) User Callable Interrupt Gate
        self.pointer_middle = ((handler >> 16) & 0xFFFF) as u16;
        self.pointer_high = ((handler >> 32) & 0xFFFFFFFF) as u32;
        self.reserved = 0;
    }
}

#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

static mut GDT: Gdt = Gdt {
    null: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0, flags_limit_high: 0, base_high: 0 },
    code: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0x9A, flags_limit_high: 0x20, base_high: 0 },
    data: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0x92, flags_limit_high: 0, base_high: 0 },
    driver_code: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0xBA, flags_limit_high: 0x20, base_high: 0 },
    driver_data: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0xB2, flags_limit_high: 0, base_high: 0 },
    user_code: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0xDA, flags_limit_high: 0x20, base_high: 0 },
    user_data: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0xD2, flags_limit_high: 0, base_high: 0 },
    tss: GdtSystemEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0, flags_limit_high: 0, base_high_middle: 0, base_high: 0, reserved: 0 },
};

static mut TSS_STACK: [u8; 16384] = [0; 16384];

static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved_1: 0, rsp0: 0, rsp1: 0, rsp2: 0, reserved_2: 0,
    ist1: 0, ist2: 0, ist3: 0, ist4: 0, ist5: 0, ist6: 0, ist7: 0,
    reserved_3: 0, reserved_4: 0, iomap_base: 104,
};

static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];
static mut TIMER_TICKS: u64 = 0;

pub fn timer_ticks() -> u64 {
    unsafe { TIMER_TICKS }
}

pub fn init() {
    unsafe {
        setup_gdt();
        setup_idt();
        setup_pic();
        setup_pit(100);
    }
}

unsafe fn setup_gdt() {
    TSS.rsp0 = (&TSS_STACK as *const _ as u64) + 16384;

    let tss_address = &TSS as *const TaskStateSegment as u64;
    let tss_size = core::mem::size_of::<TaskStateSegment>() as u32;

    GDT.tss.limit_low = (tss_size - 1) as u16;
    GDT.tss.base_low = (tss_address & 0xFFFF) as u16;
    GDT.tss.base_middle = ((tss_address >> 16) & 0xFF) as u8;
    GDT.tss.access_byte = 0x89;
    GDT.tss.flags_limit_high = (((tss_size - 1) >> 16) & 0x0F) as u8;
    GDT.tss.base_high_middle = ((tss_address >> 24) & 0xFF) as u8;
    GDT.tss.base_high = (tss_address >> 32) as u32;

    let gdt_ptr = GdtPointer {
        limit: (core::mem::size_of::<Gdt>() - 1) as u16,
        base: &GDT as *const Gdt as u64,
    };

    asm!(
        "lgdt [{}]",
        "mov ax, 0x10",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",
        "push 0x08",
        "lea rax, [2f]",
        "push rax",
        "rex64 retf",
        "2:",
        "mov ax, 0x3B",
        "ltr ax",
        in(reg) &gdt_ptr,
        options(readonly, nostack, preserves_flags)
    );
}

unsafe fn setup_idt() {
    IDT[0].set_handler(divide_by_zero_handler as u64);
    IDT[8].set_handler(double_fault_handler as u64);
    IDT[14].set_handler(page_fault_handler as u64);

    IDT[32].set_handler(timer_interrupt_handler as u64);    // IRQ0: Timer
    IDT[33].set_handler(keyboard_interrupt_handler as u64); // IRQ1: Keyboard
    IDT[0x80].set_user_syscall_handler(syscall_interrupt_handler as u64); // System Call Gate

    let idt_ptr = IdtPointer {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: &IDT as *const [IdtEntry; 256] as u64,
    };

    asm!(
        "lidt [{}]",
        in(reg) &idt_ptr,
        options(readonly, nostack, preserves_flags)
    );
}

unsafe fn setup_pic() {
    outb(PIC1_COMMAND, 0x11);
    outb(PIC2_COMMAND, 0x11);

    outb(PIC1_DATA, 0x20); // Master IRQ0-7 -> vectors 32-39
    outb(PIC2_DATA, 0x28); // Slave IRQ8-15 -> vectors 40-47

    outb(PIC1_DATA, 0x04);
    outb(PIC2_DATA, 0x02);

    outb(PIC1_DATA, 0x01);
    outb(PIC2_DATA, 0x01);

    outb(PIC1_DATA, 0xFC); // Unmask IRQ0 (timer) and IRQ1 (keyboard)
    outb(PIC2_DATA, 0xFF);
}

unsafe fn setup_pit(frequency: u32) {
    let divisor = 1193182 / frequency;
    outb(PIT_COMMAND, 0x36);
    outb(PIT_DATA0, (divisor & 0xFF) as u8);
    outb(PIT_DATA0, ((divisor >> 8) & 0xFF) as u8);
}

unsafe fn outb(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nostack, nomem)
    );
}

macro_rules! exception_handler {
    ($name:ident, $inner_name:ident, $msg:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
                "push r8", "push r9", "push r10", "push r11",
                "call {rust_handler}",
                "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
                "pop rsi", "pop rdx", "pop rcx", "pop rax",
                "iretq",
                rust_handler = sym $inner_name,
            );
        }

        extern "C" fn $inner_name() {
            let mut vga = WRITER.lock();
            vga.set_color(Color::LightRed, Color::Black);
            vga.println("\n--- CPU EXCEPTION ---");
            vga.println($msg);
            vga.println("Halting system.");
            loop {}
        }
    };
}

exception_handler!(divide_by_zero_handler, divide_by_zero_inner, "Divide by Zero Exception (0x00)");
exception_handler!(double_fault_handler, double_fault_inner, "Double Fault Exception (0x08)");

#[unsafe(naked)]
unsafe extern "C" fn page_fault_handler() {
    core::arch::naked_asm!(
        "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
        "push r8", "push r9", "push r10", "push r11",
        "mov rdi, cr2",
        "call {rust_handler}",
        "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
        "pop rsi", "pop rdx", "pop rcx", "pop rax",
        "add rsp, 8",
        "iretq",
        rust_handler = sym page_fault_inner,
    );
}

extern "C" fn page_fault_inner(faulting_address: u64) {
    let mut vga = WRITER.lock();
    vga.set_color(Color::LightRed, Color::Black);
    vga.println("\n--- CPU EXCEPTION ---");
    vga.write("Page Fault at address: 0x");
    let temp = faulting_address;
    for i in (0..16).rev() {
        let digit = ((temp >> (i * 4)) & 0xF) as u8;
        let c = if digit < 10 { (b'0' + digit) as char } else { (b'A' + (digit - 10)) as char };
        vga.put_char(c);
    }
    vga.println("\nHalting system.");
    loop {}
}

#[unsafe(naked)]
unsafe extern "C" fn timer_interrupt_handler() {
    core::arch::naked_asm!(
        "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
        "push r8", "push r9", "push r10", "push r11",
        "call {rust_handler}",
        "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
        "pop rsi", "pop rdx", "pop rcx", "pop rax",
        "iretq",
        rust_handler = sym timer_interrupt_inner,
    );
}

extern "C" fn timer_interrupt_inner() {
    unsafe {
        TIMER_TICKS += 1;
        outb(PIC1_COMMAND, 0x20);
    }
}

#[unsafe(naked)]
unsafe extern "C" fn keyboard_interrupt_handler() {
    core::arch::naked_asm!(
        "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
        "push r8", "push r9", "push r10", "push r11",
        "call {rust_handler}",
        "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
        "pop rsi", "pop rdx", "pop rcx", "pop rax",
        "iretq",
        rust_handler = sym keyboard_interrupt_inner,
    );
}

extern "C" fn keyboard_interrupt_inner() {
    unsafe {
        let mut scancode: u8;
        asm!(
            "in al, dx",
            in("dx") 0x60u16,
            out("al") scancode,
            options(nomem, nostack)
        );

        SCANCODES.lock().push(scancode);
        outb(PIC1_COMMAND, 0x20);
    }
}

#[unsafe(naked)]
unsafe extern "C" fn syscall_interrupt_handler() {
    core::arch::naked_asm!(
        "push rcx", "push rdx", "push rsi", "push rdi",
        "push r8", "push r9", "push r10", "push r11",
        "mov rdi, rax",
        "mov rsi, rbx",
        "mov rdx, rcx",
        "call {rust_handler}",
        "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
        "pop rsi", "pop rdx", "pop rcx",
        "iretq",
        rust_handler = sym syscall_interrupt_inner,
    );
}

extern "C" fn syscall_interrupt_inner(sys_num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    crate::syscall::syscall_dispatch(sys_num, arg1, arg2, arg3)
}
