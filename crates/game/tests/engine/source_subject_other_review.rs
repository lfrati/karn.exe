//! Which object a root "other"
//! excludes when a permanent's ability first has its source act on a target: the source,
//! beside the target-relative contrasts that must stay. Run
//! with `engine::source_subject_other_review::`.
use crate::support::cite;
use game::state::Decision;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn bear(name: &str, p: i32) -> CardDef { CardDef::new(name, "{0}", "Creature — Bear", "", Some((p, 9))).unwrap() }
fn probe(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Creature — Bear", text, Some((1, 5))).unwrap() }

struct Run { g: Game, probe: ObjId, mine: ObjId, theirs: ObjId }
impl Run {
    fn dealt(&self) -> Vec<(ObjId, Target, u32)> {
        self.g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect()
    }
    fn pt(&self, o: ObjId) -> (i32, i32) { (self.g.power(o).unwrap(), self.g.toughness(o).unwrap()) }
}

/// Probe 1/5 (with `text`), my Mine 3/9 and the opponent's Theirs 2/9. 'P' targets the
/// opponent, 'T' Theirs and 'M' Mine. A triggered probe is cast; an activated one (`{T}: …`)
/// starts on the battlefield and is activated.
fn run(text: &str, target: char) -> Run {
    let d = probe(text);
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, bear("Mine", 3), bear("Theirs", 2)], 2, 20);
    let (mine, theirs) = (g.put(0, 1, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    let target = match target { 'P' => Target::Player(1), 'T' => Target::Object(theirs), _ => Target::Object(mine) };
    let probe = if text.starts_with("{T}") {
        let s = g.put(0, 0, Zone::Battlefield);
        g.act(0, Move::Activate { source: s, ability: 0, targets: vec![vec![target]], paying: vec![] }).unwrap();
        s
    } else {
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
        c
    };
    for _ in 0..8 {
        if let Some(Decision::Targets { .. }) = g.decision { g.act(0, Move::ChooseTargets(vec![vec![target]])).unwrap(); continue; }
        if g.stack.is_empty() { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    let probe = g.battlefield.iter().copied().find(|o| g.obj(*o).owner == 0 && *o != mine).unwrap_or(probe);
    Run { g, probe, mine, theirs }
}

/// The source dealt 1 to the target and is "other"'s antecedent, so only Mine
/// deals the second 1. The same holds without any trigger input: the body names the source
/// ("this creature deals") or the ability is activated. So a proof based only on the own
/// trigger input (Named.own / Input::Subject) is not enough.
#[test]
fn source_that_acted_is_not_one_of_each_other() {
    for text in ["When this creature enters, it deals 1 damage to any target. Each other creature you control deals 1 damage to it.",
                 "When this creature enters, this creature deals 1 damage to any target. Each other creature you control deals 1 damage to it.",
                 "{T}: This creature deals 1 damage to any target. Each other creature you control deals 1 damage to it."] {
        let r = run(text, 'P');
        // Uncited (card text): "Each other creature you control deals 1 damage to it": other than this creature
        assert_eq!(r.dealt(), vec![(r.probe, Target::Player(1), 1), (r.mine, Target::Player(1), 1)], "{text}");
        cite!(["120.3a"]; case: text; assert_eq!(r.g.players[1].life, 18, "{text}"));
    }
}

/// The same rule for a continuous change. The source dealt damage to the
/// opponent's creature; "each other creature you control" is everything but the source, so
/// only Mine grows (the probe ends 1/5, not 2/6).
#[test]
fn source_that_acted_is_not_other_for_a_continuous_change() {
    for text in ["When this creature enters, it deals 1 damage to target creature. Each other creature you control gets +1/+1 until end of turn.",
                 "{T}: This creature deals 1 damage to target creature. Each other creature you control gets +1/+1 until end of turn."] {
        let r = run(text, 'T');
        // Uncited (card text): "Each other creature you control gets +1/+1": other than this creature
        assert_eq!((r.pt(r.probe), r.pt(r.mine), r.pt(r.theirs)), ((1, 5), (4, 10), (2, 9)), "{text}");
    }
}

/// Contrast: a target that is the subject of the clause before "other" stays its antecedent, as
/// on Phalanx Tactics, even after the trigger has said "it" of the source. An own-source
/// occurrence alone must not take precedence: the probe gets both bonuses (3/7) and Mine only
/// its own +1/+2 (4/11).
#[test]
fn target_subject_stays_the_antecedent_after_an_own_mention() {
    let r = run("When this creature enters, it gets +1/+1 until end of turn. Target creature you control gets +1/+2 until end of turn. Each other creature you control gets +1/+1 until end of turn.", 'M');
    // Uncited (card text): "Target creature you control gets +1/+2 ... Each other creature you control gets +1/+1": other than the target
    assert_eq!((r.pt(r.probe), r.pt(r.mine)), ((3, 7), (4, 11)));
    let r = run("When this creature enters, target creature you control gets +1/+2 until end of turn. Each other creature you control gets +1/+1 until end of turn.", 'M');
    // Uncited (card text): as above, "Each other creature you control" is other than the target
    assert_eq!((r.pt(r.probe), r.pt(r.mine)), ((2, 6), (4, 11)));
}

/// Contrast: the member-amount guard still refuses the player-capable recipient.
#[test]
fn member_amount_to_a_mixed_recipient_stays_refused() {
    let d = probe("When this creature enters, it deals 1 damage to any target. Each other creature you control deals damage equal to its power to it.");
    assert_eq!(game::play::check(&d).map_err(|e| e.0), Err("per-source object quantity in damage a player may be dealt".into()));
}
