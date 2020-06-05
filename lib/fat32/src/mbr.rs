use core::fmt;
use core::mem;
use shim::const_assert_size;
use shim::io;

use crate::traits::BlockDevice;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CHS {
    head: u8,
    sector: u8,
    cylinder: u8,
}

impl fmt::Debug for CHS {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_struct("CHS")
            .field("head", &self.head)
            .field("sector", &self.sector)
            .field("cylinder", &self.cylinder)
            .finish()
    }
}
const_assert_size!(CHS, 3);

#[repr(C, packed)]
pub struct PartitionEntry {
    boot_indicator: u8,
    start: CHS,
    pub partition_type: u8,
    end: CHS,
    pub sector_offset: u32,
    pub num_sectors: u32,
}

impl fmt::Debug for PartitionEntry {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_struct("PartitionEntry")
            .field("boot_indicator", &self.boot_indicator)
            .field("start", &self.start)
            .field("partition_type", &self.partition_type)
            .field("end", &self.end)
            .field("sector_offset", &{ self.sector_offset })
            .field("num_sectors", &{ self.num_sectors })
            .finish()
    }
}

const_assert_size!(PartitionEntry, 16);

/// The master boot record (MBR).
#[repr(C, packed)]
pub struct MasterBootRecord {
    bootstrap: [u8; 436],
    pub disk_id: [u8; 10],
    pub partition_table: [PartitionEntry; 4],
    signature: u16,
}

impl fmt::Debug for MasterBootRecord {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt.debug_struct("MasterBootRecord")
            .field("disk_id", &self.disk_id)
            .field("partition_table", &self.partition_table)
            .field("signature", &{ self.signature })
            .finish()
    }
}

const_assert_size!(MasterBootRecord, 512);

#[derive(Debug)]
pub enum Error {
    /// There was an I/O error while reading the MBR.
    Io(io::Error),
    /// Partiion `.0` (0-indexed) contains an invalid or unknown boot indicator.
    UnknownBootIndicator(u8),
    /// The MBR magic signature was invalid.
    BadSignature,
}

impl MasterBootRecord {
    /// Reads and returns the master boot record (MBR) from `device`.
    ///
    /// # Errors
    ///
    /// Returns `BadSignature` if the MBR contains an invalid magic signature.
    /// Returns `UnknownBootIndicator(n)` if partition `n` contains an invalid
    /// boot indicator. Returns `Io(err)` if the I/O error `err` occured while
    /// reading the MBR.
    pub fn from<T: BlockDevice>(mut device: T) -> Result<MasterBootRecord, Error> {
        let mut buf = [0; 512];
        match device.read_sector(0, &mut buf) {
            Ok(_) => {}
            Err(e) => return Err(Error::Io(e))
        }
        let mbr: MasterBootRecord = unsafe { mem::transmute(buf) };
        for i in 0..mbr.partition_table.len() {
            if mbr.partition_table[i].boot_indicator & 0x7f != 0 {
                return Err(Error::UnknownBootIndicator(i as u8));
            }
        }
        if mbr.signature != 0xaa55 {
            return Err(Error::BadSignature);
        }
        Ok(mbr)
    }
}
