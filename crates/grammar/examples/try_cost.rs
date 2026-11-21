//! Parse costs given on the command line and print the outcome (grammar debugging).
use grammar::cost::costs;
use oracle::token::tokenize;
fn main() {
    for t in std::env::args().skip(1) {
        let toks = tokenize(&t);
        println!("{t}: {:?}", costs().cost.parse_all(&toks));
    }
}
