#![feature(decode_utf8)]
#![feature(io)]

extern crate argparse;
extern crate bit_set;
extern crate byteorder;
extern crate compress;
extern crate libc;

use std::fs;
use std::io;
use std::mem;
use std::path;
use std::slice;

use argparse::{Store, StoreTrue};

use bit_set::BitSet;

use byteorder::{BigEndian, ReadBytesExt};

use compress::lz4;

use libc::c_void;

//magic:
use std::io::Read;
use std::io::Seek;
use std::os::unix::io::AsRawFd;

mod tri;


static TRI_MAX: usize = 64 * 64 * 64;

struct Mapped<'a, T: 'a> {
    file: fs::File,
    map: *mut c_void,
    data: &'a mut [T],
}

impl <'a, T: 'a> Mapped<'a, T> {
    fn fixed_len<P>(path: P, len: usize) -> io::Result<Mapped<'a, T>>
        where P: AsRef<path::Path> {
        let file = fs::OpenOptions::new().read(true).write(true).create(true).open(path)?;
        file.set_len(mem::size_of::<T>() as u64 * len as u64)?;
        let map: *mut c_void = unsafe {
            libc::mmap(0 as *mut c_void,
                       len * mem::size_of::<T>(),
                       libc::PROT_READ | libc::PROT_WRITE,
                       libc::MAP_SHARED,
                       file.as_raw_fd(),
                       0)
        };

        if libc::MAP_FAILED == map {
            return Err(io::Error::last_os_error());
        }

        let data = unsafe { slice::from_raw_parts_mut(map as *mut T, len) };
        Ok(Mapped { file, map, data })
    }

    fn remap(&mut self, len: usize) -> io::Result<()> {
        self.file.set_len(mem::size_of::<T>() as u64 * len as u64)?;
        let new_map = unsafe {
            libc::mremap(self.map,
                         self.data.len() * mem::size_of::<T>(),
                         len * mem::size_of::<T>(),
                         libc::MREMAP_MAYMOVE)
        };

        if libc::MAP_FAILED == new_map {
            return Err(io::Error::last_os_error());
        }

        self.data = unsafe { slice::from_raw_parts_mut(new_map as *mut T, len) };
        self.map = new_map;
        Ok(())
    }
}

impl <'a, T: 'a> Drop for Mapped<'a, T> {
    fn drop(&mut self) {
        unsafe {
            assert_eq!(0, libc::munmap(self.map, self.data.len() * mem::size_of::<T>()));
        }
    }
}

struct Index<'a> {
    idx: Mapped<'a, u32>,
    pages: Mapped<'a, u64>,
    free_page: usize,
    page_size: usize,
}

impl<'a> Index<'a> {
    fn new() -> io::Result<Index<'a>> {
        let idx: Mapped<u32> = Mapped::fixed_len("idx", TRI_MAX).unwrap();

        let page_size: usize = 1024;

        let pages_len: usize = match fs::metadata("pages") {
            Ok(m) => {
                let proposed: u64 = m.len() / mem::size_of::<u64>() as u64;
                assert!(proposed < usize::max_value() as u64);
                proposed as usize
            },
            Err(e) => if e.kind() == io::ErrorKind::NotFound {
                2 * page_size
            } else {
                panic!("couldn't get info on pages file: {}", e)
            }
        };

        let pages: Mapped<u64> = Mapped::fixed_len("pages", pages_len)?;
        let mut free_page: usize = pages.data.len() / page_size;

        loop {
            if 0 != pages.data[(free_page - 1) * page_size] {
                break;
            }
            if 1 == free_page {
                break;
            }
            free_page -= 1;
        }

        Ok(Index { idx, pages, free_page, page_size })
    }

    fn page_for(&mut self, trigram: u32) -> io::Result<usize> {
        assert!(trigram < TRI_MAX as u32);

        let page = self.idx.data[trigram as usize] as usize;
        if 0 != page {
            return Ok(page);
        }

        let found_page = self.next_page()?;
        self.idx.data[trigram as usize] = found_page as u32;
        Ok(found_page)
    }

    fn next_page(&mut self) -> io::Result<usize> {
        let ret = self.free_page;
        self.free_page += 1;
        if self.free_page >= self.pages.data.len() / self.page_size {
            let old_len = self.pages.data.len();
            self.pages.remap(old_len + 100 * self.page_size)?;
        }

        Ok(ret)
    }

    fn append(&mut self, trigram: u32, document: u64) -> io::Result<()> {
        let mut page = self.page_for(trigram)?;
        let mut header_loc;
        let mut header;
        loop {
            header_loc = page * self.page_size;
            header = self.pages.data[header_loc];
            if header == (self.page_size - 1) as u64 {
                page = self.next_page()?;
                self.pages.data[header_loc] = page as u64 + self.page_size as u64;
                header = 0;
                header_loc = page * self.page_size;
                break;
            } else if header >= self.page_size as u64 {
                page = header as usize - self.page_size;
            } else {
                break;
            }
        }
        self.pages.data[header_loc] += 1;
        self.pages.data[header_loc + 1 + header as usize] = document;
        Ok(())
    }

    fn append_trigrams(&mut self, trigrams: BitSet, document: u64) -> io::Result<()> {
        for found in trigrams.iter() {
            self.append(found as u32, document)?;
        }
        Ok(())
    }
}

fn round_up(x: u64) -> u64 {
    let mut ret = x;
    loop {
        if ret % 16 == 0 {
            return ret;
        }
        ret += 1;
    }
}

fn eat_chunk(mut fh: &mut fs::File) -> io::Result<BitSet> {
    let end = fh.read_u64::<BigEndian>()?;
    let extra_len = fh.read_u64::<BigEndian>()?;
    let start = fh.seek(io::SeekFrom::Current(extra_len as i64))?;
    let ret = {
        let decoder = lz4::Decoder::new(&mut fh);
        let range = decoder.bytes();
        let exploding = range.map(|x| x.unwrap());
        let decoder = std::char::decode_utf8(exploding);
        let errors = decoder.map(|x| x.map_err(|_| io::CharsError::NotUtf8));
        tri::trigrams_for(errors).map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))
    };
    let next = round_up(start + end - extra_len - 16);
    fh.seek(io::SeekFrom::Start(next))?;
    ret
}

fn main() {
    let mut from: String = "".to_string();
    let mut simple = false;
    let mut addendum: u64 = 0;
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("totally not a load of tools glued together");
        ap.refer(&mut from)
                .required()
                .add_option(&["-f", "--input-file"], Store,
                            "pack file to read");
        ap.refer(&mut addendum)
                .add_option(&["-i", "--addendum"], Store,
                            "number to add to file offset");
        ap.refer(&mut simple)
                .add_option(&["--simple"], StoreTrue,
                            "not a pack, just a normal decompressed file");
        ap.parse_args_or_exit();
    }

    let mut idx = Index::new().unwrap();

    let mut fh = fs::File::open(from).expect("input file must exist and be readable");
    if simple {
        let trigrams = tri::trigrams_for(fh.chars()).expect("trigramming must work");
        idx.append_trigrams(trigrams, addendum).unwrap();
        return;
    }

    fh.seek(io::SeekFrom::Start(16)).unwrap();
    loop {
        let document = fh.seek(io::SeekFrom::Current(0)).unwrap() + addendum;
        match eat_chunk(&mut fh) {
            Ok(trigrams) => idx.append_trigrams(trigrams, document).unwrap(),
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    break;
                }
                println!("document {}: trigramming failed: {}", document, e)
            },
        };
    }
}
