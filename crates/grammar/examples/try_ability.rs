//! Parse ability rows given on the command line and print the outcome (grammar debugging).
use grammar::ability::abilities;
use oracle::token::tokenize;
fn main() {
    for t in std::env::args().skip(1) {
        let toks = tokenize(&t);
        let a = &abilities().ability;
        let out = a.parse_all(&toks);
        let printed = match &out {
            grammar::kernel::Outcome::Accepted { value, .. } => a.print_text(value, true),
            _ => None,
        };
        println!("{t}: {out:?}\\n  printed: {printed:?}");
    }
}
