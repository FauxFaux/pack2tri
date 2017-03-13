#![feature(io)]

extern crate argparse;
extern crate compress;


use std::fs;
use std::io;

use argparse::{Store, StoreTrue};

//magic:
use std::io::Read;

fn trigrams_for<T: Iterator<Item=Result<char, io::CharsError>>>(input: T) -> Result<(), String> {
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

    if simple {
        let fh = fs::File::open(from).expect("input file must exist and be readable");
        let trigrams = trigrams_for(fh.chars()).expect("trigramming must work");
        return;
    }


    unimplemented!();

}
