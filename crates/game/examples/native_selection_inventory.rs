//! Diagnostic inventory, not a support or correctness claim. Emits parsed selectors,
//! selection actions and their owning IR paths for every pinned Standard face.
use game::CardDef;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn walk(value: &Value, path: &str, out: &mut Vec<Value>) {
    match value {
        Value::Object(fields) => {
            for (key, child) in fields {
                let next = format!("{path}/{key}");
                if matches!(key.as_str(), "Target" | "Indefinite" | "Scry" | "Surveil" | "Search" | "Discard" | "Sacrifice" | "slice" | "WhereX") && !child.is_null() {
                    out.push(json!({"path": next, "kind": key, "value": child}));
                }
                walk(child, &next, out);
            }
        }
        Value::Array(items) => for (i, child) in items.iter().enumerate() { walk(child, &format!("{path}/{i}"), out); },
        _ => {}
    }
}
fn main() {
    let root = oracle::corpus::root();
    let legal: BTreeSet<String> = std::fs::read_to_string(root.join("resources/oracle/legal-standard.jsonl")).unwrap().lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["oracle_id"].as_str().unwrap().to_owned()).collect();
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    for face in faces.iter().filter(|f| legal.contains(&f.oracle_id)) {
        let mut row = json!({"oracle_id": face.oracle_id, "face": face.face, "name": face.face_name, "oracle_text": face.oracle_text});
        match CardDef::from_face(face) {
            Err(error) => { row["parse_error"] = json!(error); }
            Ok(def) => {
                row["face_admission"] = json!(game::play::check(&def).map_err(|e| e.0));
                let mut consumers = Vec::new();
                for (i, ability) in def.abilities().iter().enumerate() {
                    walk(&serde_json::to_value(ability).unwrap(), &format!("ability/{i}"), &mut consumers);
                    walk(&serde_json::to_value(def.modes_of(i)).unwrap(), &format!("modes/{i}"), &mut consumers);
                }
                row["consumers"] = json!(consumers);
                // Retain compiled native lowering for targeted follow-up, without adding
                // a public engine API just for this diagnostic.
                if face.face_name == "Spider-Man Noir" || face.face_name == "Prismabasher" || face.face_name == "Cerebral Download" || face.face_name == "Meathook Massacre II" {
                    row["definition_debug"] = json!(format!("{def:?}"));
                }
            }
        }
        println!("{row}");
    }
}
