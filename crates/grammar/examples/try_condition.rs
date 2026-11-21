//! Parse conditions given on the command line and print the outcome (grammar debugging).
use grammar::condition::conditions;
use oracle::token::tokenize;
fn main() {
    for t in std::env::args().skip(1) {
        let toks = tokenize(&t);
        let c = &conditions().condition;
        let out = c.parse_all(&toks);
        let printed = match &out {
            grammar::kernel::Outcome::Accepted { value, .. } => c.print_text(value, false),
            _ => None,
        };
        println!("{t}\n  {out:?}\n  printed: {printed:?}");
    }
}
