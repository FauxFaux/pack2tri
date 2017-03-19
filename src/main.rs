#![feature(io)]

extern crate argparse;
extern crate bit_set;
extern crate compress;
extern crate libc;

use std::fs;
use std::io;
use std::mem;
use std::path;
use std::slice;

use argparse::Store;

use bit_set::BitSet;

use libc::c_void;

//magic:
use std::io::Read;
use std::os::unix::io::AsRawFd;

type CharResult = Result<char, io::CharsError>;

static TRI_MAX: usize = 64 * 64 * 64;

fn simplify(wut: char) -> u8 {
    let c = match wut {
        'a' ... 'z' => (wut as u8 - 'a' as u8 + 'A' as u8) as char,
        _ => wut,
    };

    if c > 128 as char {
        if c.is_whitespace() {
            return 2;
        }
        if c.is_control() {
            return 0;
        }
        return 63;
    }

    let sym_end = 33u8;
    let letters = 21u8;

    match c {
        '\r' | '\n' => 1,
        '\t' | '\x0c' | '\x0b' | ' ' => 2,
        '!' => 3,
        '"' | '\'' | '`' => 4,
        '$' => 5,
        '%' => 6,
        '&' => 7,
        '(' ... '@' => 8 + (c as u8 - '(' as u8),
        'A' ... 'I' => sym_end + (c as u8 - 'A' as u8),
        'J' | 'K' => sym_end + letters,
        'L' ... 'P' => sym_end + (c as u8 - 'A' as u8) - 2,
        'Q' => sym_end + letters,
        'R' ... 'W' => sym_end + (c as u8 - 'A' as u8) - 3,
        'X' | 'Z' => sym_end + letters,
        'Y' => sym_end + letters - 1,
        '[' => 55,
        '\\' => 56,
        ']'=> 57,
        '^' | '~' | '#' => 58,
        '_' => 59,
        '{' ... '}' => 60 + (c as u8 - '{' as u8),
        _ => 0,
    }
}

fn explain(wut: u8) -> char {
    match wut {
        0 => 'X',
        1 => 'N',
        2 => ' ',
        3 => '!',
        4 => '"',
        5 => '$',
        6 => '%',
        7 => '&',
        8 ... 32 => (wut - 8 + '(' as u8) as char,
        33 ... 41 => (wut - 33 + 'a' as u8) as char,
        42 ... 46 => (wut - 42 + 'l' as u8) as char,
        47 ... 52 => (wut - 47 + 'r' as u8) as char,
        53 => 'y',
        54 => 'X',
        55 => '[',
        56 => '\\',
        57 => ']',
        58 => '#',
        59 => '_',
        60 ... 62 => (wut - 60 + '{' as u8) as char,
        63 => 'U',
        _ => 'D',
    }
}

fn unpack(wut: usize) -> String {
    let mut ret = String::with_capacity(3);
    ret.push(explain((wut / 64 / 64 % 64) as u8));
    ret.push(explain((wut / 64 % 64) as u8));
    ret.push(explain((wut % 64) as u8));
    return ret;
}

fn trigrams_for<T: Iterator<Item=CharResult>>(input: T) -> Result<BitSet, String> {
    let mut line: u64 = 1;
    let mut prev: [u8; 3] = [0; 3];
    let mut ret: BitSet = BitSet::with_capacity(64 * 64 * 64);

    for (off, maybe_char) in input.enumerate() {
        let c = try!(maybe_char.map_err(|e| {
            format!("line {}: file char {}: failed: {}", line, off, e)
        }));
        if '\n' == c {
            line += 1;
        }
        if '\0' == c {
            return Err(format!("line {}: null found: not a text file", line));
        }
        prev[0] = prev[1];
        prev[1] = prev[2];
        prev[2] = simplify(c);
        let tri: usize = 64 * 64 * prev[0] as usize + 64 * prev[1] as usize + prev[2] as usize;
        ret.insert(tri);
    }
    return Ok(ret);
}

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

        self.idx.data[trigram as usize] = self.free_page as u32;
        self.free_page += 1;
        if self.free_page > self.pages.data.len() / self.page_size {
            let old_len = self.pages.data.len();
            self.pages.remap(old_len + 100 * self.page_size)?;
        }
        Ok(self.free_page)
    }

    fn append(&mut self, trigram: u32, document: u64) -> io::Result<()> {
        let page = self.page_for(trigram)?;
        let header_loc = page * self.page_size;
        let header = self.pages.data[header_loc];
        assert!(header < self.page_size as u64);
        self.pages.data[header_loc] += 1;
        self.pages.data[page * self.page_size + 1 + header as usize] = document;
        Ok(())
    }
}

fn main() {
    let mut from: String = "".to_string();
    let mut simple: u64 = 0;
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("totally not a load of tools glued together");
        ap.refer(&mut from)
                .required()
                .add_option(&["-f", "--input-file"], Store,
                            "pack file to read");
        ap.refer(&mut simple)
                .add_option(&["--simple"], Store,
                            "not a pack, just a normal decompressed file");
        ap.parse_args_or_exit();
    }

    let mut idx = Index::new().unwrap();

    if 0 != simple {
        let fh = fs::File::open(from).expect("input file must exist and be readable");
        let trigrams = trigrams_for(fh.chars()).expect("trigramming must work");
        for found in trigrams.iter() {
            idx.append(found as u32, simple).unwrap();
        }
        return;
    }



    unimplemented!();
}
