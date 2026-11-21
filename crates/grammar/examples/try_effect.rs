//! Parse effect sentences given on the command line and print the outcome and the reprint
//! (grammar debugging).
use grammar::effect::effects;
use oracle::token::tokenize;
fn main() {
    for t in std::env::args().skip(1) {
        let toks = tokenize(&t);
        let e = &effects().sentence;
        let out = e.parse_all(&toks);
        let printed = match &out {
            grammar::kernel::Outcome::Accepted { value, .. } => e.print_text(value, true),
            _ => None,
        };
        println!("{t}\n  {out:?}\n  printed: {printed:?}");
    }
}
