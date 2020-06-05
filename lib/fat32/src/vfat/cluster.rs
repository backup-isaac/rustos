#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone, Hash)]
pub struct Cluster(u32);

impl From<u32> for Cluster {
    fn from(raw_num: u32) -> Cluster {
        Cluster(raw_num & !(0xF << 28))
    }
}

impl Cluster {
    pub fn fat_table_sector(&self, fat_start_sector: u64, bytes_per_sector: u16) -> u64 {
        fat_start_sector + self.0 as u64 * 4 / bytes_per_sector as u64
    }

    pub fn fat_sector_index(&self, fat_sector_len: usize) -> usize {
        self.0 as usize % fat_sector_len
    }

    pub fn get_value(&self) -> u32 {
        self.0 /*& !(0xF << 28)*/
    }
}