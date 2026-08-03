//! # AliluOS Thread Control Block & Execution Context (`thread.rs`)
//!
//! - **WHAT**: Execution Thread Control Block (TCB) and x86_64 CPU register context manager.
//! - **WHY**: Threads are the sole execution entities in AliluOS. They hold execution state (`RIP`, `RSP`, registers).
//! - **WHEN**: Instantiated during kernel boot and scheduled by the thread scheduler (`scheduler.rs`).
//! - **HOW**: Exposes **a single unified handler method (`handle_request`)** to the Process Resource Manager for all thread interactions.

#![allow(dead_code)]

use alloc::string::String;
use crate::config::process::THREAD_STACK_SIZE;

/// Thread Execution Lifecycle State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

/// Saved x86_64 Register CPU Execution Context.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CpuContext {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9:  u64,
    pub r8:  u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs:  u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss:  u64,
}

impl CpuContext {
    pub const fn empty() -> Self {
        Self {
            r15: 0, r14: 0, r13: 0, r12: 0, r11: 0, r10: 0, r9: 0, r8: 0,
            rbp: 0, rdi: 0, rsi: 0, rdx: 0, rcx: 0, rbx: 0, rax: 0,
            rip: 0, cs: 0x08, rflags: 0x202, rsp: 0, ss: 0x10,
        }
    }
}

/// Unified Request Actions sent to Thread Handler.
#[derive(Debug, Clone)]
pub enum ThreadAction {
    QueryState,
    SetState(ThreadState),
    UpdateResources { ram_bytes: usize, disk_blocks: usize, net_tokens: u64 },
    GetStats,
}

/// Information returned by Thread Handler.
#[derive(Debug, Clone)]
pub struct ThreadStats {
    pub tid: usize,
    pub name: String,
    pub state: ThreadState,
    pub priority: u8,
    pub stack_size: usize,
    pub allocated_ram: usize,
    pub allocated_disk_blocks: usize,
    pub net_bandwidth_tokens: u64,
}

/// Result returned by the unified Thread Handler.
#[derive(Debug, Clone)]
pub enum ThreadResult {
    State(ThreadState),
    Info(ThreadStats),
    Success,
    Error(&'static str),
}

/// Thread Control Block (TCB).
pub struct ThreadControlBlock {
    pub tid: usize,
    pub name: String,
    pub state: ThreadState,
    pub priority: u8,
    pub context: CpuContext,
    pub stack_base: u64,
    pub stack_size: usize,
    pub allocated_ram: usize,
    pub allocated_disk_blocks: usize,
    pub net_bandwidth_tokens: u64,
}

impl ThreadControlBlock {
    /// Constructs a new Thread Control Block.
    pub fn new(tid: usize, name: &str, entry_fn: fn(), stack_top: u64) -> Self {
        let mut ctx = CpuContext::empty();
        ctx.rip = entry_fn as u64;
        ctx.rsp = stack_top;

        Self {
            tid,
            name: String::from(name),
            state: ThreadState::Ready,
            priority: 1,
            context: ctx,
            stack_base: stack_top.saturating_sub(THREAD_STACK_SIZE as u64),
            stack_size: THREAD_STACK_SIZE,
            allocated_ram: THREAD_STACK_SIZE,
            allocated_disk_blocks: 0,
            net_bandwidth_tokens: 1000,
        }
    }

    /// Single Unified Method for Process/System Interaction.
    ///
    /// - **WHAT**: Handles all inspection, state transitions, and resource assignment requests.
    /// - **WHY**: Fulfills the explicit single-method class interface design for simple thread-process architecture.
    pub fn handle_request(&mut self, action: ThreadAction) -> ThreadResult {
        match action {
            ThreadAction::QueryState => ThreadResult::State(self.state),
            ThreadAction::SetState(new_state) => {
                self.state = new_state;
                ThreadResult::Success
            }
            ThreadAction::UpdateResources { ram_bytes, disk_blocks, net_tokens } => {
                self.allocated_ram = ram_bytes;
                self.allocated_disk_blocks = disk_blocks;
                self.net_bandwidth_tokens = net_tokens;
                ThreadResult::Success
            }
            ThreadAction::GetStats => ThreadResult::Info(ThreadStats {
                tid: self.tid,
                name: self.name.clone(),
                state: self.state,
                priority: self.priority,
                stack_size: self.stack_size,
                allocated_ram: self.allocated_ram,
                allocated_disk_blocks: self.allocated_disk_blocks,
                net_bandwidth_tokens: self.net_bandwidth_tokens,
            }),
        }
    }
}
