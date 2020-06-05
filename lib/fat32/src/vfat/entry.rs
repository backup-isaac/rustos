use crate::traits;
use crate::vfat::{Dir, File, Metadata, VFatHandle};

// You can change this definition if you want
#[derive(Debug)]
pub enum Entry<HANDLE: VFatHandle> {
    File(File<HANDLE>),
    Dir(Dir<HANDLE>),
}

impl<HANDLE: VFatHandle> traits::Entry for Entry<HANDLE> {
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Metadata = Metadata;

    fn name(&self) -> &str {
        match self {
            Entry::File(f) => {
                &f.name
            }
            Entry::Dir(d) => {
                &d.name
            }
        }
    }

    fn metadata(&self) -> &Self::Metadata {
        match self {
            Entry::File(f) => {
                &f.metadata
            }
            Entry::Dir(d) => {
                &d.metadata
            }
        }
    }

    fn as_file(&self) -> Option<&File<HANDLE>> {
        match self {
            Entry::File(f) => Some(f),
            _ => None
        }
    }

    fn as_dir(&self) -> Option<&Dir<HANDLE>> {
        match self {
            Entry::Dir(d) => Some(d),
            _ => None
        }
    }

    fn into_file(self) -> Option<File<HANDLE>> {
        match self {
            Entry::File(f) => Some(f),
            _ => None
        }
    }

    fn into_dir(self) -> Option<Dir<HANDLE>> {
        match self {
            Entry::Dir(d) => Some(d),
            _ => None
        }
    }
}
