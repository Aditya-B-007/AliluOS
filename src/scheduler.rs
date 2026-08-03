//! # AliluOS Preemptive Multi-Threaded Scheduler (`scheduler.rs`)
//!
//! - **WHAT**: Round-Robin preemptive thread context scheduler.
//! - **WHY**: Dispatches execution time across kernel threads (TID 0 to 10) driven by PIT IRQ0 timer ticks.
//! - **WHEN**: Triggered on every PIT timer interrupt (100 Hz).
//! - **HOW**: Saves current thread CPU registers (`CpuContext`), updates thread state, selects next `Ready` thread, and switches context.

#![allow(dead_code)]

use alloc::vec::Vec;
use crate::config::scheduler::TIME_SLICE_TICKS;
use crate::thread::{CpuContext, ThreadState};
use crate::process::PROCESS_MANAGER;

/// Multi-Threaded Kernel Scheduler State.
pub struct Scheduler {
    pub current_tid: usize,
    pub ticks_remaining: u64,
    pub initialized: bool,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            current_tid: 0,
            ticks_remaining: TIME_SLICE_TICKS,
            initialized: false,
        }
    }

    /// Initializes scheduler state.
    pub fn init(&mut self) {
        if !self.initialized {
            self.initialized = true;
            self.current_tid = 1; // Default to Shell thread (TID 1)
        }
    }

    /// Invoked on every PIT timer tick (100 Hz) to perform preemptive thread scheduling.
    pub fn tick_and_schedule(&mut self, ctx: &mut CpuContext) {
        if !self.initialized {
            return;
        }

        if self.ticks_remaining > 1 {
            self.ticks_remaining -= 1;
            return;
        }

        // Time slice expired: reset timer quantum and context switch
        self.ticks_remaining = TIME_SLICE_TICKS;

        let mut pm = PROCESS_MANAGER.lock();
        let total_threads = pm.threads.len();
        if total_threads == 0 {
            return;
        }

        // Save current thread's CPU register context
        let prev_tid = self.current_tid;
        if let Some(prev_tcb) = pm.threads.get_mut(prev_tid) {
            prev_tcb.context = *ctx;
            if prev_tcb.state == ThreadState::Running {
                prev_tcb.state = ThreadState::Ready;
            }
        }

        // Round-robin selection of next Ready thread
        let mut next_tid = (prev_tid + 1) % total_threads;
        for _ in 0..total_threads {
            if let Some(tcb) = pm.threads.get(next_tid) {
                if tcb.state == ThreadState::Ready || tcb.state == ThreadState::Running {
                    break;
                }
            }
            next_tid = (next_tid + 1) % total_threads;
        }

        // Switch execution context to next thread
        self.current_tid = next_tid;
        if let Some(next_tcb) = pm.threads.get_mut(next_tid) {
            next_tcb.state = ThreadState::Running;
            *ctx = next_tcb.context;
        }
    }
}

/// Global Thread-Safe Instance of the Multi-Threaded Scheduler.
pub static SCHEDULER: crate::vga::Locked<Scheduler> = crate::vga::Locked::new(Scheduler::new());
