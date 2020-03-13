#[cfg(test)]
mod tests {
    use super::io::BDFWriter;
    use std::io::{BufWriter, Error};
    use std::fs::File;
    use crate::chunks::{HashEntry, DataEntry};

    const FOO: &str = "foo";
    const BAR: &str = "bar";

    #[test]
    fn it_writes_uncompressed() -> Result<(), Error> {
        let file = File::create("tmp.bdf")?;
        let f = BufWriter::new(file);
        let mut writer = BDFWriter::new(f, 2, false);
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
        writer.flush()?;
        writer.flush_writer()?;

        Ok(())
    }

    #[test]
    fn it_writes_compressed() -> Result<(), Error> {
        let file = File::create("tmp-compressed.bdf")?;
        let f = BufWriter::new(file);
        let mut writer = BDFWriter::new(f, 2, true);
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
        writer.flush()?;
        writer.flush_writer()?;

        Ok(())
    }
}

pub mod chunks;
pub mod io;