use core::arch::asm;
use crate::vga::{Color, VGA, WRITER};

/// Fixed-capacity Lock-Free Ring Buffer for PS/2 Scancodes.
///
/// WHAT IT DOES:
/// Stores raw hardware scancode bytes received from PS/2 keyboard port 0x60 in a circular buffer array.
///
/// WHY IT DOES IT:
/// Decouples low-level hardware IRQ1 interrupt handling from high-level shell and keyboard event processing.
/// Reading port 0x60 immediately in the interrupt handler prevents the hardware PS/2 controller from stalling
/// or dropping keystrokes during fast typing or long input lines.
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

    /// Pushes a scancode byte into the ring buffer (called inside IRQ1 interrupt handler).
    pub fn push(&mut self, scancode: u8) {
        let next_head = (self.head + 1) % 256;
        if next_head != self.tail {
            self.buffer[self.head] = scancode;
            self.head = next_head;
        }
    }

    /// Pops a scancode byte from the ring buffer if available (called by keyboard consumer).
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

/// Global thread-safe scancode queue instance.
pub static SCANCODES: crate::vga::Locked<ScancodeBuffer> = crate::vga::Locked::new(ScancodeBuffer::new());

/// Global Descriptor Table (GDT) Entry structure for 64-bit mode.
/// Standard segment descriptors are 8 bytes long.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct GdtEntry {
    limit_low: u16,        // Low 16 bits of segment limit
    base_low: u16,         // Low 16 bits of base address
    base_middle: u8,       // Middle 8 bits of base address
    access_byte: u8,       // Privilege ring and access flags
    flags_limit_high: u8,  // Granularity & high limit bits
    base_high: u8,         // High 8 bits of base address
}

/// System Segment Descriptor for 64-bit mode (e.g. Task State Segment - TSS).
/// Expanded to 16 bytes in 64-bit mode to hold full 64-bit addresses.
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

/// Global Descriptor Table (GDT) layout containing null, kernel code, kernel data, and TSS descriptors.
#[repr(C, align(16))]
struct Gdt {
    null: GdtEntry,
    code: GdtEntry,
    data: GdtEntry,
    tss: GdtSystemEntry,
}

/// GDT Pointer structure passed to CPU's `lgdt` instruction.
#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

/// Task State Segment (TSS) structure for x86_64 CPU mode.
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

/// Interrupt Descriptor Table (IDT) entry structure for 64-bit mode (16 bytes).
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
    const fn missing() -> Self {
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
        self.gdt_selector = 8; // Code segment selector at 0x08
        self.options = 0x8E00; // Present, Ring 0, 64-bit Interrupt Gate
        self.pointer_middle = ((handler >> 16) & 0xFFFF) as u16;
        self.pointer_high = ((handler >> 32) & 0xFFFFFFFF) as u32;
        self.reserved = 0;
    }
}

/// IDT Pointer structure passed to CPU's `lidt` instruction.
#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const PIT_CHANNEL_0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

static mut GDT: Gdt = Gdt {
    null: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0, flags_limit_high: 0, base_high: 0 },
    code: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0x9A, flags_limit_high: 0x20, base_high: 0 },
    data: GdtEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0x92, flags_limit_high: 0, base_high: 0 },
    tss: GdtSystemEntry { limit_low: 0, base_low: 0, base_middle: 0, access_byte: 0, flags_limit_high: 0, base_high_middle: 0, base_high: 0, reserved: 0 },
};

static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved_1: 0, rsp0: 0, rsp1: 0, rsp2: 0, reserved_2: 0,
    ist1: 0, ist2: 0, ist3: 0, ist4: 0, ist5: 0, ist6: 0, ist7: 0,
    reserved_3: 0, reserved_4: 0, iomap_base: 104,
};

static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];
static mut TIMER_TICKS: u64 = 0;

/// Initializes CPU low-level hardware structures: GDT, IDT, dual 8259 PICs, and PIT timer.
///
/// WHAT IT DOES:
/// Sets up memory protection segments, builds exception/interrupt gates, remaps hardware PIC vector offsets,
/// and configures the system clock.
///
/// WHY IT DOES IT:
/// Prepares x86_64 bare-metal CPU mode for handling hardware interrupts safely without crashing or rebooting.
pub fn init() {
    unsafe {
        setup_gdt();
        setup_idt();
        setup_pic();
        setup_pit(100);
    }
}

/// Configures GDT entries and TSS segment descriptors, then loads them into CPU register GDTR and TR.
unsafe fn setup_gdt() {
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
        "mov ax, 0x18",
        "ltr ax",
        in(reg) &gdt_ptr,
        options(readonly, nostack, preserves_flags)
    );
}

/// Sets up exception handlers and hardware interrupt vectors in the IDT, then executes `lidt`.
unsafe fn setup_idt() {
    IDT[0].set_handler(divide_by_zero_handler as u64);
    IDT[8].set_handler(double_fault_handler as u64);
    IDT[14].set_handler(page_fault_handler as u64);

    IDT[32].set_handler(timer_interrupt_handler as u64);    // IRQ0: Timer
    IDT[33].set_handler(keyboard_interrupt_handler as u64); // IRQ1: Keyboard

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

/// Remaps 8259 PIC IRQs 0-15 to CPU interrupt vectors 32-47 to avoid collision with CPU exceptions 0-31.
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

/// Configures 8254 PIT timer frequency (e.g. 100 Hz).
unsafe fn setup_pit(frequency: u32) {
    let divisor = 1193182 / frequency;
    outb(PIT_COMMAND, 0x36);
    outb(PIT_CHANNEL_0, (divisor & 0xFF) as u8);
    outb(PIT_CHANNEL_0, ((divisor >> 8) & 0xFF) as u8);
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
    ($name:ident, $msg:expr) => {
        #[naked]
        unsafe extern "C" fn $name() {
            asm!(
                "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
                "push r8", "push r9", "push r10", "push r11",
                "call {rust_handler}",
                "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
                "pop rsi", "pop rdx", "pop rcx", "pop rax",
                "iretq",
                rust_handler = sym $name_inner,
                options(noreturn)
            );
        }

        extern "C" fn $name_inner() {
            let mut vga = WRITER.lock();
            vga.set_color(Color::LightRed, Color::Black);
            vga.println("\n--- CPU EXCEPTION ---");
            vga.println($msg);
            vga.println("Halting system.");
            loop {}
        }
    };
}

exception_handler!(divide_by_zero_handler, "Divide by Zero Exception (0x00)");
exception_handler!(double_fault_handler, "Double Fault Exception (0x08)");

#[naked]
unsafe extern "C" fn page_fault_handler() {
    asm!(
        "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
        "push r8", "push r9", "push r10", "push r11",
        "mov rdi, cr2",
        "call {rust_handler}",
        "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
        "pop rsi", "pop rdx", "pop rcx", "pop rax",
        "add rsp, 8",
        "iretq",
        rust_handler = sym page_fault_inner,
        options(noreturn)
    );
}

extern "C" fn page_fault_inner(faulting_address: u64) {
    let mut vga = WRITER.lock();
    vga.set_color(Color::LightRed, Color::Black);
    vga.println("\n--- CPU EXCEPTION ---");
    vga.write("Page Fault at address: 0x");
    let mut temp = faulting_address;
    for i in (0..16).rev() {
        let digit = ((temp >> (i * 4)) & 0xF) as u8;
        let c = if digit < 10 { (b'0' + digit) as char } else { (b'A' + (digit - 10)) as char };
        vga.put_char(c);
    }
    vga.println("\nHalting system.");
    loop {}
}

#[naked]
unsafe extern "C" fn timer_interrupt_handler() {
    asm!(
        "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
        "push r8", "push r9", "push r10", "push r11",
        "call {rust_handler}",
        "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
        "pop rsi", "pop rdx", "pop rcx", "pop rax",
        "iretq",
        rust_handler = sym timer_interrupt_inner,
        options(noreturn)
    );
}

extern "C" fn timer_interrupt_inner() {
    unsafe {
        TIMER_TICKS += 1;
        outb(PIC1_COMMAND, 0x20);
    }
}

/// Assembly interrupt handler wrapper for IRQ1 PS/2 Keyboard interrupts.
///
/// WHAT IT DOES:
/// Saves CPU caller-saved registers onto stack, calls `keyboard_interrupt_inner`, restores registers, and issues `iretq`.
///
/// WHY IT DOES IT:
/// Preserves register states so kernel execution resumes transparently after the interrupt handler finishes.
#[naked]
unsafe extern "C" fn keyboard_interrupt_handler() {
    asm!(
        "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
        "push r8", "push r9", "push r10", "push r11",
        "call {rust_handler}",
        "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
        "pop rsi", "pop rdx", "pop rcx", "pop rax",
        "iretq",
        rust_handler = sym keyboard_interrupt_inner,
        options(noreturn)
    );
}

/// Keyboard interrupt handler inner execution function.
///
/// WHAT IT DOES:
/// Reads raw hardware scancode byte from PS/2 data port 0x60, pushes it into `SCANCODES` ring buffer,
/// and sends an End of Interrupt (EOI) command `0x20` to PIC1 command port 0x20.
///
/// WHY IT DOES IT:
/// Draining port 0x60 immediately inside the IRQ1 handler guarantees the PS/2 controller buffer never overflows
/// or locks up typing, even during long or continuous key presses.
extern "C" fn keyboard_interrupt_inner() {
    unsafe {
        let scancode: u8;
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

/// Returns total system timer ticks since boot.
pub fn timer_ticks() -> u64 {
    unsafe { TIMER_TICKS }
}

/// Pops a pending scancode from the interrupt-driven ring buffer.
pub fn pop_scancode() -> Option<u8> {
    SCANCODES.lock().pop()
}
