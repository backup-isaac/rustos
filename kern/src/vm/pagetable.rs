use core::iter::Chain;
use core::ops::{Deref, DerefMut};
use core::slice::Iter;
use core::fmt::Formatter;

use alloc::boxed::Box;
use alloc::fmt;
use core::alloc::{GlobalAlloc, Layout};

use crate::allocator;
use crate::param::*;
use crate::vm::{PhysicalAddr, VirtualAddr};
use crate::ALLOCATOR;

use aarch64::vmsa::*;
use shim::const_assert_size;

#[repr(C)]
pub struct Page([u8; PAGE_SIZE]);
const_assert_size!(Page, PAGE_SIZE);

impl Page {
    pub const SIZE: usize = PAGE_SIZE;
    pub const ALIGN: usize = PAGE_SIZE;

    fn layout() -> Layout {
        unsafe { Layout::from_size_align_unchecked(Self::SIZE, Self::ALIGN) }
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L2PageTable {
    pub entries: [RawL2Entry; 8192],
}
const_assert_size!(L2PageTable, PAGE_SIZE);

impl L2PageTable {
    /// Returns a new `L2PageTable`
    fn new() -> L2PageTable {
        L2PageTable {
            entries: [RawL2Entry::new(0); 8192],
        }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        PhysicalAddr::from(self as *const L2PageTable)
    }
}

impl fmt::Debug for L2PageTable {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        self.entries[..].fmt(fmt)
    }
}

#[derive(Copy, Clone)]
pub struct L3Entry(RawL3Entry);

impl L3Entry {
    /// Returns a new `L3Entry`.
    fn new() -> L3Entry {
        L3Entry(RawL3Entry::new(0))
    }

    /// Returns `true` if the L3Entry is valid and `false` otherwise.
    fn is_valid(&self) -> bool {
        self.0.get_masked(RawL3Entry::VALID) != 0
    }

    /// Extracts `ADDR` field of the L3Entry and returns as a `PhysicalAddr`
    /// if valid. Otherwise, return `None`.
    fn get_page_addr(&self) -> Option<PhysicalAddr> {
        if self.is_valid() {
            Some(PhysicalAddr::from(self.0.get_masked(RawL3Entry::ADDR)))
        } else {
            None
        }
    }
}

impl fmt::Debug for L3Entry {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(fmt)
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L3PageTable {
    pub entries: [L3Entry; 8192],
}
const_assert_size!(L3PageTable, PAGE_SIZE);

impl L3PageTable {
    /// Returns a new `L3PageTable`.
    fn new() -> L3PageTable {
        L3PageTable {
            entries: [L3Entry::new(); 8192],
        }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        PhysicalAddr::from(self as *const L3PageTable)
    }
}

impl fmt::Debug for L3PageTable {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        self.entries[..].fmt(fmt)
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct PageTable {
    pub l2: L2PageTable,
    pub l3: [L3PageTable; 2],
}

impl PageTable {
    /// Returns a new `Box` containing `PageTable`.
    /// Entries in L2PageTable should be initialized properly before return.
    fn new(perm: u64) -> Box<PageTable> {
        let mut b = Box::new(PageTable {
            l2: L2PageTable::new(),
            l3: [L3PageTable::new(), L3PageTable::new()],
        });
        for i in 0..b.l3.len() {
            b.l2.entries[i]
                .set_value(EntryValid::Valid, RawL2Entry::VALID)
                .set_value(EntryType::Table, RawL2Entry::TYPE)
                .set_value(perm, RawL2Entry::AP)
                .set_value(EntrySh::ISh, RawL2Entry::SH)
                .set_value(EntryAttr::Mem, RawL2Entry::ATTR)
                .set_bit(RawL2Entry::AF)
                .set_masked(b.l3[i].as_ptr().as_u64(), RawL2Entry::ADDR);
        }
        b
    }

    /// Returns the (L2index, L3index) extracted from the given virtual address.
    /// Since we are only supporting 1GB virtual memory in this system, L2index
    /// should be smaller than 2.
    ///
    /// # Panics
    ///
    /// Panics if the virtual address is not properly aligned to page size.
    /// Panics if extracted L2index exceeds the number of L3PageTable.
    fn locate(va: VirtualAddr) -> (usize, usize) {
        let l2 = (va.as_usize() & (1 << 29)) >> 29;
        let l3 = (va.as_usize() & (0x1fff << 16)) >> 16;
        (l2, l3)
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is valid.
    /// Otherwise, `false` is returned.
    pub fn is_valid(&self, va: VirtualAddr) -> bool {
        let (l2, l3) = PageTable::locate(va);
        let l2_entry = self.l2.entries[l2];
        if l2_entry.get_masked(RawL2Entry::VALID) == 0 {
            return false;
        }
        let l3_address = l2_entry.get_masked(RawL2Entry::ADDR) as usize;
        let l3_index = (l3_address - self.l3[0].as_ptr().as_usize()) / PAGE_SIZE;
        let l3_entry = self.l3[l3_index].entries[l3];
        l3_entry.is_valid()
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is invalid.
    /// Otherwise, `false` is returned.
    pub fn is_invalid(&self, va: VirtualAddr) -> bool {
        !self.is_valid(va)
    }

    /// Set the given RawL3Entry `entry` to the L3Entry indicated by the given virtual
    /// address.
    pub fn set_entry(&mut self, va: VirtualAddr, entry: RawL3Entry) -> &mut Self {
        let (l2, l3) = PageTable::locate(va);
        let l3_address = self.l2.entries[l2].get_masked(RawL2Entry::ADDR) as usize;
        let l3_index = (l3_address - self.l3[0].as_ptr().as_usize()) / PAGE_SIZE;
        self.l3[l3_index].entries[l3] = L3Entry(entry);
        self
    }

    /// Returns a base address of the pagetable. The returned `PhysicalAddr` value
    /// will point the start address of the L2PageTable.
    pub fn get_baddr(&self) -> PhysicalAddr {
        self.l2.as_ptr()
    }
}

impl fmt::Debug for PageTable {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        fmt.debug_struct("PageTable")
            .field("l2", &self.l2)
            .field("l3", &self.l3)
            .finish()
    }
}

impl<'a> IntoIterator for &'a PageTable {
    type Item = &'a L3Entry;
    type IntoIter = Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>>;

    fn into_iter(self) -> Self::IntoIter {
        self.l3[0].entries.iter().chain(self.l3[1].entries.iter())
    }
}

pub struct KernPageTable(Box<PageTable>);

impl KernPageTable {
    /// Returns a new `KernPageTable`. `KernPageTable` should have a `Pagetable`
    /// created with `KERN_RW` permission.
    ///
    /// Set L3entry of ARM physical address starting at 0x00000000 for RAM and
    /// physical address range from `IO_BASE` to `IO_BASE_END` for peripherals.
    /// Each L3 entry should have correct value for lower attributes[10:0] as well
    /// as address[47:16]. Refer to the definition of `RawL3Entry` in `vmsa.rs` for
    /// more details.
    pub fn new() -> KernPageTable {
        let mut kpt = KernPageTable(PageTable::new(EntryPerm::KERN_RW));
        if let Some((_, end)) = allocator::memory_map() {
            let mut addr = 0;
            while addr < end {
                let mut entry = RawL3Entry::new(0);
                entry
                    .set_value(EntryValid::Valid, RawL3Entry::VALID)
                    .set_value(PageType::Page, RawL3Entry::TYPE)
                    .set_value(EntryAttr::Mem, RawL3Entry::ATTR)
                    .set_value(EntryPerm::KERN_RW, RawL3Entry::AP)
                    .set_masked(addr as u64, RawL3Entry::ADDR)
                    .set_value(EntrySh::ISh, RawL3Entry::SH)
                    .set_bit(RawL3Entry::AF);
                kpt.set_entry(addr.into(), entry);
                addr += PAGE_SIZE;
            }
            addr = IO_BASE;
            while addr < IO_BASE_END {
                let mut entry = RawL3Entry::new(0);
                entry
                    .set_value(EntryValid::Valid, RawL3Entry::VALID)
                    .set_value(PageType::Page, RawL3Entry::TYPE)
                    .set_value(EntryAttr::Dev, RawL3Entry::ATTR)
                    .set_value(EntryPerm::KERN_RW, RawL3Entry::AP)
                    .set_masked(addr as u64, RawL3Entry::ADDR)
                    .set_value(EntrySh::OSh, RawL3Entry::SH)
                    .set_bit(RawL3Entry::AF);
                kpt.set_entry(addr.into(), entry);
                addr += PAGE_SIZE;
            }
        } else {
            panic!("could not map memory");
        }
        kpt
    }
}

pub enum PagePerm {
    RW,
    RO,
    RWX,
}

pub struct UserPageTable(Box<PageTable>);

impl UserPageTable {
    /// Returns a new `UserPageTable` containing a `PageTable` created with
    /// `USER_RW` permission.
    pub fn new() -> UserPageTable {
        UserPageTable(PageTable::new(EntryPerm::USER_RW))
    }

    /// Allocates a page and set an L3 entry translates given virtual address to the
    /// physical address of the allocated page. Returns the allocated page.
    ///
    /// # Panics
    /// Panics if the virtual address is lower than `USER_IMG_BASE`.
    /// Panics if the virtual address has already been allocated.
    /// Panics if allocator fails to allocate a page.
    ///
    /// TODO. use Result<T> and make it failurable
    /// TODO. use perm properly
    pub fn alloc(&mut self, va: VirtualAddr, _perm: PagePerm) -> &mut [u8] {
        if va.as_usize() < USER_IMG_BASE {
            panic!("invalid virtual address {:?}", va);
        }
        if self.0.is_valid(va) {
            panic!("address {:?} already allocated", va);
        }
        let ptr = unsafe { ALLOCATOR.alloc(Page::layout()) };
        if ptr == core::ptr::null_mut() {
            panic!("could not allocate page");
        }
        let mut entry = RawL3Entry::new(0);
        entry
            .set_value(EntryValid::Valid, RawL3Entry::VALID)
            .set_value(PageType::Page, RawL3Entry::TYPE)
            .set_value(EntryAttr::Mem, RawL3Entry::ATTR)
            .set_value(EntryPerm::USER_RW, RawL3Entry::AP)
            .set_masked(ptr as u64, RawL3Entry::ADDR)
            .set_value(EntrySh::ISh, RawL3Entry::SH)
            .set_bit(RawL3Entry::AF);
        self.set_entry(va, entry);

        unsafe {
            core::slice::from_raw_parts_mut(ptr, PAGE_SIZE)
        }
    }
}

impl fmt::Debug for UserPageTable {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        fmt.debug_struct("UserPageTable")
            .field("l2", &self.0.l2)
            .field("l3", &self.0.l3)
            .finish()
    }
}

impl Drop for UserPageTable {
    fn drop(&mut self) {
        for page_addr in self.into_iter() {
            if let Some(mut phys) = page_addr.get_page_addr() {
                unsafe {
                    ALLOCATOR.dealloc(phys.as_mut_ptr(), Page::layout())
                };
            }
        }
    }
}

impl Deref for KernPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for UserPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KernPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DerefMut for UserPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
