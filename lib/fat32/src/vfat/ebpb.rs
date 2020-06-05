use core::fmt;
use core::mem;
use shim::const_assert_size;

use crate::traits::BlockDevice;
use crate::vfat::Error;

#[repr(C, packed)]
pub struct BiosParameterBlock {
    jmp_short_noop: [u8; 3],
    oem_identifier: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fats: u8,
    max_directory_entries: u16,
    total_logical_sectors_smol: u16,
    fat_id: u8,
    sectors_per_fat_smol: u16,
    sectors_per_track: u16,
    heads: u16,
    hidden_sectors: u32,
    pub total_logical_sectors: u32,
    pub sectors_per_fat: u32,
    flags: u16,
    version_number: u16,
    pub root_directory_cluster: u32,
    fsinfo_sector: u16,
    backup_boot_sector: u16,
    reserved: [u8; 12],
    drive_number: u8,
    reserved_flags: u8,
    signature: u8,
    volume_id: u32,
    volume_label: [u8; 11],
    system_id: [u8; 8],
    boot_code: [u8; 420],
    bootable_partition_signature: u16,
}

const_assert_size!(BiosParameterBlock, 512);

impl BiosParameterBlock {
    /// Reads the FAT32 extended BIOS parameter block from sector `sector` of
    /// device `device`.
    ///
    /// # Errors
    ///
    /// If the EBPB signature is invalid, returns an error of `BadSignature`.
    pub fn from<T: BlockDevice>(mut device: T, sector: u64) -> Result<BiosParameterBlock, Error> {
        let mut buf = [0; 512];
        device.read_sector(sector, &mut buf)?;
        let ebpb: BiosParameterBlock = unsafe { mem::transmute(buf) };
        if ebpb.bootable_partition_signature != 0xaa55 {
            return Err(Error::BadSignature);
        }
        Ok(ebpb)
    }
}

impl fmt::Debug for BiosParameterBlock {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BiosParameterBlock")
            .field("oem_identifier", &self.oem_identifier)
            .field("bytes_per_sector", &{ self.bytes_per_sector })
            .field("sectors_per_cluster", &self.sectors_per_cluster)
            .field("reserved_sectors", &{ self.reserved_sectors })
            .field("fats", &self.fats)
            .field("max_directory_entries", &{ self.max_directory_entries })
            .field("fat_id", &self.fat_id)
            .field("sectors_per_track", &{ self.sectors_per_track })
            .field("heads", &{ self.heads })
            .field("hidden_sectors", &{ self.hidden_sectors })
            .field("total_logical_sectors", &{ self.total_logical_sectors })
            .field("sectors_per_fat", &{ self.sectors_per_fat })
            .field("flags", &{ self.flags })
            .field("version_number", &{ self.version_number })
            .field("root_directory_cluster", &{ self.root_directory_cluster })
            .field("fsinfo_sector", &{ self.fsinfo_sector })
            .field("backup_boot_sector", &{ self.backup_boot_sector })
            .field("drive_number", &self.drive_number)
            .field("signature", &self.signature)
            .field("volume_id", &{ self.volume_id })
            .field("volume_label", &self.volume_label)
            .field("system_id", &self.system_id)
            .field("bootable_partition_signature", &{ self.bootable_partition_signature })
            .finish()
    }
}
