use core::fmt;

use shim::const_assert_size;

use crate::traits;

/// A date as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Date(u16);

/// Time as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Time(u16);

/// File attributes as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Attributes(u8);

/// A structure containing a date and time.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub date: Date,
    pub time: Time,
}

/// Metadata for a directory entry.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone)]
pub struct Metadata {
    attributes: Attributes,
    reserved: u8,
    creation_time_tenths_s: u8,
    created_time: Time,
    created_date: Date,
    accessed_date: Date,
    first_cluster_high: u16,
    modified_time: Time,
    modified_date: Date,
    first_cluster_low: u16,
}

const_assert_size!(Metadata, 17);

impl Metadata {
    pub fn first_cluster(&self) -> u32 {
        self.first_cluster_low as u32 | (self.first_cluster_high as u32) << 16
    }

    pub fn is_dir(&self) -> bool {
        self.attributes.0 & 0x10 != 0
    }

    pub fn is_system(&self) -> bool {
        self.attributes.0 & 0x4 != 0
    }

    pub fn is_volume_id(&self) -> bool {
        self.attributes.0 & 0x8 != 0
    }

    pub fn is_archive(&self) -> bool {
        self.attributes.0 & 0x20 != 0
    }
}

impl traits::Timestamp for Timestamp {
    fn year(&self) -> usize {
        (((self.date.0 & (0x7f << 9)) >> 9) + 1980) as usize
    }

    fn month(&self) -> u8 {
        ((self.date.0 & (0xf << 5)) >> 5) as u8
    }

    fn day(&self) -> u8 {
        (self.date.0 & 0x1f) as u8
    }

    fn hour(&self) -> u8 {
        ((self.time.0 & (0x1f << 11)) >> 11) as u8
    }

    fn minute(&self) -> u8 {
        ((self.time.0 & (0x3f << 5)) >> 5) as u8
    }

    fn second(&self) -> u8 {
        ((self.time.0 & 0x1f) * 2) as u8
    }
}

impl traits::Metadata for Metadata {
    type Timestamp = Timestamp;
    fn read_only(&self) -> bool {
        self.attributes.0 & 0x1 == 0x1
    }

    fn hidden(&self) -> bool {
        self.attributes.0 & 0x2 == 0x2
    }

    fn created(&self) -> Self::Timestamp {
        Timestamp {
            time: self.created_time,
            date: self.created_date,
        }
    }

    fn accessed(&self) -> Self::Timestamp {
        Timestamp {
            time: Time(0),
            date: self.accessed_date,
        }
    }

    fn modified(&self) -> Self::Timestamp {
        Timestamp {
            time: self.modified_time,
            date: self.modified_date,
        }
    }
}

impl fmt::Display for Metadata {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt.debug_struct("Metadata")
            .field("attributes", &self.attributes)
            .field("creation_time_tenths_s", &self.creation_time_tenths_s)
            .field("created_time", &self.created_time)
            .field("created_date", &self.created_date)
            .field("accessed_date", &self.accessed_date)
            .field("first_cluster_high", &{ self.first_cluster_high })
            .field("modified_time", &self.modified_time)
            .field("modified_date", &self.modified_date)
            .field("first_cluster_low", &{ self.first_cluster_low })
            .finish()
    }
}