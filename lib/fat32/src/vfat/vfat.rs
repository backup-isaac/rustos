use core::fmt::Debug;
use core::marker::PhantomData;

use alloc::vec::Vec;

use shim::io;
use shim::newioerr;
use shim::path::{Path,Component};

use crate::alloc::string::ToString;
use crate::mbr::MasterBootRecord;
use crate::traits::{BlockDevice, FileSystem};
use crate::util::SliceExt;
use crate::vfat::{BiosParameterBlock, CachedPartition, Partition};
use crate::vfat::{Cluster, Dir, Entry, Error, FatEntry, File, Status};

/// A generic trait that handles a critical section as a closure
pub trait VFatHandle: Clone + Debug + Send + Sync {
    fn new(val: VFat<Self>) -> Self;
    fn lock<R>(&self, f: impl FnOnce(&mut VFat<Self>) -> R) -> R;
}

#[derive(Debug)]
pub struct VFat<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    device: CachedPartition,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    sectors_per_fat: u32,
    fat_start_sector: u64,
    data_start_sector: u64,
    rootdir_cluster: Cluster,
}

impl<HANDLE: VFatHandle> VFat<HANDLE> {
    pub fn from<T>(mut device: T) -> Result<HANDLE, Error>
    where
        T: BlockDevice + 'static,
    {
        let mbr = MasterBootRecord::from(&mut device)?;
        let mut which_partition = 0;
        let mut found_partition = false;
        for i in 0..mbr.partition_table.len() {
            if mbr.partition_table[i].partition_type == 0xb || mbr.partition_table[i].partition_type == 0xc {
                which_partition = i;
                found_partition = true;
                break;
            }
        }
        if !found_partition {
            return Err(Error::NotFound);
        }
        let bpb_sector = mbr.partition_table[which_partition].sector_offset as u64;
        let bpb = BiosParameterBlock::from(&mut device, bpb_sector)?;
        let data_start = bpb.reserved_sectors as u64 + (bpb.fats as u64 * bpb.sectors_per_fat as u64);
        let fat = VFat {
            phantom: PhantomData,
            device: CachedPartition::new(device, Partition {
                start: bpb_sector,
                num_sectors: bpb.total_logical_sectors as u64,
                sector_size: bpb.bytes_per_sector as u64,
            }),
            bytes_per_sector: bpb.bytes_per_sector,
            sectors_per_cluster: bpb.sectors_per_cluster,
            sectors_per_fat: bpb.sectors_per_fat,
            fat_start_sector: bpb.reserved_sectors as u64,
            data_start_sector: data_start,
            rootdir_cluster: Cluster::from(bpb.root_directory_cluster),
        };
        Ok(HANDLE::new(fat))
    }

    pub fn get_cluster_size(&self) -> usize {
        self.bytes_per_sector as usize * self.sectors_per_cluster as usize
    }

    //
    //  * A method to read from an offset of a cluster into a buffer.
    //
    pub fn read_cluster(
        &mut self,
        cluster: Cluster,
        offset: usize,
        buf: &mut [u8]
    ) -> io::Result<usize> {
        let mut ctr = 0;
        let start_sector = offset / self.bytes_per_sector as usize;
        let mut sector_start_index = offset % self.bytes_per_sector as usize;
        for i in start_sector..self.sectors_per_cluster as usize {
            let sector_num = self.sectors_per_cluster as u64 * (cluster.get_value() - 2) as u64 + self.data_start_sector + i as u64;
            let sector = self.device.get(sector_num)?;
            for j in sector_start_index..sector.len() {
                if ctr >= buf.len() {
                    return Ok(ctr)
                }
                buf[ctr] = sector[j];
                ctr += 1;
            }
            sector_start_index = 0;
        }
        Ok(ctr)
    }

    pub fn read_file(
        &mut self,
        chain_start: Cluster,
        offset: usize,
        file_size: usize,
        buf: &mut [u8]
    ) -> io::Result<usize> {
        let mut bytes_to_skip = offset;
        let mut curr = chain_start;
        let mut chain_complete = false;
        let mut bytes_read = 0;
        let mut bytes_skipped = 0;
        while !chain_complete {
            match self.fat_entry(curr)?.status() {
                Status::Free => chain_complete = true,
                Status::Reserved => chain_complete = true,
                Status::Bad => chain_complete = true,
                Status::Eoc(_) => {
                    if bytes_to_skip < self.get_cluster_size() {
                        bytes_read += self.read_cluster(curr, bytes_to_skip, &mut buf[bytes_read..])?;
                        bytes_skipped += bytes_to_skip;
                        if bytes_read + bytes_skipped > file_size {
                            bytes_read -= bytes_read + bytes_skipped - file_size;
                        }
                    }
                    chain_complete = true;
                }
                Status::Data(next) => {
                    if bytes_to_skip < self.get_cluster_size() {
                        bytes_read += self.read_cluster(curr, bytes_to_skip, &mut buf[bytes_read..])?;
                        if bytes_read >= buf.len() {
                            return Ok(bytes_read);
                        }
                        bytes_skipped += bytes_to_skip;
                        bytes_to_skip = 0;
                    } else {
                        bytes_to_skip -= self.get_cluster_size();
                        bytes_skipped += self.get_cluster_size();
                    }
                    curr = next;
                }
            }
        }
        Ok(bytes_read)
    }

    //
    //  * A method to read all of the clusters chained from a starting cluster
    //    into a vector.
    //
    pub fn read_chain(
        &mut self,
        start: Cluster,
        buf: &mut Vec<u8>
    ) -> io::Result<usize> {
        let mut curr = start;
        let mut chain_complete = false;
        let mut bytes_read = 0;
        while !chain_complete {
            let f = self.fat_entry(curr)?;
            match f.status() {
                Status::Free => chain_complete = true,
                Status::Reserved => chain_complete = true,
                Status::Bad => chain_complete = true,
                Status::Eoc(_) => {
                    let mut cluster_buf = Vec::with_capacity(self.get_cluster_size());
                    cluster_buf.resize(self.get_cluster_size(), 0);
                    bytes_read += self.read_cluster(curr, 0, &mut cluster_buf)?;
                    buf.extend(cluster_buf);
                    chain_complete = true;
                }
                Status::Data(next) => {
                    let mut cluster_buf = Vec::with_capacity(self.get_cluster_size());
                    cluster_buf.resize(self.get_cluster_size(), 0);
                    bytes_read += self.read_cluster(curr, 0, &mut cluster_buf)?;
                    buf.extend(cluster_buf);
                    curr = next;
                }
            }
        }
        Ok(bytes_read)
    }
    //
    //  * A method to return a reference to a `FatEntry` for a cluster where the
    //    reference points directly into a cached sector.
    //
    fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let fat_sector_number = cluster.fat_table_sector(self.fat_start_sector, self.bytes_per_sector);
        let fat_sector = self.device.get(fat_sector_number)?;
        let fat_entries = unsafe { fat_sector.cast::<FatEntry>() };
        Ok(&fat_entries[cluster.fat_sector_index(fat_entries.len())])
    }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Entry = Entry<HANDLE>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        let mut dir_stack = Vec::new();
        let mut a_file = None;
        for component in path.as_ref().components() {
            if let Some(_) = a_file {
                return Err(newioerr!(InvalidInput, "was not directory"));
            }
            match component {
                Component::RootDir => {
                    dir_stack.push(Dir {
                        vfat: self.clone(),
                        first_cluster: self.lock(|vfat| vfat.rootdir_cluster),
                        name: "".to_string(),
                        metadata: Default::default(),
                    });
                }
                Component::ParentDir => {
                    match dir_stack.pop() {
                        None => return Err(newioerr!(InvalidInput, "no such directory")),
                        _ => {}
                    }
                }
                Component::Normal(name) => {
                    if let Some(dir) = dir_stack.last() {
                        match dir.find(name)? {
                            Entry::File(f) => a_file = Some(f),
                            Entry::Dir(d) => {
                                dir_stack.push(d);
                            }
                        }
                    } else {
                        return Err(newioerr!(InvalidInput, "path is not absolute"));
                    }
                }
                _ => {}
            }
        }
        if let Some(file) = a_file {
            Ok(Entry::File(file))
        } else if let Some(dir) = dir_stack.pop() {
            Ok(Entry::Dir(dir))
        } else {
            Err(newioerr!(InvalidInput, "empty path"))
        }
    }
}
