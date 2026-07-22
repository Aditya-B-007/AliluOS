use core::arch::asm;
use crate::vga::{Color, VGA};

/// Global Descriptor Table (GDT) Entry structure for 64-bit mode.
/// Standard segment descriptors are 8 bytes long.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct GdtEntry {
    limit_low: u16,        // Low 16 bits of the segment limit
    base_low: u16,         // Low 16 bits of the base address
    base_middle: u8,       // Middle 8 bits of the base address
    access_byte: u8,       // Access control flags (e.g., ring level, read/write/execute permissions)
    flags_limit_high: u8,  // Granularity flags & high 4 bits of the segment limit
    base_high: u8,         // High 8 bits of the base address
}

/// System Segment Descriptor for 64-bit mode (e.g., Task State Segment - TSS).
/// In 64-bit mode, system segment descriptors are expanded to 16 bytes to support 64-bit base addresses.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct GdtSystemEntry {
    limit_low: u16,        // Low 16 bits of the system segment limit
    base_low: u16,         // Low 16 bits of the base address
    base_middle: u8,       // Middle 8 bits of the base address
    access_byte: u8,       // Type & access flags (indicates it is a TSS, privilege levels, etc.)
    flags_limit_high: u8,  // Granularity flags & high 4 bits of the limit
    base_high_middle: u8,  // Next 8 bits of the base address
    base_high: u32,        // High 32 bits of the base address
    reserved: u32,         // Reserved/Zero bits
}

/// Global Descriptor Table (GDT) layout.
/// Standard structure containing the null segment, kernel code segment, kernel data segment,
/// and the system segment descriptor pointing to the TSS.
#[repr(C, align(16))]
struct Gdt {
    null: GdtEntry,
    code: GdtEntry,
    data: GdtEntry,
    tss: GdtSystemEntry,
}

/// GDT Pointer structure loaded into the CPU's GDTR register using `lgdt`.
#[repr(C, packed)]
struct GdtPointer {
    limit: u16, // Size of GDT in bytes minus 1
    base: u64,  // Virtual address of the GDT
}

/// Task State Segment (TSS) structure for x86_64.
/// In 64-bit mode, its primary role is to store stack pointers for different privilege levels (RSP0-2)
/// and the Interrupt Stack Table (IST), used to swap stacks during exceptions/interrupts safely.
#[repr(C, packed)]
struct TaskStateSegment {
    reserved_1: u32,
    rsp0: u64,          // Stack pointer for Ring 0 (Kernel)
    rsp1: u64,          // Stack pointer for Ring 1
    rsp2: u64,          // Stack pointer for Ring 2
    reserved_2: u64,
    ist1: u64,          // Interrupt Stack Table pointers 1 through 7
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved_3: u64,
    reserved_4: u16,
    iomap_base: u16,    // Offset to I/O Permission Bit Map
}

/// Interrupt Descriptor Table (IDT) entry structure for 64-bit mode (16 bytes).
/// Each entry (gate) describes how to handle a specific interrupt vector.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    pointer_low: u16,     // Low 16 bits of the ISR (Interrupt Service Routine) function address
    gdt_selector: u16,    // Segment selector in the GDT (points to kernel code segment)
    options: u16,         // Entry flags: Presence, Ring Privilege Level (DPL), Gate Type, IST index
    pointer_middle: u16,  // Middle 16 bits of the ISR address
    pointer_high: u32,    // High 32 bits of the ISR address
    reserved: u32,        // Reserved/Zero
}

impl IdtEntry {
    /// Creates a missing (disabled) IDT entry.
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

    /// Sets up the IDT gate entry to point to a specific ISR function.
    fn set_handler(&mut self, handler: u64) {
        self.pointer_low = (handler & 0xFFFF) as u16;
        self.gdt_selector = 8; // Kernel code segment selector is at index 1 (1 * 8 = 8)
        self.options = 0x8E00; // Present, Ring 0, 64-bit Interrupt Gate type
        self.pointer_middle = ((handler >> 16) & 0xFFFF) as u16;
        self.pointer_high = ((handler >> 32) & 0xFFFFFFFF) as u32;
        self.reserved = 0;
    }
}

/// IDT Pointer structure loaded into the CPU's IDTR register using `lidt`.
#[repr(C, packed)]
struct IdtPointer {
    limit: u16, // Size of the IDT in bytes minus 1
    base: u64,  // Virtual address of the IDT array
}

// 8259 Programmable Interrupt Controller (PIC) I/O Ports
const PIC1_COMMAND: u16 = 0x20; // Master PIC Command Port
const PIC1_DATA: u16 = 0x21;    // Master PIC Data/Mask Port
const PIC2_COMMAND: u16 = 0xA0; // Slave PIC Command Port
const PIC2_DATA: u16 = 0xA1;    // Slave PIC Data/Mask Port

// 8253/8254 Programmable Interval Timer (PIT) I/O Ports
const PIT_CHANNEL_0: u16 = 0x40; // Channel 0 Data Port
const PIT_COMMAND: u16 = 0x43;   // Mode/Command register

/// Global GDT instance. Needs static mut as we modify the TSS descriptor base address at runtime.
static mut GDT: Gdt = Gdt {
    null: GdtEntry {
        limit_low: 0,
        base_low: 0,
        base_middle: 0,
        access_byte: 0,
        flags_limit_high: 0,
        base_high: 0,
    },
    code: GdtEntry {
        limit_low: 0,
        base_low: 0,
        base_middle: 0,
        access_byte: 0x9A,     // Present, Ring 0, Code, Executable, Readable
        flags_limit_high: 0x20, // Long Mode flag set
        base_high: 0,
    },
    data: GdtEntry {
        limit_low: 0,
        base_low: 0,
        base_middle: 0,
        access_byte: 0x92,     // Present, Ring 0, Data, Writable
        flags_limit_high: 0,
        base_high: 0,
    },
    tss: GdtSystemEntry {
        limit_low: 0,
        base_low: 0,
        base_middle: 0,
        access_byte: 0,         // Populated dynamically at runtime
        flags_limit_high: 0,
        base_high_middle: 0,
        base_high: 0,
        reserved: 0,
    },
};

/// Global Task State Segment (TSS) instance.
static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved_1: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    reserved_2: 0,
    ist1: 0,
    ist2: 0,
    ist3: 0,
    ist4: 0,
    ist5: 0,
    ist6: 0,
    ist7: 0,
    reserved_3: 0,
    reserved_4: 0,
    iomap_base: 104, // Offset to I/O map points to end of TSS structure (104 bytes)
};

/// Global 256-entry Interrupt Descriptor Table (IDT) instance.
static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];

/// System timer tick counter.
static mut TIMER_TICKS: u64 = 0;

/// Initializes GDT, IDT, PIC, and PIT configurations.
pub fn init() {
    unsafe {
        setup_gdt();
        setup_idt();
        setup_pic();
        setup_pit(100); // 100 Hz timer frequency
    }
}

/// Configures the Global Descriptor Table, writes TSS pointers, and loads them using assembly.
unsafe fn setup_gdt() {
    let tss_address = &TSS as *const TaskStateSegment as u64;
    let tss_size = core::mem::size_of::<TaskStateSegment>() as u32;

    // Dynamically calculate and write the 64-bit base address and limit of TSS in the GDT descriptor
    GDT.tss.limit_low = (tss_size - 1) as u16;
    GDT.tss.base_low = (tss_address & 0xFFFF) as u16;
    GDT.tss.base_middle = ((tss_address >> 16) & 0xFF) as u8;
    GDT.tss.access_byte = 0x89; // Present, Ring 0, Type: 64-bit TSS (available)
    GDT.tss.flags_limit_high = (((tss_size - 1) >> 16) & 0x0F) as u8;
    GDT.tss.base_high_middle = ((tss_address >> 24) & 0xFF) as u8;
    GDT.tss.base_high = (tss_address >> 32) as u32;

    let gdt_ptr = GdtPointer {
        limit: (core::mem::size_of::<Gdt>() - 1) as u16,
        base: &GDT as *const Gdt as u64,
    };

    // Load GDT, update segment registers to point to Kernel Data segment (0x10),
    // perform a far return (`retf`) to set Kernel Code segment selector (0x08),
    // and load the Task Register (`ltr`) to point to the TSS descriptor (0x18).
    asm!(
        "lgdt [{}]",
        "mov ax, 0x10", // Load Kernel Data segment selector (index 2: 16)
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",
        "push 0x08", // Push code segment selector to stack for long far return
        "lea rax, [2f]",
        "push rax",  // Push target address to stack
        "rex64 retf", // Far return: pops CS and RIP from stack (enters Ring 0 long mode segment)
        "2:",
        "mov ax, 0x18", // Load TSS segment selector (index 3: 24)
        "ltr ax",       // Load Task Register
        in(reg) &gdt_ptr,
        options(readonly, nostack, preserves_flags)
    );
}

/// Sets up individual interrupt vectors in the IDT and loads the IDT pointer.
unsafe fn setup_idt() {
    // 1. Exceptions (Vectors 0 - 31)
    IDT[0].set_handler(divide_by_zero_handler as u64); // Divide by Zero
    IDT[8].set_handler(double_fault_handler as u64);   // Double Fault
    IDT[14].set_handler(page_fault_handler as u64);    // Page Fault

    // 2. Hardware Interrupts (Vectors 32 - 47)
    IDT[32].set_handler(timer_interrupt_handler as u64);    // IRQ0: Timer
    IDT[33].set_handler(keyboard_interrupt_handler as u64); // IRQ1: Keyboard

    let idt_ptr = IdtPointer {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: &IDT as *const [IdtEntry; 256] as u64,
    };

    // Load the IDT pointer into CPU's IDTR register
    asm!(
        "lidt [{}]",
        in(reg) &idt_ptr,
        options(readonly, nostack, preserves_flags)
    );
}

/// Reconfigures and initializes the dual 8259 PIC.
/// Out of the box, the master PIC maps IRQ0-7 to vectors 0x08-0x0F, which clashes with CPU exceptions.
/// We remap them to start at 0x20 (IRQ0-7) and 0x28 (IRQ8-15).
unsafe fn setup_pic() {
    // ICW1 (Initialization Control Word 1): Start initialization sequence
    outb(PIC1_COMMAND, 0x11);
    outb(PIC2_COMMAND, 0x11);

    // ICW2: Set vector offsets
    outb(PIC1_DATA, 0x20); // Master IRQ0-7 -> vectors 0x20-0x27 (32-39)
    outb(PIC2_DATA, 0x28); // Slave IRQ8-15 -> vectors 0x28-0x2F (40-47)

    // ICW3: Establish Master/Slave cascade connection
    outb(PIC1_DATA, 0x04); // Master PIC has a slave connected on IRQ2 (0b00000100)
    outb(PIC2_DATA, 0x02); // Slave PIC is connected to master's IRQ line 2

    // ICW4: Set modes
    outb(PIC1_DATA, 0x01); // 8086 mode (standard x86 execution)
    outb(PIC2_DATA, 0x01);

    // Set Interrupt Masks (disables all IRQs except those set to 0)
    outb(PIC1_DATA, 0xFC); // Enable IRQ0 (Timer) and IRQ1 (Keyboard) (0b11111100)
    outb(PIC2_DATA, 0xFF); // Disable all lines on slave PIC
}

/// Configures the 8253/8254 Programmable Interval Timer (PIT).
/// Sets the frequency divisor to tick at the requested frequency.
unsafe fn setup_pit(frequency: u32) {
    let divisor = 1193182 / frequency; // Base oscillator rate is 1.193182 MHz
    outb(PIT_COMMAND, 0x36); // Command: Channel 0, access low/high byte, square wave mode
    outb(PIT_CHANNEL_0, (divisor & 0xFF) as u8);        // Send low byte
    outb(PIT_CHANNEL_0, ((divisor >> 8) & 0xFF) as u8); // Send high byte
}

/// Basic port output function using inline assembly.
unsafe fn outb(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nostack, nomem)
    );
}

/// Helper macro to generate naked/assembly wrapper entry points for simple exceptions.
/// Naked functions do not generate prologues/epilogues, allowing precise assembly control.
macro_rules! exception_handler {
    ($name:ident, $msg:expr) => {
        #[naked]
        unsafe extern "C" fn $name() {
            asm!(
                // Save caller-saved (volatile) registers
                "push rax",
                "push rcx",
                "push rdx",
                "push rsi",
                "push rdi",
                "push r8",
                "push r9",
                "push r10",
                "push r11",
                // Call the actual Rust handling logic
                "call {rust_handler}",
                // Restore volatile registers
                "pop r11",
                "pop r10",
                "pop r9",
                "pop r8",
                "pop rdi",
                "pop rsi",
                "pop rdx",
                "pop rcx",
                "pop rax",
                "iretq", // Interrupt return (64-bit)
                rust_handler = sym $name_inner,
                options(noreturn)
            );
        }

        extern "C" fn $name_inner() {
            let mut vga = VGA::new();
            vga.set_color(Color::LightRed, Color::Black);
            vga.println("\n--- CPU EXCEPTION ---");
            vga.println($msg);
            vga.println("Halting system.");
            loop {} // Freeze system
        }
    };
}

// Generate simple CPU exception handlers
exception_handler!(divide_by_zero_handler, "Divide by Zero Exception (0x00)");
exception_handler!(double_fault_handler, "Double Fault Exception (0x08)");

/// Naked assembly handler for Page Fault exception.
/// Page Faults push a custom error code onto the stack, and store the faulting virtual address in `cr2`.
#[naked]
unsafe extern "C" fn page_fault_handler() {
    asm!(
        // Save register states
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "mov rdi, cr2", // Load faulting address from CR2 to RDI (the first argument to the Rust function)
        "call {rust_handler}",
        // Restore register states
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "add rsp, 8", // Clear the CPU-pushed error code off the stack
        "iretq",      // Return from interrupt
        rust_handler = sym page_fault_inner,
        options(noreturn)
    );
}

/// Page Fault handler logic. Displays the faulting address to the user in hexadecimal.
extern "C" fn page_fault_inner(faulting_address: u64) {
    let mut vga = VGA::new();
    vga.set_color(Color::LightRed, Color::Black);
    vga.println("\n--- CPU EXCEPTION ---");
    vga.write("Page Fault at address: 0x");
    
    // Hexadecimal string formatting helper (no Alloc/Std support)
    let mut temp = faulting_address;
    for i in (0..16).rev() {
        let digit = ((temp >> (i * 4)) & 0xF) as u8;
        let c = if digit < 10 {
            (b'0' + digit) as char
        } else {
            (b'A' + (digit - 10)) as char
        };
        vga.put_char(c);
    }
    vga.println("\nHalting system.");
    loop {}
}

/// Naked assembly wrapper for PIT Timer interrupt.
#[naked]
unsafe extern "C" fn timer_interrupt_handler() {
    asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "call {rust_handler}",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
        rust_handler = sym timer_interrupt_inner,
        options(noreturn)
    );
}

/// Timer interrupt handler logic. Increments tick count and acknowledges PIC1.
extern "C" fn timer_interrupt_inner() {
    unsafe {
        TIMER_TICKS += 1;
        // Send End of Interrupt (EOI) command to PIC1
        outb(PIC1_COMMAND, 0x20);
    }
}

/// Naked assembly wrapper for PS/2 Keyboard interrupt.
#[naked]
unsafe extern "C" fn keyboard_interrupt_handler() {
    asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "call {rust_handler}",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
        rust_handler = sym keyboard_interrupt_inner,
        options(noreturn)
    );
}

/// Keyboard interrupt handler logic. Sends EOI to master PIC.
extern "C" fn keyboard_interrupt_inner() {
    unsafe {
        // Send End of Interrupt (EOI) command to PIC1
        outb(PIC1_COMMAND, 0x20);
    }
}

/// Safely returns the current system timer ticks.
pub fn timer_ticks() -> u64 {
    unsafe { TIMER_TICKS }
}
