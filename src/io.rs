use super::chunks::*;
use std::io::{Write, BufWriter, ErrorKind, BufReader, Read};
use std::fs::File;
use std::collections::HashMap;
use std::io::Error;
use byteorder::{BigEndian, ByteOrder};
use std::convert::TryInto;

const ENTRIES_PER_CHUNK: u32 = 100_000;


pub struct BDFReader {
    reader: BufReader<File>,
    pub metadata: Option<MetaChunk>,
    pub lookup_table: Option<HashLookupTable>,
    compressed: bool,
}


pub struct BDFWriter {
    writer: BufWriter<File>,
    metadata: MetaChunk,
    lookup_table: HashLookupTable,
    data_entries: Vec<DataEntry>,
    head_written: bool,
    compressed: bool,
    compression_level: u32,
}

impl BDFWriter {
    
    pub fn new(writer: BufWriter<File>, entry_count: u64, compress: bool) -> Self {
        Self {
            metadata: MetaChunk::new(entry_count, ENTRIES_PER_CHUNK, compress),
            lookup_table: HashLookupTable::new(HashMap::new()),
            data_entries: Vec::new(),
            writer,
            head_written: false,
            compressed: compress,
            compression_level: 1,
        }
    }

    /// Adds an entry to the hash lookup table
    /// If the lookup table has already been written to the file, an error is returned
    pub fn add_lookup_entry(&mut self, mut entry: HashEntry) -> Result<u32, Error> {
        if self.head_written {
            return Err(Error::new(
                ErrorKind::Other,
                "the head has already been written",
            ));
        }
        let id = self.lookup_table.entries.len() as u32;
        entry.id = id;
        self.lookup_table.entries.insert(id, entry);

        Ok(id)
    }

    /// Adds a data entry to the file.
    /// If the number of entries per chunk is reached,
    /// the data will be written to the file
    pub fn add_data_entry(&mut self, data_entry: DataEntry) -> Result<(), Error> {
        self.data_entries.push(data_entry);
        if self.data_entries.len() >= ENTRIES_PER_CHUNK as usize {
            self.flush()?;
        }

        Ok(())
    }

    /// Writes the data to the file
    
    pub fn flush(&mut self) -> Result<(), Error> {
        if !self.head_written {
            self.writer.write(BDF_HDR)?;
            let mut generic_meta = GenericChunk::from(&self.metadata);
            self.writer.write(generic_meta.serialize().as_slice())?;
            let mut generic_lookup = GenericChunk::from(&self.lookup_table);
            self.writer.write(generic_lookup.serialize().as_slice())?;
            self.head_written = true;
        }
        let mut data_chunk =
            GenericChunk::from_data_entries(&self.data_entries, &self.lookup_table);
        if self.compressed {
            data_chunk.compress(self.compression_level)?;
        }
        let data = data_chunk.serialize();
        self.writer.write(data.as_slice())?;
        self.data_entries = Vec::new();

        Ok(())
    }

    /// Flushes the writer
    /// This should be called when no more data is being written
    pub fn flush_writer(&mut self) -> Result<(), Error> {
        self.writer.flush()
    }

    /// Sets the compression level for lzma compression
    pub fn set_compression_level(&mut self, level: u32) {
        self.compression_level = level;
    }
}

impl BDFReader {

    /// Creates a new BDFReader
    pub fn new(reader: BufReader<File>) -> Self {
        Self {
            metadata: None,
            lookup_table: None,
            reader,
            compressed: false,
        }
    }

    /// Verifies the header of the file and reads and stores the metadata
    pub fn read_metadata(&mut self) -> Result<&MetaChunk, Error> {
        if !self.validate_header() {
            return Err(Error::new(ErrorKind::InvalidData, "invalid BDF Header"));
        }
        let meta_chunk: MetaChunk = self.next_chunk()?.try_into()?;
        if let Some(method) = &meta_chunk.compression_method {
            if *method == LZMA.to_string() {
                self.compressed = true;
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "unsupported compression method",
                ));
            }
        }
        self.metadata = Some(meta_chunk);

        if let Some(chunk) = &self.metadata {
            Ok(&chunk)
        } else {
            Err(Error::new(
                ErrorKind::Other,
                "Failed to read self assigned metadata.",
            ))
        }
    }

    /// Reads the lookup table of the file.
    /// This function should be called after the read_metadata function was called
    pub fn read_lookup_table(&mut self) -> Result<&HashLookupTable, Error> {
        match &self.metadata {
            None => self.read_metadata()?,
            Some(t) => t,
        };
        let lookup_table: HashLookupTable = self.next_chunk()?.try_into()?;
        self.lookup_table = Some(lookup_table);

        if let Some(chunk) = &self.lookup_table {
            Ok(&chunk)
        } else {
            Err(Error::new(
                ErrorKind::Other,
                "failed to read self assigned chunk",
            ))
        }
    }

    /// Validates the header of the file
    fn validate_header(&mut self) -> bool {
        let mut header = [0u8; 11];
        let _ = self.reader.read(&mut header);

        header == BDF_HDR.as_ref()
    }

    /// Returns the next chunk if one is available.
    pub fn next_chunk(&mut self) -> Result<GenericChunk, Error> {
        let mut length_raw = [0u8; 4];
        let _ = self.reader.read_exact(&mut length_raw)?;
        let length = BigEndian::read_u32(&mut length_raw);
        let mut name_raw = [0u8; 4];
        let _ = self.reader.read_exact(&mut name_raw)?;
        let name = String::from_utf8(name_raw.to_vec()).expect("Failed to parse name string.");
        let mut data = vec![0u8; length as usize];
        let _ = self.reader.read_exact(&mut data)?;
        let mut crc_raw = [0u8; 4];
        let _ = self.reader.read_exact(&mut crc_raw)?;
        let crc = BigEndian::read_u32(&mut crc_raw);
        let mut gen_chunk = GenericChunk {
            length,
            name,
            data,
            crc,
        };
        if gen_chunk.name == DTBL_CHUNK_NAME.to_string() && self.compressed {
            gen_chunk.decompress()?;
        }

        Ok(gen_chunk)
    }
}