//! Same-source triggers retain different subjects and are independently ordered.
use crate::support::*;
use game::state::Decision;

// Rules: 603.2c, 603.3b, 608.2k, 608.2c
// Oracle: Tribute to the World Tree draws for power >=3, otherwise puts two
// +1/+1 counters on the entering creature; Raise the Alarm creates two 1/1s.
// Each token triggers Tribute separately. The controller chooses which trigger
// is on top, and only its particular token gets counters after one resolution.
#[test]
fn r603_3b_same_source_triggers_bind_different_entrants_and_resolve_in_chosen_order() {
    let mut t=Board::new(2).battlefield(0,&["Tribute to the World Tree"])
        .hand(0,&["Raise the Alarm"]).build();
    t.mana(0,"{1}{W}").cast(0,"Raise the Alarm",vec![], crate::support::mana_pool("CW")).unwrap();
    t.g.act(t.g.priority,Move::Pass).unwrap();
    t.g.act(t.g.priority,Move::Pass).unwrap();
    let Some(Decision::Order{player:0,abilities})=t.g.decision.clone() else {panic!("expected separate Tribute occurrences: {:?}",t.g.decision)};
    cite!(["603.2c","603.3b"]; assert_eq!(abilities.len(),2));
    assert_eq!(abilities[0].source,abilities[1].source);
    let audit=t.g.audit_pending_trigger_order();
    assert_eq!(t.g.audit_game_objects()["pending_triggers"],audit["occurrences"]);
    let occurrences=audit["occurrences"].as_array().unwrap();
    assert_eq!(occurrences.len(),2);
    let entrants:Vec<_>=occurrences.iter().map(|occurrence| {
        assert!(occurrence.get("unavailable").is_none(),"{occurrence}");
        assert_eq!(occurrence["controller"],0);
        let inputs=occurrence["bound"]["inputs"].as_array().unwrap();
        assert!(inputs.iter().any(|input|input["kind"]=="entry_controller" && input["subjects"]==serde_json::json!([{"player":0}])));
        let subject=inputs.iter().find(|input|input["kind"]=="subject").unwrap();
        game::ObjId(subject["subjects"][0]["object"].as_u64().unwrap())
    }).collect();
    assert_ne!(entrants[0],entrants[1]);
    for order in [vec![0,1],vec![1,0]] {
        let top=*order.last().unwrap();
        let mut game=t.g.clone();
        game.act(0,Move::Order(order.clone())).unwrap();
        assert_eq!(game.stack.len(),2);
        let stacked=game.audit_game_objects();
        for (object,selected) in game.stack.iter().zip(&order) {
            let bound=&stacked["objects"][object.0.to_string()]["occurrences"][0]["bound"];
            let inputs=bound["inputs"].as_array().expect("verified stack bindings");
            let subject=inputs.iter().find(|input|input["kind"]=="subject").unwrap();
            cite!(["603.3b","608.2k"]; assert_eq!(subject["subjects"][0]["object"],entrants[*selected].0));
        }
        game.act(game.priority,Move::Pass).unwrap();
        game.act(game.priority,Move::Pass).unwrap();
        cite!(["608.2k","608.2c"]; assert_eq!(game.counters(entrants[top]).get("+1/+1"),Some(&2)));
        assert_eq!(game.counters(entrants[1-top]).get("+1/+1"),None);
        assert_eq!(game.stack.len(),1);
    }
}
