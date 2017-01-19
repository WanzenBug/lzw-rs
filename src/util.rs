extern crate bitstream;

use std::mem::{size_of};
use std::io::Read;
use std::ops::Index;

use std::io::Result as IOResult;

pub struct LZWDict(Vec<LZWDictEntry>);


impl LZWDict {
    pub fn new() -> Self {
        let mut vec = Vec::with_capacity(4096);
        for i in 0..256 {
            vec.push(LZWDictEntry {
                symbol: i as u8,
                prefix: None,
            });
        }
        LZWDict(vec)
    }

    pub fn push(&mut self, entry: LZWDictEntry) {
        self.0.push(entry)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl Index<usize> for LZWDict {
    type Output = LZWDictEntry;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

pub struct LZWSearchTreeNode {
    pub children: [Option<usize>; 256],
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct BitIndex(usize, usize);

impl BitIndex {
    pub fn new(data: usize, len: usize) -> Self {
        BitIndex(data, len)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }

    pub fn from_bits<R>(reader: &mut bitstream::BitReader<R>, len: usize) -> IOResult<Option<Self>> where R: Read {
        let mut data = 0;
        for i in 0..len {
            let bit = match reader.read_bit()? {
                Some(true) => 1,
                Some(false) => 0,
                None => { return Ok(None); }
            };
            data ^= bit << (len - i - 1);
        }
        Ok(Some(BitIndex(data, len)))
    }
}

impl Iterator for BitIndex {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.1 > 0 {
            let bit = 1usize << (self.1 - 1);
            let ret = (self.0 & bit) > 0;
            self.1 -= 1;
            Some(ret)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct BitSizeEnumerator {
    current: usize,
    current_size: usize,
    next_increase: usize
}

impl BitSizeEnumerator {
    pub fn new(start: usize) -> Self {
        let size = size_of::<usize>() * 8;
        let leading_one = size - start.leading_zeros() as usize;
        BitSizeEnumerator {
            current: start,
            current_size: leading_one,
            next_increase: 1usize << leading_one,
        }
    }
}

impl Iterator for BitSizeEnumerator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.next_increase {
            self.current_size += 1;
            let res = Some(self.current_size);
            self.current += 1;
            self.next_increase <<= 1;
            res
        } else if self.next_increase < self.current {
            None
        } else {
            let res = Some(self.current_size);
            self.current += 1;
            res
        }
    }
}

pub struct LZWDictEntry {
    pub symbol: u8,
    pub prefix: Option<usize>
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerator() {
        let mut it = BitSizeEnumerator::new(1);
        assert_eq!(it.next(), Some((1)));
        assert_eq!(it.next(), Some((2)));
        assert_eq!(it.next(), Some((2)));
        assert_eq!(it.next(), Some((3)));
    }

    #[test]
    fn test_bit_index() {
        let test = BitIndex::new(84, 8);
        let mut vec = Vec::new();
        {
            use bitstream::BitWriter;
            let mut writer = BitWriter::new(&mut vec);
            for bit in test {
                assert!(writer.write_bit(bit).is_ok());
            }
        }
        let test2 = {
            use std::io::Cursor;
            use bitstream::BitReader;
            let cursor = Cursor::new(&vec);
            let mut reader = BitReader::new(cursor);
            let res = BitIndex::from_bits(&mut reader, 8);
            assert!(res.is_ok());
            let opt = res.unwrap();
            assert!(opt.is_some());
            opt.unwrap()
        };
        assert_eq!(test, test2);
    }
}