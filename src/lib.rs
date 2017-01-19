extern crate bitstream;

use std::io::{Read, Write};
use std::io::Result as IOResult;
use std::io::Error as IOError;
use std::io::ErrorKind as IOErrorKind;

use bitstream::{BitWriter, BitReader};

mod util;

use util::{LZWDict, LZWDictEntry, LZWSearchTreeNode, BitSizeEnumerator, BitIndex};

pub struct LZWWriter<W> where W: Write {
    inner: BitWriter<W>,
    lookup: Vec<LZWSearchTreeNode>,
    state: LZWWriterState,
    enumerator: BitSizeEnumerator,
}

enum LZWWriterState {
    Empty,
    Found(usize)
}

impl<W> LZWWriter<W> where W: Write {
    pub fn new(writer: W) -> Self {
        let mut v = Vec::with_capacity(256);
        for _ in 0..256 {
            v.push(LZWSearchTreeNode {
                children: [None; 256]
            });
        }


        LZWWriter {
            inner: BitWriter::new(writer),
            lookup: v,
            state: LZWWriterState::Empty,
            enumerator: BitSizeEnumerator::new(255),
        }
    }
}

impl<W> Write for LZWWriter<W> where W: Write {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        for &byte in buf {
            match self.state {
                LZWWriterState::Empty => {
                    self.state = LZWWriterState::Found(byte as usize)
                }
                LZWWriterState::Found(idx) => {
                    match self.lookup[idx].children[byte as usize] {
                        Some(x) => {
                            self.state = LZWWriterState::Found(x as usize);
                        },
                        None => {
                            let next_size = self.enumerator.next().ok_or(IOError::from(IOErrorKind::Other))?;
                            let output = BitIndex::new(idx, next_size);
                            self.lookup[idx].children[byte as usize] = Some(self.lookup.len());
                            self.lookup.push(LZWSearchTreeNode { children: [None; 256] });
                            for bit in output {
                                self.inner.write_bit(bit)?;
                            }
                            self.state = LZWWriterState::Found(byte as usize);
                        }
                    }
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> IOResult<()> {
        Ok(())
    }
}

impl<W> Drop for LZWWriter<W> where W: Write {
    fn drop(&mut self) {
        if let LZWWriterState::Found(idx) = self.state {
            let next_size = match self.enumerator.next() {
                Some(x) => x,
                None => { return; }
            };
            let output = BitIndex::new(idx, next_size);
            for bit in output {
                match self.inner.write_bit(bit) {
                    Ok(_) => (),
                    Err(_) => { return; }
                }
            }
            self.state = LZWWriterState::Empty;
        }
    }
}


pub struct LZWReader<R> where R: Read {
    inner: BitReader<R>,
    dict: LZWDict,
    state: LZWReaderState,
    enumerator: BitSizeEnumerator,
    cache: Vec<u8>,
}

#[derive(Debug)]
enum LZWReaderState {
    Empty,
    Copy(usize),
    AwaitNew(usize),
    Ended,
}

impl<R> LZWReader<R> where R: Read {
    pub fn new(reader: R) -> Self {
        LZWReader {
            inner: BitReader::new(reader),
            dict: LZWDict::new(),
            state: LZWReaderState::Empty,
            enumerator: BitSizeEnumerator::new(255),
            cache: Vec::new(),
        }
    }

    pub fn find_first_symbol(&self, idx: usize) -> u8 {
        let mut entry = &self.dict[idx];
        while let Some(i) = entry.prefix {
            entry = &self.dict[i];
        }
        entry.symbol
    }
}

impl<R> Read for LZWReader<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        let mut counter = 0;
        while counter < buf.len() {
            match self.state {
                LZWReaderState::Empty => {
                    let next_size = self.enumerator.next().ok_or(IOError::from(IOErrorKind::Other))?;
                    let next = match BitIndex::from_bits(&mut self.inner, next_size)? {
                        Some(idx) => idx.as_usize(),
                        None => {
                            self.state = LZWReaderState::Ended;
                            break;
                        }
                    };
                    let mut entry = &self.dict[next];
                    self.cache.push(entry.symbol);
                    while let Some(idx) = entry.prefix {
                        entry = &self.dict[idx];
                        self.cache.push(entry.symbol);
                    }
                    self.state = LZWReaderState::Copy(next);
                },
                LZWReaderState::AwaitNew(idx) => {
                    let next_size = self.enumerator.next().ok_or(IOError::from(IOErrorKind::Other))?;
                    let next = match BitIndex::from_bits(&mut self.inner, next_size)? {
                        Some(idx) => idx.as_usize(),
                        None => {
                            self.state = LZWReaderState::Ended;
                            break;
                        }
                    };

                    // Case K[omega]K
                    let first_symbol = if self.dict.len() == next {
                        self.find_first_symbol(idx)
                    } else {
                        self.find_first_symbol(next)
                    };
                    self.dict.push(LZWDictEntry { symbol: first_symbol, prefix: Some(idx) });

                    let mut entry = &self.dict[next];
                    self.cache.push(entry.symbol);
                    while let Some(i) = entry.prefix {
                        entry = &self.dict[i];
                        self.cache.push(entry.symbol);
                    }
                    self.state = LZWReaderState::Copy(next);
                },
                LZWReaderState::Copy(idx) => {
                    buf[counter] = self.cache.pop().ok_or(IOError::from(IOErrorKind::Other))?;
                    counter += 1;
                    if self.cache.len() == 0 {
                        self.state = LZWReaderState::AwaitNew(idx);
                    }
                },
                LZWReaderState::Ended => { break; }
            }
        }
        Ok(counter)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lzw_writer() {
        let mut vec = Vec::new();
        {
            use std::io::Write;
            let mut writer = LZWWriter::new(&mut vec);
            let text = "This was a triumph! I'm making a note here: HUGE SUCCESS!";
            assert!(writer.write_all(text.as_bytes()).is_ok());
        }
        {
            use std::io::{Cursor, Read};
            let cursor = Cursor::new(&vec);
            let mut reader = LZWReader::new(cursor);
            let mut result = String::new();
            let iores = reader.read_to_string(&mut result);
            assert!(iores.is_ok());
            assert_eq!(result, "This was a triumph! I'm making a note here: HUGE SUCCESS!");
        }
    }
}