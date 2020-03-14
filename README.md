# bdflib

This library provides methods to read and write Binary Dictionary Format files that can be used to represent rainbow tables.

## Usage

### Read

```rust
use bdf::io::BDFReader;
use std::fs::File;
use std::io::BufReader;

fn main() {
    let f = File::open("dictionary.bdf").unwrap();
    let buf_reader = BufReader::new(f);
    let mut bdf_reader = BDFReader::new(buf_reader);
    bdf_reader.read_metadata().unwrap();
    let lookup_table = bdf_reader.read_lookup_table().unwrap();
    let lookup_table = lookup_table.clone();
    while let Ok(next_chunk) = &mut bdf_reader.next_chunk() {
        if let Ok(entries) = next_chunk.data_entries(&lookup_table) {
            println!("{:?}", entries);
        }
    }
}
```

### Write

```rust
use bdf::chunks::{DataEntry, HashEntry};
use bdf::io::BDFWriter;
use std::fs::File;
use std::io::BufWriter;
use std::convert::Into;

fn main() {
    let f = File::create("dictionary.bdf").unwrap();
    let buf_writer = BufWriter::new(f);
    let entry_count = 1;
    let mut bdf_writer = BDFWriter::new(buf_writer, entry_count, false);
    bdf_writer.add_lookup_entry(HashEntry::new("fakehash".into(), 3)).unwrap();
    let mut entry = DataEntry::new("foo".into());
    entry.add_hash_value("fakehash".into(), vec![0, 2, 3]);
    bdf_writer.add_data_entry(entry).unwrap();
    bdf_writer.flush().unwrap();
    bdf_writer.flush_writer().unwrap();
    println!("Finished writing!");
}
```

## Binary Dictionary File Format (bdf)

```
<BDF> = <Header><Chunk(META)><Chunk(HTBL)>[<Chunk(DTBL)>]
```

All `u8` format are unsigned BigEndian numbers.

### Header

Raw (hex) `42 44 46 01 52 41 49 4e 42 4f 57`

| Position | Name        | Value     | Meaning                            |
| -------- | ----------- | --------- | ---------------------------------- |
| 0-2      | Format      | `BDF`     | Indicates the bdf Format           |
| 3-4      | Version     | u32       | The revision of the format (0x01)  |
| 4-10     | to be fancy | `RAINBOW` | The word "Rainbow" because why not |


### Chunk

| Position      | Name       | Value | Meaning                        |
| ------------- | ---------- | ----- | ------------------------------ |
| 0-3           | length (l) | u32   | the length of the data chunk   |
| 4-7           | name       | ASCII | the name of the chunk          |
| 8-l           | data       | any   | the data of the chunk          |
| l + 1 - l + 5 | crc        | crc   | the crc sum value of the chunk |

### Meta Chunk

The format of the data inside the `META` chunk.
The data is mandatory for the file to be interpreted and the chunk should be the first chunk in the file.

| Position | Name                    | Value            | Meaning                                                          |
| -------- | ----------------------- | ---------------- | ---------------------------------------------------------------- |
| 0-3      | chunk count             | u32              | The number of `DTBL` chunks in the file                          |
| 4-7      | entries per chunk       | u32              | The maximum number of Data Rows in each chunk                    |
| 8-15     | total number of entries | u64              | The total number Data Rows in the file                           |
| 16-19    | compression method      | ASCII/0x00000000 | The name of the compression method or null bytes if none is used |

### Data Row

The format inside the `DTBL` chunk.
A chunk contains multiple data rows.

| Position  | Name       | Value | Meaning                                                                                 |
| --------- | ---------- | ----- | --------------------------------------------------------------------------------------- |
| 0-3       | length(lt) | u32   | the total length of the data row                                                        |
| 4-7       | length (l) | u32   | the length of the password string                                                       |
| 8-l       | password   | UTF-8 | The password string                                                                     |
| 1: l+1 - l+5 | type       | u32   | the id of the hash function                                                             |
| l+6 - l+x | hash       | any   | the value of the hash function. The length has to be looked up by the hash functions ID |
| goto 1   |


### Hash Entry

The format inside the `HTBL` chunk.
Just like the DataRow the HashEntry is contained in the chunk multiple times.

| Position  | Name          | Value | Meaning                                             |
| --------- | ------------- | ----- | --------------------------------------------------- |
| 0-3       | ID            | u32   | the id of the entry that is used in the data tables |
| 4-7       | output length | u32   | the length of the output of the hash function       |
| 8-11      | length        | u32   | the length of the hash functions name               |
| 12-length | name          | ASCII | the name of the hash function                       |