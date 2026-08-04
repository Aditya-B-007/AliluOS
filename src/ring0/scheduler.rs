//! # AliluOS Preemptive Multi-Threaded Scheduler (`ring0/scheduler.rs`)

#![allow(dead_code)]

use crate::config::scheduler::TIME_SLICE_TICKS;
use crate::thread::{CpuContext, ThreadState};
use crate::process::PROCESS_MANAGER;

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

    pub fn init(&mut self) {
        if !self.initialized {
            self.initialized = true;
            self.current_tid = 1; // Default to Shell thread (TID 1)
        }
    }

    pub fn tick_and_schedule(&mut self, ctx: &mut CpuContext) {
        if !self.initialized {
            return;
        }

        if self.ticks_remaining > 1 {
            self.ticks_remaining -= 1;
            return;
        }

        self.ticks_remaining = TIME_SLICE_TICKS;

        let mut pm = PROCESS_MANAGER.lock();
        let total_threads = pm.threads.len();
        if total_threads == 0 {
            return;
        }

        let prev_tid = self.current_tid;
        if let Some(prev_tcb) = pm.threads.get_mut(prev_tid) {
            prev_tcb.context = *ctx;
            if prev_tcb.state == ThreadState::Running {
                prev_tcb.state = ThreadState::Ready;
            }
        }

        let mut next_tid = (prev_tid + 1) % total_threads;
        for _ in 0..total_threads {
            if let Some(tcb) = pm.threads.get(next_tid) {
                if tcb.state == ThreadState::Ready || tcb.state == ThreadState::Running {
                    break;
                }
            }
            next_tid = (next_tid + 1) % total_threads;
        }

        self.current_tid = next_tid;
        if let Some(next_tcb) = pm.threads.get_mut(next_tid) {
            next_tcb.state = ThreadState::Running;
            *ctx = next_tcb.context;
        }
    }
}

pub static SCHEDULER: crate::vga::Locked<Scheduler> = crate::vga::Locked::new(Scheduler::new());
