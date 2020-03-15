#[cfg(test)]
mod tests {

    use super::io::BDFWriter;
    use crate::chunks::{DataEntry, HashEntry};
    use crate::io::BDFReader;
    use std::fs::{remove_file, File};
    use std::io::Error;

    const FOO: &str = "foo";
    const BAR: &str = "bar";

    #[test]
    fn it_writes_uncompressed() -> Result<(), Error> {
        let mut writer = new_writer("tmp1.bdf", 2, false)?;
        writer.set_entries_per_chunk(24)?;

        writer.add_lookup_entry(HashEntry::new(BAR.to_string(), 5))?;
        writer.add_lookup_entry(HashEntry::new(FOO.to_string(), 4))?;

        let mut entry_1 = DataEntry::new("lol".to_string());
        entry_1.add_hash_value(FOO.to_string(), vec![0, 1, 0, 2]);
        entry_1.add_hash_value(BAR.to_string(), vec![0, 2, 3, 4, 5]);
        writer.add_data_entry(entry_1)?;

        let mut entry_2 = DataEntry::new("lel".to_string());
        entry_2.add_hash_value(BAR.to_string(), vec![0, 3, 2, 1, 5]);
        entry_2.add_hash_value(FOO.to_string(), vec![4, 5, 2, 3]);
        writer.add_data_entry(entry_2)?;

        writer.finish()?;
        remove_file("tmp1.bdf")?;

        Ok(())
    }

    #[test]
    fn it_writes_compressed() -> Result<(), Error> {
        let mut writer = new_writer("tmp2.bdf", 2, true)?;
        writer.set_compression_level(3);

        writer.add_lookup_entry(HashEntry::new(FOO.to_string(), 4))?;
        writer.add_lookup_entry(HashEntry::new(BAR.to_string(), 5))?;

        let mut entry_1 = DataEntry::new("lol".to_string());
        entry_1.add_hash_value(FOO.to_string(), vec![2, 4, 0, 2]);
        entry_1.add_hash_value(BAR.to_string(), vec![5, 2, 1, 4, 5]);
        writer.add_data_entry(entry_1)?;

        let mut entry_2 = DataEntry::new("lel".to_string());
        entry_2.add_hash_value(BAR.to_string(), vec![0, 3, 2, 1, 5]);
        entry_2.add_hash_value(FOO.to_string(), vec![4, 5, 2, 3]);
        writer.add_data_entry(entry_2)?;

        writer.finish()?;

        remove_file("tmp2.bdf")?;

        Ok(())
    }

    #[test]
    fn it_reads() -> Result<(), Error> {
        create_simple_file("tmp3.bdf", false)?;
        let mut reader = new_reader("tmp3.bdf")?;
        reader.read_start()?;
        let lookup_table = &reader.lookup_table.clone().unwrap();
        let mut next_chunk = reader.next_chunk()?;
        let data_entries = next_chunk.data_entries(lookup_table)?;
        assert_eq!(data_entries[0].plain, "lol".to_string());

        remove_file("tmp3.bdf")?;

        Ok(())
    }

    #[test]
    fn it_reads_compressed() -> Result<(), Error> {
        create_simple_file("tmp4.bdf", true)?;
        let mut reader = new_reader("tmp4.bdf")?;
        reader.read_metadata()?;
        let lookup_table = &reader.read_lookup_table()?.clone();
        let mut next_chunk = reader.next_chunk()?;
        let data_entries = next_chunk.data_entries(lookup_table)?;
        assert_eq!(data_entries[0].plain, "lol".to_string());

        remove_file("tmp4.bdf")?;

        Ok(())
    }

    fn create_simple_file(name: &str, compressed: bool) -> Result<(), Error> {
        let mut writer = new_writer(name, 1, compressed)?;

        writer.add_lookup_entry(HashEntry::new(FOO.to_string(), 4))?;
        let mut entry_1 = DataEntry::new("lol".to_string());
        entry_1.add_hash_value(FOO.to_string(), vec![2, 4, 0, 2]);
        writer.add_data_entry(entry_1)?;

        writer.finish()?;

        Ok(())
    }

    fn new_reader(file_name: &str) -> Result<BDFReader, Error> {
        let file = File::open(file_name)?;

        Ok(BDFReader::new(file))
    }

    fn new_writer(file_name: &str, entries: u64, compress: bool) -> Result<BDFWriter, Error> {
        let file = File::create(file_name)?;

        Ok(BDFWriter::new(file, entries, compress))
    }
}

pub mod chunks;
pub mod io;
