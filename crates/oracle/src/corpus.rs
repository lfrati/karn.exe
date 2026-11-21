//! Card faces as written by `scripts/fetch_resources.py` (`resources/oracle/cards.jsonl`).

use serde::Deserialize;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

/// One face of a card. Single-faced cards have one face; split, adventure, flip, transforming,
/// modal double-faced and meld cards have one per face (Scryfall's own split).
#[derive(Debug, Clone, Deserialize)]
pub struct Face {
    pub oracle_id: String,
    /// The card's full name ("Fire // Ice" for a split card).
    pub name: String,
    #[serde(default)]
    pub face: u8,
    pub face_name: String,
    #[serde(default = "normal_layout")]
    pub layout: String,
    pub mana_cost: Option<String>,
    pub type_line: String,
    #[serde(default)]
    pub oracle_text: String,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    #[serde(default)]
    pub colors: Option<Vec<String>>,
    pub color_indicator: Option<Vec<String>>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

fn normal_layout() -> String { "normal".into() }

/// The repository root this build was compiled from (see `build.rs`).
pub fn root() -> PathBuf {
    PathBuf::from(env!("KARN_ROOT"))
}

/// `resources/oracle/cards.jsonl` under the repository root.
pub fn default_path() -> PathBuf {
    root().join("resources/oracle/cards.jsonl")
}

/// The pinned Comprehensive Rules text under the repository root: the edition
/// `resources/manifest.json` names (`sources.rules.edition`, the one place it is written;
/// `scripts/pinned.py` derives the same file).
pub fn rules_path() -> io::Result<PathBuf> {
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root().join("resources/manifest.json"))?)?;
    let edition = manifest["sources"]["rules"]["edition"].as_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "resources/manifest.json names no sources.rules.edition"))?;
    Ok(root().join(format!("resources/rules/MagicCompRules-{edition}.txt")))
}

pub fn load(path: &Path) -> io::Result<Vec<Face>> {
    let file = std::fs::File::open(path)?;
    let mut faces = Vec::new();
    for (i, line) in io::BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let face: Face = serde_json::from_str(&line)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("line {}: {e}", i + 1)))?;
        faces.push(face);
    }
    Ok(faces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_faces_keep_zero_cost_distinct_from_no_mana_cost() {
        let base = r#"{"oracle_id":"id","name":"A","face_name":"A","type_line":"Creature"}"#;
        let missing: Face = serde_json::from_str(base).unwrap();
        assert_eq!(missing.face, 0);
        assert_eq!(missing.layout, "normal");
        assert!(missing.mana_cost.is_none());
        assert!(missing.oracle_text.is_empty());
        assert!(missing.keywords.is_empty());
        let explicit: Face = serde_json::from_str(
            r#"{"oracle_id":"id","name":"A // B","face_name":"B","type_line":"Creature",
                "face":1,"layout":"transform","mana_cost":"{0}","power":"0","color_indicator":["R"]}"#
        ).unwrap();
        assert_eq!(explicit.face, 1);
        assert_eq!(explicit.layout, "transform");
        assert_eq!(explicit.mana_cost.as_deref(), Some("{0}"));
        assert_eq!(explicit.power.as_deref(), Some("0"));
        assert_eq!(explicit.color_indicator, Some(vec!["R".to_owned()]));
    }
}
