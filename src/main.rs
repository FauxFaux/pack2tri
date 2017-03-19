#![feature(io)]

extern crate argparse;
extern crate bit_set;
extern crate compress;


use std::fs;
use std::io;

use argparse::{Store, StoreTrue};

use bit_set::BitSet;

//magic:
use std::io::Read;

type CharResult = Result<char, io::CharsError>;

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

fn main() {
    let mut from: String = "".to_string();
    let mut simple = false;
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("totally not a load of tools glued together");
        ap.refer(&mut from)
                .required()
                .add_option(&["-f", "--input-file"], Store,
                            "pack file to read");
        ap.refer(&mut simple)
                .add_option(&["--simple"], StoreTrue,
                            "not a pack, just a normal decompressed file");
        ap.parse_args_or_exit();
    }

    if simple {
        let fh = fs::File::open(from).expect("input file must exist and be readable");
        let trigrams = trigrams_for(fh.chars()).expect("trigramming must work");
        for found in trigrams.iter() {
            println!("{}: {}", found, unpack(found));
        }
        return;
    }


    unimplemented!();

}
