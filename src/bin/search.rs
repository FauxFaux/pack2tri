extern crate regex_syntax;

use std::env;

use regex_syntax::Expr;


fn unpack(e: &Expr) {
    println!("unpacking: {}", e);
    match *e {
        Expr::Empty => println!("empty"),
        Expr::Group { ref e, i: _, name: _ } => {
            println!("group of..");
            unpack(&e);
        },
        Expr::Repeat { ref e, ref r, greedy } => {
            println!("{} repeat of {} ..", greedy, r);
            unpack(&e);
        },
        Expr::Concat(ref exprs) => {
            println!("{} different expressions ..", exprs.len());
            for expr in exprs {
                unpack(expr);
            }
        }

        Expr::Literal { ref chars, casei } => {
            println!("literal: {} ({})", chars.iter().collect::<String>(), casei);
        }

        ref other => println!("unimplemented: {}", other),
    };
    println!("done unpacking: {}", e);
}

fn main() {
    let regex = env::args().skip(1).next().expect("first arg: regex");
    println!("{}", regex);
    let e = Expr::parse(regex.as_str()).unwrap();
    unpack(&e)
}
