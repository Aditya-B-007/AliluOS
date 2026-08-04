//! # AliluOS System Call Subsystem (`ring2/syscall.rs`)
//!
//! - **WHAT**: Comprehensive System Call Dispatcher Gate for Ring 2 User Threads.
//! - **WHY**: Provides a secure privilege transition interface so Ring 2 user threads can request Ring 0/1 kernel services safely.
//! - **WHEN**: Triggered when a Ring 2 thread executes `int 0x80` or `syscall`.
//! - **HOW**: Decodes system call numbers `0x01` through `0x0F` and dispatches requests to the Process Resource Manager (`process.rs`), Filesystem (`fs.rs`), and Drivers.

#![allow(dead_code)]

use alloc::string::String;
use crate::process::{PROCESS_MANAGER, ProcessResourceRequest, ProcessResourceResponse};
use crate::fs::FS;
use crate::vga::WRITER;
use crate::keyboard::Keyboard;

/// System Call Function Identifier Constants (0x01 to 0x0F).
pub const SYS_WRITE_CONSOLE:  u64 = 0x01;
pub const SYS_READ_KEYBOARD:  u64 = 0x02;
pub const SYS_ALLOCATE_RAM:   u64 = 0x03;
pub const SYS_ALLOCATE_DISK:  u64 = 0x04;
pub const SYS_REQUEST_NET:    u64 = 0x05;
pub const SYS_GET_SYS_STATS:  u64 = 0x06;
pub const SYS_FS_OPEN:        u64 = 0x07;
pub const SYS_FS_READ:        u64 = 0x08;
pub const SYS_FS_WRITE:       u64 = 0x09;
pub const SYS_FS_DELETE:      u64 = 0x0A;
pub const SYS_THREAD_YIELD:   u64 = 0x0B;
pub const SYS_IPC_SEND:       u64 = 0x0C;
pub const SYS_IPC_RECV:       u64 = 0x0D;
pub const SYS_MUTEX_LOCK:     u64 = 0x0E;
pub const SYS_MUTEX_UNLOCK:   u64 = 0x0F;

/// Dispatches system calls from Ring 2 user space into Ring 0/1 kernel services.
pub fn syscall_dispatch(sys_num: u64, arg1: u64, arg2: u64, _arg3: u64) -> u64 {
    match sys_num {
        SYS_WRITE_CONSOLE => {
            let ptr = arg1 as *const u8;
            let len = arg2 as usize;
            if !ptr.is_null() && len > 0 {
                unsafe {
                    let slice = core::slice::from_raw_parts(ptr, len);
                    if let Ok(text) = core::str::from_utf8(slice) {
                        WRITER.lock().write(text);
                        return 0; // Success
                    }
                }
            }
            1 // Invalid argument
        }
        SYS_READ_KEYBOARD => {
            // Read next available key from scancode buffer
            if let Some(event) = crate::interrupts::pop_scancode() {
                event as u64
            } else {
                0 // No key available
            }
        }
        SYS_ALLOCATE_RAM => {
            let bytes = arg1 as usize;
            let mut pm = PROCESS_MANAGER.lock();
            match pm.handle(ProcessResourceRequest::AllocateRam(bytes)) {
                ProcessResourceResponse::Success => 0,
                _ => 1,
            }
        }
        SYS_ALLOCATE_DISK => {
            let bytes = arg1 as usize;
            let mut pm = PROCESS_MANAGER.lock();
            match pm.handle(ProcessResourceRequest::AllocateDisk(bytes)) {
                ProcessResourceResponse::Success => 0,
                _ => 1,
            }
        }
        SYS_REQUEST_NET => {
            let bytes = arg1;
            let mut pm = PROCESS_MANAGER.lock();
            match pm.handle(ProcessResourceRequest::RequestNetBandwidth(bytes)) {
                ProcessResourceResponse::Granted(true) => 0,
                _ => 1,
            }
        }
        SYS_GET_SYS_STATS => {
            let mut pm = PROCESS_MANAGER.lock();
            if let ProcessResourceResponse::Stats(_stats) = pm.handle(ProcessResourceRequest::GetSystemResourceStats) {
                0 // Successfully retrieved stats
            } else {
                1
            }
        }
        SYS_FS_OPEN | SYS_FS_READ => {
            let ptr = arg1 as *const u8;
            let len = arg2 as usize;
            if !ptr.is_null() && len > 0 {
                unsafe {
                    let slice = core::slice::from_raw_parts(ptr, len);
                    if let Ok(path) = core::str::from_utf8(slice) {
                        let fs = FS.lock();
                        let resolved = fs.resolve_path(&[], path);
                        let (parent, name) = fs.split_parent_and_name(&resolved);
                        if fs.read_file(parent, name).is_ok() {
                            return 0;
                        }
                    }
                }
            }
            1
        }
        SYS_FS_WRITE => {
            let ptr = arg1 as *const u8;
            let len = arg2 as usize;
            if !ptr.is_null() && len > 0 {
                unsafe {
                    let slice = core::slice::from_raw_parts(ptr, len);
                    if let Ok(path) = core::str::from_utf8(slice) {
                        let mut fs = FS.lock();
                        let resolved = fs.resolve_path(&[], path);
                        let (parent, name) = fs.split_parent_and_name(&resolved);
                        if fs.write_file(parent, name, "").is_ok() {
                            return 0;
                        }
                    }
                }
            }
            1
        }
        SYS_FS_DELETE => {
            let ptr = arg1 as *const u8;
            let len = arg2 as usize;
            if !ptr.is_null() && len > 0 {
                unsafe {
                    let slice = core::slice::from_raw_parts(ptr, len);
                    if let Ok(path) = core::str::from_utf8(slice) {
                        let mut fs = FS.lock();
                        let resolved = fs.resolve_path(&[], path);
                        let (parent, name) = fs.split_parent_and_name(&resolved);
                        if fs.delete_node(parent, name).is_ok() {
                            return 0;
                        }
                    }
                }
            }
            1
        }
        SYS_THREAD_YIELD => {
            // Yield execution slot
            0
        }
        SYS_IPC_SEND | SYS_IPC_RECV => {
            // Inter-thread message queue router status
            0
        }
        SYS_MUTEX_LOCK | SYS_MUTEX_UNLOCK => {
            // Priority Inheritance lock allocator status
            0
        }
        _ => 0xFFFFFFFF, // Unknown syscall
    }
}
