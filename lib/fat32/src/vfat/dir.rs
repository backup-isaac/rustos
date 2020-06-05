use alloc::string::String;
use alloc::vec::Vec;

use core::char::{decode_utf16, REPLACEMENT_CHARACTER};

use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io;
use shim::newioerr;

use crate::traits;
use crate::util::VecExt;
use crate::vfat::{Attributes, Metadata};
use crate::vfat::{Cluster, Entry, File, VFatHandle};

#[derive(Debug)]
pub struct Dir<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub first_cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct VFatRegularDirEntry {
    file_name: [u8; 8],
    file_extension: [u8; 3],
    metadata: Metadata,
    file_size: u32,
}

const_assert_size!(VFatRegularDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct VFatLfnDirEntry {
    sequence_number: u8,
    first_name_chars: [u16; 5],
    attributes: Attributes,
    lfn_type: u8,
    checksum: u8,
    second_name_chars: [u16; 6],
    always_zero: u16,
    third_name_chars: [u16; 2],
}

const_assert_size!(VFatLfnDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatUnknownDirEntry([u8; 32]);

const_assert_size!(VFatUnknownDirEntry, 32);

pub union VFatDirEntry {
    unknown: VFatUnknownDirEntry,
    regular: VFatRegularDirEntry,
    long_filename: VFatLfnDirEntry,
}

impl<HANDLE: VFatHandle> Dir<HANDLE> {
    /// Finds the entry named `name` in `self` and returns it. Comparison is
    /// case-insensitive.
    ///
    /// # Errors
    ///
    /// If no entry with name `name` exists in `self`, an error of `NotFound` is
    /// returned.
    ///
    /// If `name` contains invalid UTF-8 characters, an error of `InvalidInput`
    /// is returned.
    pub fn find<P: AsRef<OsStr>>(&self, name: P) -> io::Result<Entry<HANDLE>> {
        if let Some(utf8) = name.as_ref().to_str() {
            use crate::traits::{Dir, Entry};
            for entry in self.entries()? {
                if entry.name().eq_ignore_ascii_case(utf8) {
                    return Ok(entry);
                }
            }
        } else {
            return Err(newioerr!(InvalidInput, "invalid UTF-8"));
        }
        Err(newioerr!(NotFound, "file not found"))
    }
}

pub struct EntryIterator<HANDLE: VFatHandle> {
    vfat: HANDLE,
    entries: Vec<VFatDirEntry>,
    curr: usize,
}

impl<HANDLE: VFatHandle> Iterator for EntryIterator<HANDLE> {
    type Item = Entry<HANDLE>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut is_lfn = true;
        let mut long_file_name = Vec::new();
        let mut long_file_pieces = Vec::new();
        while is_lfn {
            if self.curr >= self.entries.len() {
                return None;
            }
            let unknown_entry = unsafe { self.entries[self.curr].unknown };
            if unknown_entry.0[0] == 0 {
                return None;
            }
            if unknown_entry.0[0] == 0xe5 {
                self.curr += 1;
                continue;
            }
            is_lfn = unknown_entry.0[11] == 0xf;
            if is_lfn {
                let mut utf16 = Vec::new();
                let lfn_entry = unsafe { self.entries[self.curr].long_filename };
                for ucs in { lfn_entry.first_name_chars }.iter() {
                    let ucs_char = *ucs;
                    if ucs_char != 0 && ucs_char != 0xffff {
                        utf16.push(ucs_char);
                    }
                }
                for ucs in { lfn_entry.second_name_chars }.iter() {
                    let ucs_char = *ucs;
                    if ucs_char != 0 && ucs_char != 0xffff {
                        utf16.push(ucs_char);
                    }
                }
                for ucs in { lfn_entry.third_name_chars }.iter() {
                    let ucs_char = *ucs;
                    if ucs_char != 0 && ucs_char != 0xffff {
                        utf16.push(ucs_char);
                    }
                }
                let sequence_number = lfn_entry.sequence_number;
                let mut insertion_index = 0;
                for i in 0..long_file_pieces.len() {
                    if sequence_number > long_file_pieces[i] {
                        insertion_index = i + 1;
                    }
                }
                long_file_pieces.insert(insertion_index, sequence_number);
                long_file_name.insert(insertion_index, utf16);
                self.curr += 1;
            }
        }
        let regular_entry = unsafe { self.entries[self.curr].regular };
        self.curr += 1;
        let cluster_num = regular_entry.metadata.first_cluster();
        let entry_name = if long_file_name.len() > 0 {
            let mut lfn = Vec::new();
            for piece in long_file_name {
                lfn.extend(piece);
            }
            decode_utf16(lfn).map(|r| r.unwrap_or(REPLACEMENT_CHARACTER)).collect::<String>()
        } else {
            let mut filename = Vec::new();
            for b in regular_entry.file_name.iter() {
                let byte = *b;
                if byte == 0 || byte == 0x20 {
                    break;
                }
                filename.push(byte);
            }
            if regular_entry.file_extension[0] != 0 && regular_entry.file_extension[0] != 0x20 {
                filename.push('.' as u8);
                for b in regular_entry.file_extension.iter() {
                    let byte = *b;
                    if byte == 0 || byte == 0x20 {
                        break;
                    }
                    filename.push(byte);
                }
            }
            match String::from_utf8(filename) {
                Ok(s) => s,
                Err(_) => {
                    return None;
                }
            }
        };
        if regular_entry.metadata.is_dir() {
            Some(Entry::Dir(Dir {
                vfat: self.vfat.clone(),
                first_cluster: Cluster::from(cluster_num),
                metadata: regular_entry.metadata,
                name: entry_name,
            }))
        } else {
            Some(Entry::File(File {
                vfat: self.vfat.clone(),
                metadata: regular_entry.metadata,
                name: entry_name,
                first_cluster: Cluster::from(cluster_num),
                seek_offset: 0,
                file_size: regular_entry.file_size as usize,
            }))
        }
    }
}

impl<HANDLE: VFatHandle> traits::Dir for Dir<HANDLE> {
    type Entry = Entry<HANDLE>;
    type Iter = EntryIterator<HANDLE>;
    fn entries(&self) -> io::Result<Self::Iter> {
        let mut entry_vec = Vec::new();
        self.vfat.lock(|vfat| vfat.read_chain(self.first_cluster, &mut entry_vec))?;
        Ok(EntryIterator {
            vfat: self.vfat.clone(),
            entries: unsafe { entry_vec.cast::<VFatDirEntry>() },
            curr: 0,
        })
    }
}
