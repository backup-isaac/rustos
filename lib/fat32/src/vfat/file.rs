use alloc::string::String;

use shim::io::{self, SeekFrom};
use shim::newioerr;

use crate::traits;
use crate::vfat::{Cluster, Metadata, VFatHandle};

#[derive(Debug)]
pub struct File<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub first_cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
    pub seek_offset: usize,
    pub file_size: usize,
}

impl<HANDLE: VFatHandle> traits::File for File<HANDLE> {
    fn sync(&mut self) -> io::Result<()> {
        unimplemented!("filesystem is read only")
    }
    fn size(&self) -> u64 {
        self.file_size as u64
    }
}

impl<HANDLE: VFatHandle> io::Read for File<HANDLE> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        if self.seek_offset >= self.file_size {
            // EOF
            return Ok(0);
        }
        let bytes_read = self.vfat.lock(|vfat| vfat.read_file(self.first_cluster, self.seek_offset, self.file_size, buf))?;
        self.seek_offset += bytes_read;
        Ok(bytes_read)
    }
}

impl<HANDLE: VFatHandle> io::Write for File<HANDLE> {
    fn write(&mut self, _buf: &[u8]) -> Result<usize, io::Error> {
        unimplemented!("filesystem is read only")
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        unimplemented!("filesystem is read only")
    }
}

impl<HANDLE: VFatHandle> io::Seek for File<HANDLE> {
    /// Seek to offset `pos` in the file.
    ///
    /// A seek to the end of the file is allowed. A seek _beyond_ the end of the
    /// file returns an `InvalidInput` error.
    ///
    /// If the seek operation completes successfully, this method returns the
    /// new position from the start of the stream. That position can be used
    /// later with SeekFrom::Start.
    ///
    /// # Errors
    ///
    /// Seeking before the start of a file or beyond the end of the file results
    /// in an `InvalidInput` error.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(p) => {
                if p > self.file_size as u64 {
                    Err(newioerr!(InvalidInput, "Attempt to seek outside file"))
                } else {
                    self.seek_offset = p as usize;
                    Ok(p)
                }
            },
            SeekFrom::End(p) => {
                let offset = p + self.file_size as i64;
                if offset < 0 || offset > self.file_size as i64 {
                    Err(newioerr!(InvalidInput, "Attempt to seek outside file"))
                } else {
                    self.seek_offset = offset as usize;
                    Ok(offset as u64)
                }
            },
            SeekFrom::Current(p) => {
                let offset = p + self.seek_offset as i64;
                if offset < 0 || offset > self.file_size as i64 {
                    Err(newioerr!(InvalidInput, "Attempt to seek outside file"))
                } else {
                    self.seek_offset = offset as usize;
                    Ok(offset as u64)
                }
            },
        }
    }
}
