#![feature(io)]
extern crate bit_set;
extern crate regex_syntax;

use std::env;
use std::fmt;
use std::io;

use regex_syntax::Expr;

type Tri = u32;

mod tri;

#[derive(PartialEq, Eq, Hash, Debug)]
enum Op {
    And(Vec<Op>),
    Or(Vec<Op>),
    Any,
    Lit(Tri),
}

fn render_grams_in(vec: &Vec<Op>) -> String {
    let mut ret = String::with_capacity(vec.len() * 4);
    for item in vec {
        ret.push_str(format!("{} ", item).as_str());
    }
    // remove trailing space if possible
    ret.pop();
    ret
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Op::Lit(tri) => write!(f, "{}", tri::unpack(tri as usize)),
            Op::And(ref vec) => write!(f, "and({})", render_grams_in(vec)),
            Op::Or(ref vec) => write!(f, "or({})", render_grams_in(vec)),
            Op::Any => write!(f, "FUUUU"),
        }
    }
}

fn unpack(e: &Expr) -> Result<Op, String> {
    println!("unpacking: {}", e);
    match *e {
        Expr::Group { ref e, i: _, name: _ } => {
            println!("group of..");
            unpack(&e)
        },
        Expr::Repeat { ref e, ref r, greedy } => {
            use regex_syntax::Repeater;
            println!("{} repeat of {} ..", greedy, r);
            match *r {
                Repeater::ZeroOrOne => Ok(Op::Any),
                Repeater::ZeroOrMore => Ok(Op::Any),
                Repeater::Range { min: 0, max: _ } => Ok(Op::Any),
                _ => unpack(&e)
            }
        },
        Expr::Concat(ref exprs) => {
            println!("{} different expressions ..", exprs.len());
            let maybe: Result<Vec<Op>, String> = exprs.iter().map(unpack).collect();
            Ok(Op::And(maybe?))
        },
        Expr::Alternate(ref exprs) => {
            println!("{} alternate expressions ..", exprs.len());
            let maybe: Result<Vec<Op>, String> = exprs.iter().map(unpack).collect();
            Ok(Op::Or(maybe?))
        }

        Expr::Literal { ref chars, casei } => {
            let lit = chars.iter().collect::<String>();
            if casei {
                return Err(format!("unsupported: case insensitive matching on '{}'", lit));
            }
            println!("literal: {} ({})", lit, casei);

            Ok(Op::And(tri::trigrams_for(chars.iter().map(|c| Ok::<char, io::CharsError>(*c)))?
                         .iter().map(|gram| Op::Lit(gram as u32)).collect()))
        }

        ref other => Err(format!("unimplemented: {}", other)),
    }
}

fn main() {
    let regex = env::args().skip(1).next().expect("first arg: regex");
    println!("{}", regex);
    let e = Expr::parse(regex.as_str()).unwrap();
    println!("{}", unpack(&e).unwrap());
}
