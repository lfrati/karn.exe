//! Lists self-references spelled as a legendary card's shortened name without a comma.
use oracle::{corpus, normalize::normalize};
fn main() {
    let faces = corpus::load(&corpus::default_path()).expect("corpus");
    for f in &faces {
        let n = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
        let mut seen = std::collections::BTreeSet::new();
        for r in n.self_refs() {
            let comma = [", ", " the "].iter().any(|s| f.face_name.find(s).is_some_and(|at| &f.face_name[..at] == r));
            if r != &f.face_name
                && r != &f.name
                && !r.starts_with("this ")
                && !r.starts_with("This ")
                && !comma
                && seen.insert(r.clone())
            {
                println!("{} | {}", f.face_name, r);
            }
        }
    }
}
