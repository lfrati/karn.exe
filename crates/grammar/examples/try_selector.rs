//! Parse noun phrases given on the command line as selectors (and bare plural sets) and print
//! the outcome with the printed form (grammar debugging).
use grammar::kernel::Outcome;
use grammar::selector::selectors;
use oracle::token::tokenize;
fn main() {
    let s = selectors();
    for t in std::env::args().skip(1) {
        let toks = tokenize(&t);
        println!("{t}");
        match s.selector.parse_all(&toks) {
            Outcome::Accepted { value, .. } => {
                println!("  selector: {value:?}\n  printed:  {:?}", s.selector.print_text(&value, false))
            }
            other => println!("  selector: {other:?}"),
        }
        match s.set_pl.parse_all(&toks) {
            Outcome::Accepted { value, .. } => {
                println!("  plural set: {value:?}\n  printed:    {:?}", s.set_pl.print_text(&value, false))
            }
            Outcome::Declined { at, expected, .. } => println!("  plural set: declined at {at}: {expected:?}"),
            other => println!("  plural set: {other:?}"),
        }
    }
}
