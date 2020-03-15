use super::chunks::*;
use byteorder::{BigEndian, ByteOrder};
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs::File;
use std::io::Error;
use std::io::{BufReader, BufWriter, ErrorKind, Read, Write};
use std::thread;
use crossbeam_channel::{bounded, Sender, Receiver};
use crossbeam_utils::sync::WaitGroup;

const ENTRIES_PER_CHUNK: u32 = 100_000;

struct ThreadManager<T1, T2> {
    pub sender_work: Option<Sender<T1>>,
    pub receiver_work: Receiver<T1>,
    pub sender_result: Sender<T2>,
    pub receiver_result: Receiver<T2>,
    pub wg: WaitGroup,
    pub threads_started: bool,
}

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
    thread_manager: ThreadManager<GenericChunk, Vec<u8>>,
}

impl<T1, T2> ThreadManager<T1, T2> {
    /// Creates a new thread manager to store channels and information
    /// about threads to control them
    pub fn new(cap: usize) -> Self {
        let (s1, r1) = bounded(cap);
        let (s2, r2) = bounded(cap);
        Self {
            sender_work: Some(s1),
            receiver_work: r1,
            sender_result: s2,
            receiver_result: r2,
            wg: WaitGroup::new(),
            threads_started: false,
        }
    }

    /// Drops the sender for work.
    pub fn drop_sender(&mut self) {
        self.sender_work = None;
    }

    /// Waits for the wait group
    pub fn wait(&mut self) {
        let wg = self.wg.clone();
        self.wg = WaitGroup::new();
        wg.wait();
    }
}

impl BDFWriter {
    /// Creates a new BDFWriter.
    /// The number for `entry_count` should be the total number of entries
    /// This is required since the META chunk containing the information is the
    /// first chunk to be written.
    /// The number of entries can be used in tools that provide a progress
    /// bar for how many entries were read.
    /// If the `compress` parameter is true, each data chunk will be compressed
    /// using lzma with a default level of 1.
    pub fn new(inner: File, entry_count: u64, compress: bool) -> Self {
        let thread_manager = ThreadManager::new(num_cpus::get());
        Self {
            metadata: MetaChunk::new(entry_count, ENTRIES_PER_CHUNK, compress),
            lookup_table: HashLookupTable::new(HashMap::new()),
            data_entries: Vec::new(),
            writer: BufWriter::new(inner),
            head_written: false,
            compressed: compress,
            compression_level: 1,
            thread_manager,
        }
    }

    /// Starts threads for parallel chunk compression
    pub fn start_threads(&self) {
        for _ in 0..num_cpus::get() {
            let compress = self.compressed;
            let compression_level = self.compression_level;
            thread::spawn({
                let r = self.thread_manager.receiver_work.clone();
                let s = self.thread_manager.sender_result.clone();
                let wg: WaitGroup = self.thread_manager.wg.clone();
                move || {
                    for mut chunk in r {
                        if compress {
                            chunk.compress(compression_level).expect("failed to compress chunk");
                        }
                        s.send(chunk.serialize()).expect("failed to send result");
                    }
                    drop(wg);
                }
            });
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
        if self.data_entries.len() >= self.metadata.entries_per_chunk as usize {
            self.flush()?;
        }

        Ok(())
    }

    /// Writes the data to the file
    fn flush(&mut self) -> Result<(), Error> {
        if !self.head_written {
            self.writer.write(BDF_HDR)?;
            let mut generic_meta = GenericChunk::from(&self.metadata);
            self.writer.write(generic_meta.serialize().as_slice())?;
            let mut generic_lookup = GenericChunk::from(&self.lookup_table);
            self.writer.write(generic_lookup.serialize().as_slice())?;
            self.head_written = true;
        }
        if !self.thread_manager.threads_started {
            self.start_threads();
            self.thread_manager.threads_started = true;
        }
        let mut data_chunk =
            GenericChunk::from_data_entries(&self.data_entries, &self.lookup_table);
        if let Some(sender) = &self.thread_manager.sender_work {
            sender.send(data_chunk).expect("failed to send work to threads");
        } else {
            if self.compressed {
                data_chunk.compress(self.compression_level)?;
            }
            self.thread_manager.sender_result.send(data_chunk.serialize()).expect("failed to send serialization result");
        }
        self.write_serialized()?;
        self.data_entries = Vec::new();

        Ok(())
    }

    fn write_serialized(&mut self) -> Result<(), Error> {
        while let Ok(data) = self.thread_manager.receiver_result.try_recv() {
            self.writer.write(data.as_slice())?;
        }

        Ok(())
    }

    /// Flushes the writer
    /// This should be called when no more data is being written
    fn flush_writer(&mut self) -> Result<(), Error> {
        self.writer.flush()
    }

    /// Flushes the buffered chunk data and the writer
    /// to finish the file.
    pub fn finish(&mut self) -> Result<(), Error> {
        self.flush()?;
        self.thread_manager.drop_sender();
        self.thread_manager.wait();
        self.write_serialized()?;
        self.flush_writer()?;

        Ok(())
    }

    /// Sets the compression level for lzma compression
    pub fn set_compression_level(&mut self, level: u32) {
        self.compression_level = level;
    }

    /// Changes the entries per chunk value.
    /// Returns an error if the metadata has already been written.
    pub fn set_entries_per_chunk(&mut self, number: u32) -> Result<(), Error> {
        if self.head_written {
            return Err(Error::new(
                ErrorKind::Other,
                "the head has already been written",
            ));
        }
        self.metadata.entries_per_chunk = number;
        self.metadata.chunk_count =
            (self.metadata.entry_count as f64 / number as f64).ceil() as u32;
        Ok(())
    }
}

impl BDFReader {
    /// Creates a new BDFReader
    pub fn new(inner: File) -> Self {
        Self {
            metadata: None,
            lookup_table: None,
            reader: BufReader::new(inner),
            compressed: false,
        }
    }

    /// Reads the metadata and lookup table
    pub fn read_start(&mut self) -> Result<(), Error> {
        self.read_metadata()?;
        self.read_lookup_table()?;

        Ok(())
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
