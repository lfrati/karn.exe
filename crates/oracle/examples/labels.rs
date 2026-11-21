//! Lists flavor-word labels that start with a CR keyword or keyword action (normalizer check).
use oracle::{
    corpus, cr,
    normalize::{LabelKind, normalize},
};
fn main() {
    let faces = corpus::load(&corpus::default_path()).expect("corpus");
    for f in &faces {
        let n = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
        for l in n.labels.iter().flatten() {
            let folded = cr::fold(&l.text);
            let kw = cr::keyword_abilities()
                .iter()
                .chain(cr::keyword_actions().iter())
                .find(|k| folded.starts_with(&format!("{k} ")));
            if l.kind == LabelKind::FlavorWord && kw.is_some() {
                println!("{}: {}", f.face_name, l.text);
            }
        }
    }
}
