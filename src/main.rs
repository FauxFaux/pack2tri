#![feature(io)]

extern crate argparse;
extern crate compress;


use std::fs;
use std::io;

use argparse::{Store, StoreTrue};

//magic:
use std::io::Read;

type CharResult = Result<char, io::CharsError>;

enum TChar {
    Unknown,
//    Control,
    Line,
    Space,
    Normal,
}

fn simplify(wut: char) -> u8 {
    let c = match wut {
        'a' ... 'z' => (wut as u8 - 'a' as u8 + 'A' as u8) as char,
        _ => wut,
    };

    let sym_end = TChar::Normal as u8 + '@' as u8 - '!' as u8 - 1;
    match c {
//        '\0' ... '\x08' => TChar::Control as u8,
        '\t' | '\x0c' | '\x0b' | ' ' => TChar::Space as u8,
        '\r' | '\n' => TChar::Line as u8,
//        '\x10' ... '\x1f' => TChar::Control as u8,
        '!' ... '@' => TChar::Normal as u8 + (c as u8 - '!' as u8),
        'A' ... '`' => sym_end + (c as u8 - 'A' as u8),
        '{' ... '~' => sym_end + 32 + (c as u8 - '{' as u8),
        _ => TChar::Unknown as u8,
    }
}

fn trigrams_for<T: Iterator<Item=CharResult>>(input: T) -> Result<(), String> {
    let mut line: u64 = 1;
    let mut prev: [char; 3] = ['\0'; 3];
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
        prev[2] = c;
        let stred: String = prev.into_iter().collect();
        println!("{}", stred);
    }
    return Ok(());
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

    for i in 0..128u8 {
        println!("{} ({}) => {}", i, i as char, simplify(i as char));
    }

    if simple {
        let fh = fs::File::open(from).expect("input file must exist and be readable");
        let trigrams = trigrams_for(fh.chars()).expect("trigramming must work");
        return;
    }


    unimplemented!();

}
