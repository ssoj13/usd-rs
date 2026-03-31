// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Virtual memory management.
//!
//! Provides platform-independent functions for querying and modifying virtual memory
//! properties like protection flags and page residency.

use std::io;

/// Memory protection flags.
///
/// Defines access permissions for memory pages. These flags map to platform-specific
/// protection modes:
///
/// - **Windows**: PAGE_NOACCESS, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY
/// - **Unix**: PROT_NONE, PROT_READ, PROT_READ|PROT_WRITE, PROT_READ|PROT_WRITE
///
/// # Platform Notes
///
/// On POSIX systems, `ReadWrite` and `ReadWriteCopy` are identical.
/// On Windows, they differ: `ReadWrite` is for shared mappings, while
/// `ReadWriteCopy` triggers copy-on-write for private file-backed mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum MemoryProtection {
    /// No access allowed (page faults on any access)
    None = 0,
    /// Read-only access
    ReadOnly = 1,
    /// Read and write access
    ReadWrite = 2,
    /// Read, write, copy-on-write access.
    ///
    /// On Windows, maps to `PAGE_WRITECOPY`.
    /// On Unix, identical to `ReadWrite` (COW is handled at mmap level).
    /// Value 3 matches C++ ArchMemoryProtection enum.
    ReadWriteCopy = 3,
}

impl MemoryProtection {
    /// Returns the platform-specific protection flags.
    #[cfg(windows)]
    #[must_use]
    const fn to_platform_flags(self) -> u32 {
        use windows_sys::Win32::System::Memory::*;
        match self {
            Self::None => PAGE_NOACCESS,
            Self::ReadOnly => PAGE_READONLY,
            Self::ReadWrite => PAGE_READWRITE,
            Self::ReadWriteCopy => PAGE_WRITECOPY,
        }
    }

    #[cfg(unix)]
    #[must_use]
    const fn to_platform_flags(self) -> i32 {
        match self {
            Self::None => libc::PROT_NONE,
            Self::ReadOnly => libc::PROT_READ,
            Self::ReadWrite | Self::ReadWriteCopy => libc::PROT_READ | libc::PROT_WRITE,
        }
    }
}

/// Rounds an address down to the nearest page boundary.
///
/// # Safety
///
/// The caller must ensure the returned pointer is used correctly.
#[inline]
unsafe fn round_to_page_addr(addr: *const u8) -> *mut u8 {
    let page_size = super::get_page_size();
    let page_mask = !(page_size - 1);
    let addr_int = addr as usize;
    (addr_int & page_mask) as *mut u8
}

/// Sets memory protection for a range of pages.
///
/// Changes the protection flags for pages containing the range `[start, start + num_bytes)`.
/// The `start` address is automatically rounded down to the nearest page boundary.
///
/// # Arguments
///
/// * `start` - Starting address (will be rounded to page boundary)
/// * `num_bytes` - Number of bytes to protect
/// * `protection` - New protection flags to apply
///
/// # Errors
///
/// Returns an error if the system call fails. Common causes:
/// - Invalid address range
/// - Insufficient permissions
/// - Address not mapped
///
/// # Examples
///
/// ```no_run
/// use usd_arch::{set_memory_protection, MemoryProtection};
///
/// # unsafe {
/// let mut data = vec![0u8; 4096];
/// let ptr = data.as_mut_ptr();
///
/// // Make the page read-only
/// set_memory_protection(ptr, 4096, MemoryProtection::ReadOnly)?;
///
/// // Restore read-write access
/// set_memory_protection(ptr, 4096, MemoryProtection::ReadWrite)?;
/// # }
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// # Platform Notes
///
/// - **Windows**: Uses `VirtualProtect`
/// - **Unix**: Uses `mprotect`
///
/// # Safety
///
/// This function is unsafe because:
/// - The caller must ensure `start` points to valid mapped memory
/// - Changing protection on code pages can cause undefined behavior
/// - Multiple threads accessing the same pages may observe inconsistent state
pub unsafe fn set_memory_protection(
    start: *const u8,
    num_bytes: usize,
    protection: MemoryProtection,
) -> io::Result<()> {
    unsafe {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::VirtualProtect;

            let page_start = round_to_page_addr(start);
            let offset = (start as usize) - (page_start as usize);
            let len = num_bytes + offset;
            let prot_flags = protection.to_platform_flags();

            let mut old_protect: u32 = 0;
            let result = VirtualProtect(page_start as *const _, len, prot_flags, &mut old_protect);

            if result == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }

        #[cfg(unix)]
        {
            let page_start = round_to_page_addr(start);
            let offset = (start as usize) - (page_start as usize);
            let len = num_bytes + offset;
            let prot_flags = protection.to_platform_flags();

            let result = libc::mprotect(page_start as *mut _, len, prot_flags);

            if result == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        }

        #[cfg(not(any(windows, unix)))]
        {
            let _ = (start, num_bytes, protection);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Memory protection not supported on this platform",
            ))
        }
    }
}

/// Queries which memory pages are resident in RAM.
///
/// For a given memory range, determines which pages are currently resident (in physical RAM)
/// versus swapped out. Resident pages won't cause page faults on access, while non-resident
/// pages will.
///
/// # Arguments
///
/// * `addr` - Starting address (must be page-aligned)
/// * `len` - Length in bytes (will be rounded up to page size)
/// * `page_map` - Output buffer to receive residency info (0 = not resident, 1 = resident)
///
/// # Errors
///
/// Returns an error if:
/// - `addr` is not page-aligned
/// - The address range is not mapped
/// - The system call fails
///
/// # Examples
///
/// ```ignore
/// use usd_arch::virtual_memory::{query_mapped_memory_residency, get_page_size};
///
/// # unsafe {
/// let data = vec![0u8; 8192];
/// let ptr = data.as_ptr();
/// let page_size = get_page_size();
/// let num_pages = (8192 + page_size - 1) / page_size;
///
/// let mut residency = vec![0u8; num_pages];
/// query_mapped_memory_residency(ptr, 8192, &mut residency)?;
///
/// for (i, &resident) in residency.iter().enumerate() {
///     println!("Page {}: {}", i, if resident != 0 { "resident" } else { "swapped" });
/// }
/// # }
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// # Platform Notes
///
/// - **Linux**: Uses `mincore` with `unsigned char *` for vec
/// - **macOS**: Uses `mincore` with `char *` for vec (caddr_t for addr)
/// - **Windows**: Uses `QueryWorkingSetEx` with `PSAPI_WORKING_SET_EX_INFORMATION`
///
/// # Safety
///
/// This function is unsafe because:
/// - `addr` must point to valid mapped memory
/// - `addr` must be page-aligned
/// - `page_map` must have sufficient capacity: `(len + page_size - 1) / page_size` bytes
#[allow(dead_code)]
pub unsafe fn query_mapped_memory_residency(
    addr: *const u8,
    len: usize,
    page_map: &mut [u8],
) -> io::Result<()> {
    unsafe {
        #[cfg(target_os = "linux")]
        {
            let result = libc::mincore(addr as *mut _, len, page_map.as_mut_ptr());
            if result == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS mincore takes *const c_void for addr
            let result = libc::mincore(
                addr as *const libc::c_void,
                len,
                page_map.as_mut_ptr() as *mut libc::c_char,
            );
            if result == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::System::ProcessStatus::*;
            use windows_sys::Win32::System::Threading::GetCurrentProcess;

            let page_size = super::get_page_size();
            let num_pages = len.div_ceil(page_size);

            // QueryWorkingSetEx requires array of PSAPI_WORKING_SET_EX_INFORMATION
            let mut info_vec: Vec<PSAPI_WORKING_SET_EX_INFORMATION> = Vec::with_capacity(num_pages);

            for i in 0..num_pages {
                let page_addr = (addr as usize + i * page_size) as *mut _;
                info_vec.push(PSAPI_WORKING_SET_EX_INFORMATION {
                    VirtualAddress: page_addr,
                    VirtualAttributes: std::mem::zeroed(),
                });
            }

            let result = K32QueryWorkingSetEx(
                GetCurrentProcess(),
                info_vec.as_mut_ptr() as *mut _,
                (num_pages * std::mem::size_of::<PSAPI_WORKING_SET_EX_INFORMATION>()) as u32,
            );

            if result == 0 {
                return Err(io::Error::last_os_error());
            }

            // Extract Valid bit from VirtualAttributes
            for (i, info) in info_vec.iter().enumerate() {
                // The Valid bit is the LSB of the VirtualAttributes union
                let valid = info.VirtualAttributes.Flags & 1;
                page_map[i] = if valid != 0 { 1 } else { 0 };
            }

            Ok(())
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            let _ = (addr, len);
            // Fill with 0s to indicate unknown/not resident
            page_map.iter_mut().for_each(|b| *b = 0);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Memory residency query not supported on this platform",
            ))
        }
    }
}

/// Returns the system's virtual memory page size in bytes.
///
/// This is an alias for [`get_page_size()`](crate::get_page_size).
/// The page size is always a power of two and typically 4096 bytes on most systems.
///
/// # Examples
///
/// ```
/// use usd_arch::get_vm_page_size;
///
/// let page_size = get_vm_page_size();
/// assert!(page_size.is_power_of_two());
/// ```
#[inline]
#[must_use]
pub fn get_vm_page_size() -> usize {
    super::get_page_size()
}

/// Reserves a contiguous region of virtual address space without committing physical memory.
///
/// The reserved pages are inaccessible until committed via [`commit_virtual_memory`].
/// Matches C++ `ArchReserveVirtualMemory`.
///
/// # Safety
///
/// The returned pointer must be freed with [`free_virtual_memory`].
pub unsafe fn reserve_virtual_memory(num_bytes: usize) -> io::Result<*mut u8> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Memory::{MEM_RESERVE, PAGE_NOACCESS, VirtualAlloc};
        let ptr = VirtualAlloc(std::ptr::null(), num_bytes, MEM_RESERVE, PAGE_NOACCESS);
        if ptr.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(ptr as *mut u8)
        }
    }

    #[cfg(unix)]
    unsafe {
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            num_bytes,
            libc::PROT_NONE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        // C++ checks both null and MAP_FAILED
        if ptr.is_null() || ptr == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(ptr as *mut u8)
        }
    }

    #[cfg(not(any(windows, unix)))]
    {
        let _ = num_bytes;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Virtual memory reservation not supported on this platform",
        ))
    }
}

/// Commits a range within a previously reserved virtual memory region.
///
/// Makes the specified range accessible with read-write permissions.
/// Matches C++ `ArchCommitVirtualMemoryRange`.
///
/// # Safety
///
/// * `start` must point into a region obtained from [`reserve_virtual_memory`]
/// * The range `[start, start + num_bytes)` must not exceed the reserved region
pub unsafe fn commit_virtual_memory(start: *mut u8, num_bytes: usize) -> io::Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Memory::{MEM_COMMIT, PAGE_READWRITE, VirtualAlloc};
        let ptr = VirtualAlloc(start as *const _, num_bytes, MEM_COMMIT, PAGE_READWRITE);
        if ptr.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(unix)]
    unsafe {
        // Round start down to page boundary and extend num_bytes to cover the full range,
        // matching C++ ArchCommitVirtualMemoryRange which calls RoundToPageAddr(start).
        let page_start = round_to_page_addr(start);
        let offset = (start as usize) - (page_start as usize);
        let len = num_bytes + offset;

        let result = libc::mprotect(
            page_start as *mut _,
            len,
            libc::PROT_READ | libc::PROT_WRITE,
        );
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(not(any(windows, unix)))]
    {
        let _ = (start, num_bytes);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Virtual memory commit not supported on this platform",
        ))
    }
}

/// Releases a virtual memory region previously obtained from [`reserve_virtual_memory`].
///
/// Matches C++ `ArchFreeVirtualMemory`.
///
/// # Safety
///
/// * `start` must be the exact pointer returned by [`reserve_virtual_memory`]
/// * `num_bytes` must be the same size that was reserved
/// * The memory must not be accessed after this call
pub unsafe fn free_virtual_memory(start: *mut u8, num_bytes: usize) -> io::Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Memory::{MEM_RELEASE, VirtualFree};
        // VirtualFree with MEM_RELEASE requires dwSize=0
        let _ = num_bytes;
        let result = VirtualFree(start as *mut _, 0, MEM_RELEASE);
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(unix)]
    unsafe {
        let result = libc::munmap(start as *mut _, num_bytes);
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(not(any(windows, unix)))]
    {
        let _ = (start, num_bytes);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Virtual memory free not supported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_protection_values() {
        // Values must match C++ ArchMemoryProtection: NoAccess=0, ReadOnly=1, ReadWrite=2, ReadWriteCopy=3
        assert_eq!(MemoryProtection::None as u32, 0);
        assert_eq!(MemoryProtection::ReadOnly as u32, 1);
        assert_eq!(MemoryProtection::ReadWrite as u32, 2);
        assert_eq!(MemoryProtection::ReadWriteCopy as u32, 3);
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn test_reserve_commit_free() {
        let page_size = get_vm_page_size();
        let size = page_size * 4;

        unsafe {
            // Reserve address space
            let ptr = reserve_virtual_memory(size).expect("reserve failed");
            assert!(!ptr.is_null());

            // Commit first two pages
            commit_virtual_memory(ptr, page_size * 2).expect("commit failed");

            // Write to committed memory
            std::ptr::write(ptr, 42u8);
            std::ptr::write(ptr.add(page_size), 99u8);
            assert_eq!(std::ptr::read(ptr), 42);
            assert_eq!(std::ptr::read(ptr.add(page_size)), 99);

            // Free the entire reservation
            free_virtual_memory(ptr, size).expect("free failed");
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_read_write_copy_flag() {
        use windows_sys::Win32::System::Memory::PAGE_WRITECOPY;
        assert_eq!(
            MemoryProtection::ReadWriteCopy.to_platform_flags(),
            PAGE_WRITECOPY
        );
    }

    #[test]
    fn test_get_vm_page_size() {
        let page_size = get_vm_page_size();
        assert!(page_size > 0);
        assert!(page_size.is_power_of_two());
        assert_eq!(page_size, super::super::get_page_size());
    }

    #[test]
    fn test_round_to_page_addr() {
        let page_size = get_vm_page_size();

        unsafe {
            let addr = 0x1000 as *const u8;
            let rounded = round_to_page_addr(addr);
            assert_eq!(rounded as usize % page_size, 0);

            let addr = (page_size + 1) as *const u8;
            let rounded = round_to_page_addr(addr);
            assert_eq!(rounded as usize, page_size);
        }
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn test_set_memory_protection_basic() {
        // Allocate a page-aligned buffer
        let page_size = get_vm_page_size();
        let mut data = vec![42u8; page_size * 2];
        let ptr = data.as_mut_ptr();

        unsafe {
            // Set to read-only
            let result = set_memory_protection(ptr, page_size, MemoryProtection::ReadOnly);
            assert!(
                result.is_ok(),
                "Failed to set read-only: {:?}",
                result.err()
            );

            // Restore to read-write
            let result = set_memory_protection(ptr, page_size, MemoryProtection::ReadWrite);
            assert!(
                result.is_ok(),
                "Failed to restore read-write: {:?}",
                result.err()
            );

            // Verify we can still write
            data[0] = 99;
            assert_eq!(data[0], 99);
        }
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn test_set_memory_protection_unaligned() {
        let page_size = get_vm_page_size();
        let mut data = vec![0u8; page_size * 2];

        // Use unaligned pointer
        let ptr = unsafe { data.as_mut_ptr().add(10) };

        unsafe {
            // Should still work - will be rounded to page boundary
            let result = set_memory_protection(ptr, 100, MemoryProtection::ReadOnly);
            assert!(result.is_ok());

            let result = set_memory_protection(ptr, 100, MemoryProtection::ReadWrite);
            assert!(result.is_ok());
        }
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn test_query_mapped_memory_residency() {
        let page_size = get_vm_page_size();
        let data = vec![42u8; page_size * 3];
        let ptr = data.as_ptr();

        // Touch the data to ensure it's resident
        let _ = data[0];
        let _ = data[page_size];
        let _ = data[page_size * 2];

        unsafe {
            // Align pointer to page boundary
            let aligned_ptr = round_to_page_addr(ptr);
            let num_pages = 3;
            let mut residency = vec![0u8; num_pages];

            let result =
                query_mapped_memory_residency(aligned_ptr, page_size * num_pages, &mut residency);

            assert!(result.is_ok(), "Query failed: {:?}", result.err());

            // After touching the memory, pages should be resident
            // (though this isn't guaranteed on all systems)
            println!("Residency: {:?}", residency);
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_query_mapped_memory_residency_windows() {
        let page_size = get_vm_page_size();
        let data = vec![42u8; page_size * 2];
        let ptr = data.as_ptr();

        // Touch to make resident
        let _ = data[0];

        unsafe {
            let aligned_ptr = round_to_page_addr(ptr);
            let num_pages = 2;
            let mut residency = vec![0u8; num_pages];

            let result =
                query_mapped_memory_residency(aligned_ptr, page_size * num_pages, &mut residency);

            assert!(result.is_ok(), "Query failed: {:?}", result.err());
            println!("Windows residency: {:?}", residency);
        }
    }

    #[test]
    fn test_memory_protection_roundtrip() {
        let prot = MemoryProtection::ReadWrite;
        let flags = prot.to_platform_flags();

        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::PAGE_READWRITE;
            assert_eq!(flags, PAGE_READWRITE);
        }

        #[cfg(unix)]
        {
            assert_eq!(flags, libc::PROT_READ | libc::PROT_WRITE);
        }
    }
}
