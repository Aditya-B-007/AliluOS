//! # AliluOS Single-Process Resource Manager (`process.rs`)
//!
//! - **WHAT**: Overarching Kernel Process Control Block (PCB) acting strictly as a **Resource Manager**.
//! - **WHY**: Fulfills the user request: The Process is a resource manager only and does not know anything regarding execution.
//!   It manages memory allocation in primary/secondary storage and network bandwidth, granting resources to executable threads.
//! - **WHEN**: Initialized on kernel startup and queried by threads when allocating RAM, disk storage, or network bandwidth.
//! - **HOW**: Tracks primary memory quotas (`RAM_QUOTA_BYTES`), secondary storage quotas (`DISK_QUOTA_BYTES`), network bandwidth limits, and registered thread states.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use crate::config::process::*;
use crate::thread::{ThreadControlBlock, ThreadAction, ThreadResult, ThreadStats, ThreadState};

/// Primary Memory (RAM) Resource Manager.
#[derive(Debug, Clone)]
pub struct PrimaryMemoryManager {
    pub total_quota_bytes: usize,
    pub allocated_bytes: usize,
}

impl PrimaryMemoryManager {
    pub const fn new() -> Self {
        Self {
            total_quota_bytes: RAM_QUOTA_BYTES,
            allocated_bytes: 0,
        }
    }

    pub fn allocate(&mut self, bytes: usize) -> Result<(), &'static str> {
        if self.allocated_bytes + bytes > self.total_quota_bytes {
            Err("Primary memory quota exceeded")
        } else {
            self.allocated_bytes += bytes;
            Ok(())
        }
    }

    pub fn deallocate(&mut self, bytes: usize) {
        self.allocated_bytes = self.allocated_bytes.saturating_sub(bytes);
    }
}

/// Secondary Storage (Disk) Resource Manager.
#[derive(Debug, Clone)]
pub struct SecondaryStorageManager {
    pub total_quota_bytes: usize,
    pub allocated_bytes: usize,
}

impl SecondaryStorageManager {
    pub const fn new() -> Self {
        Self {
            total_quota_bytes: DISK_QUOTA_BYTES,
            allocated_bytes: 0,
        }
    }

    pub fn allocate(&mut self, bytes: usize) -> Result<(), &'static str> {
        if self.allocated_bytes + bytes > self.total_quota_bytes {
            Err("Secondary storage quota exceeded")
        } else {
            self.allocated_bytes += bytes;
            Ok(())
        }
    }

    pub fn deallocate(&mut self, bytes: usize) {
        self.allocated_bytes = self.allocated_bytes.saturating_sub(bytes);
    }
}

/// Network Bandwidth Resource Manager (Token-Bucket Limiter).
#[derive(Debug, Clone)]
pub struct NetworkBandwidthManager {
    pub rate_limit_bytes_per_sec: u64,
    pub available_tokens: u64,
}

impl NetworkBandwidthManager {
    pub const fn new() -> Self {
        Self {
            rate_limit_bytes_per_sec: NET_BANDWIDTH_LIMIT_BYTES_PER_SEC,
            available_tokens: NET_BANDWIDTH_LIMIT_BYTES_PER_SEC,
        }
    }

    pub fn consume_bandwidth(&mut self, bytes: u64) -> bool {
        if self.available_tokens >= bytes {
            self.available_tokens -= bytes;
            true
        } else {
            false
        }
    }

    pub fn replenish(&mut self) {
        self.available_tokens = self.rate_limit_bytes_per_sec;
    }
}

/// Resource Request sent to Single-Process Resource Manager.
#[derive(Debug, Clone)]
pub enum ProcessResourceRequest {
    AllocateRam(usize),
    FreeRam(usize),
    AllocateDisk(usize),
    FreeDisk(usize),
    RequestNetBandwidth(u64),
    GetSystemResourceStats,
}

/// Response from Single-Process Resource Manager.
#[derive(Debug, Clone)]
pub enum ProcessResourceResponse {
    Success,
    Granted(bool),
    Stats(ProcessResourceSummary),
    Error(&'static str),
}

/// Summary of all resources managed by the Single Process.
#[derive(Debug, Clone)]
pub struct ProcessResourceSummary {
    pub process_id: usize,
    pub ram_allocated: usize,
    pub ram_quota: usize,
    pub disk_allocated: usize,
    pub disk_quota: usize,
    pub net_bandwidth_limit: u64,
    pub net_tokens_available: u64,
    pub active_thread_count: usize,
    pub thread_stats: Vec<ThreadStats>,
}

/// The Single Overarching Kernel Process Control Block (PCB).
pub struct ProcessControlBlock {
    pub pid: usize,
    pub ram_mgr: PrimaryMemoryManager,
    pub disk_mgr: SecondaryStorageManager,
    pub net_mgr: NetworkBandwidthManager,
    pub threads: Vec<ThreadControlBlock>,
}

impl ProcessControlBlock {
    pub const fn new() -> Self {
        Self {
            pid: 1, // Single Overarching System Process ID
            ram_mgr: PrimaryMemoryManager::new(),
            disk_mgr: SecondaryStorageManager::new(),
            net_mgr: NetworkBandwidthManager::new(),
            threads: Vec::new(),
        }
    }

    /// Single Unified Handler Interface for Process Resource Handling.
    pub fn handle(&mut self, request: ProcessResourceRequest) -> ProcessResourceResponse {
        match request {
            ProcessResourceRequest::AllocateRam(bytes) => {
                match self.ram_mgr.allocate(bytes) {
                    Ok(_) => ProcessResourceResponse::Success,
                    Err(e) => ProcessResourceResponse::Error(e),
                }
            }
            ProcessResourceRequest::FreeRam(bytes) => {
                self.ram_mgr.deallocate(bytes);
                ProcessResourceResponse::Success
            }
            ProcessResourceRequest::AllocateDisk(bytes) => {
                match self.disk_mgr.allocate(bytes) {
                    Ok(_) => ProcessResourceResponse::Success,
                    Err(e) => ProcessResourceResponse::Error(e),
                }
            }
            ProcessResourceRequest::FreeDisk(bytes) => {
                self.disk_mgr.deallocate(bytes);
                ProcessResourceResponse::Success
            }
            ProcessResourceRequest::RequestNetBandwidth(bytes) => {
                let granted = self.net_mgr.consume_bandwidth(bytes);
                ProcessResourceResponse::Granted(granted)
            }
            ProcessResourceRequest::GetSystemResourceStats => {
                let mut thread_stats = Vec::new();
                for t in &mut self.threads {
                    if let ThreadResult::Info(stats) = t.handle_request(ThreadAction::GetStats) {
                        thread_stats.push(stats);
                    }
                }
                ProcessResourceResponse::Stats(ProcessResourceSummary {
                    process_id: self.pid,
                    ram_allocated: self.ram_mgr.allocated_bytes,
                    ram_quota: self.ram_mgr.total_quota_bytes,
                    disk_allocated: self.disk_mgr.allocated_bytes,
                    disk_quota: self.disk_mgr.total_quota_bytes,
                    net_bandwidth_limit: self.net_mgr.rate_limit_bytes_per_sec,
                    net_tokens_available: self.net_mgr.available_tokens,
                    active_thread_count: self.threads.len(),
                    thread_stats,
                })
            }
        }
    }

    /// Registers a new execution thread under the Resource Manager process.
    pub fn register_thread(&mut self, name: &str, entry_fn: fn(), stack_top: u64) -> Result<usize, &'static str> {
        let tid = self.threads.len();
        if tid >= MAX_THREADS {
            return Err("Max threads limit reached");
        }

        // Allocate primary memory for thread stack
        self.ram_mgr.allocate(THREAD_STACK_SIZE)?;

        let tcb = ThreadControlBlock::new(tid, name, entry_fn, stack_top);
        self.threads.push(tcb);
        Ok(tid)
    }

    /// Updates state of thread using its single handler interface method.
    pub fn set_thread_state(&mut self, tid: usize, state: ThreadState) {
        if let Some(tcb) = self.threads.get_mut(tid) {
            let _ = tcb.handle_request(ThreadAction::SetState(state));
        }
    }
}

/// Global Thread-Safe Instance of the Single Process Resource Manager.
pub static PROCESS_MANAGER: crate::vga::Locked<ProcessControlBlock> = crate::vga::Locked::new(ProcessControlBlock::new());
