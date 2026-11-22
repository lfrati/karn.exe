//! Instruction execution against retained frames. Syntax lowering lives in `compile`.
//! Mutations and suspension continue through their existing operation owners.
use super::*;

fn offer(game: &Game, f: &mut Frame, attempt: Option<&Attempt>, created: Option<Slot>, make: impl FnOnce(&Game, &Frame) -> Res<Request>) -> Result<Option<Request>, Halt> {
    let (mut request, players, mut accepted, mut cursor, mut answer) = match f.pending.take() {
        Some(Pending::Attempt { request, players, accepted, cursor, answer }) => (*request, players, accepted, cursor, answer),
        None => (make(game, f)?, attempt.map(|a| action::players(game, f, &a.actor)).unwrap_or_default(), Vec::new(), 0, None),
        _ => unreachable!("ordinary action resumes its own decision"),
    };
    if let Some(attempt) = attempt {
        let possible = request.possible(game, attempt.payment || attempt.cost);
        if possible && attempt.optional {
            while cursor < players.len() {
                match answer.take() {
                    Some(answer) => {
                        let player = players[cursor];
                        let yes = if let Some(count) = &attempt.count {
                            let Answer::Value(Value::Number(n)) = answer else { unreachable!("play validates numeric answers") };
                            request.draw_count(player, n);
                            f.slots[count.out] = Some(Value::Number(n));
                            // 101.4b: this answer is public before the next player chooses.
                            f.choices.push(crate::state::PublicChoice::Value {
                                player, source: f.ctx.object_source(), ability: f.ctx.ability.clone(),
                                slot: count.out, value: Value::Number(n),
                            });
                            n > 0
                        } else { answer == Answer::Yes };
                        if yes { accepted.push(player); }
                        cursor += 1;
                    }
                    None => {
                        let player = players[cursor];
                        f.pending = Some(Pending::Attempt { request: Box::new(request), players, accepted, cursor, answer: None });
                        return Err(Halt::Ask(Decision::Choose { player, choice: attempt.count.as_ref().map_or(Choice::May { possible: true }, |c| Choice::Number { min: 0, max: c.max }) }));
                    }
                }
            }
        }
        let paid = possible && (!attempt.optional || !accepted.is_empty());
        f.slots[attempt.paid] = Some(Value::Consent(paid));
        f.slots[attempt.possible] = Some(Value::Consent(possible));
        // what is done only in part isn't done (118.12, 603.12), but an instruction is
        // followed as far as it can be (609.3, 701.17b): only a choice not taken, or a cost
        // that can't be paid in full (118.3), does nothing
        if (attempt.optional || attempt.cost) && !paid {
            if let Some(out) = created { f.slots[out] = Some(f.action_output().empty()); }
            return Ok(None);
        }
        if attempt.optional && attempt.distributed { request.retain_players(&accepted); }
    }
    Ok(Some(request))
}

/// Is this card of a group still where its instruction put it: in the group's zone, or, as
/// the object it became, in the public zone a replacement put it into instead (400.7j,
/// 701.17c)? One that has left since is a new object, and is not (400.7).
fn still_there(game: &Game, card: ObjId, zone: Zone) -> bool {
    let public = |z: Zone| matches!(z, Zone::Battlefield | Zone::Graveyard | Zone::Stack | Zone::Exile | Zone::Command);
    let now = game.obj(card).zone;
    now == zone || public(zone) && public(now)
}

pub(super) fn step(game: &mut Game, f: &mut Frame, i: &Instr) -> Result<(), Halt> {
    match i {
        Instr::Reflexive { when, each, program } => {
            let each_one: Vec<Option<(Slot, ObjId)>> = match each {
                Some(each) => each.members.members_counted(game, &f.ctx, f).into_iter().map(|o| Some((each.member, o))).collect(),
                None => if when.evaluate(game, &f.ctx, f) { vec![None] } else { Vec::new() },
            };
            for one in each_one {
                let mut values = f.slots.clone();
                if let Some((member, o)) = one {
                    if values.len() <= member { values.resize(member + 1, None); }
                    values[member] = Some(Value::Subjects(vec![Target::Object(o)]));
                }
                let captured = Captured { ctx: f.ctx.clone(), body: Body::Plain(program.clone()), intervening: None, triggered: true, chapter: false, values, keyword_producer: None, granted_keyword: None, sparse_checked: false };
                let bound = crate::state::Bound { inputs: f.inputs.clone(), event_amount: f.event_amount, became: f.transitions.clone(), linked_ability: f.ctx.ability.clone(), ..Default::default() };
                game.triggered.push(crate::state::Triggered { source: f.ctx.source, controller: f.ctx.controller,
                    origin: f.ctx.ability.clone().map_or(crate::binding::Origin::Own, |key| key.origin), index: 0,
                    bound, delayed: Some(captured.identity()), captured: Some(Ok(captured)) });
            }
        }
        Instr::Delayed { pattern, subjects, program, lasts, once } => {
            let subjects = subjects.iter().flat_map(|s| action::found(game, f, s)).collect();
            let captured = Captured { ctx: f.ctx.clone(), body: Body::Plain(program.clone()), intervening: None, triggered: true, chapter: false, values: f.slots.clone(), keyword_producer: None, granted_keyword: None, sparse_checked: false };
            let bound = crate::state::Bound { inputs: f.inputs.clone(), event_amount: f.event_amount, became: f.transitions.clone(), linked_ability: f.ctx.ability.clone(), ..Default::default() };
            game.delayed.push(crate::state::Delayed { source: f.ctx.object_source(), controller: f.ctx.controller, pattern: pattern.clone(), subjects, captured, bound, lasts: *lasts, once: *once });
        }
        Instr::Place { objects, position, chooser, order, attempt, by, before_shuffle } => {
            let cards = if matches!(f.pending, Some(Pending::Library(_))) { Vec::new() } else {
                let Some(Request::Place(cards)) = offer(game,f,attempt.as_ref(),None,|game,f|Ok(Request::Place(action::objects(game,f,objects))))? else { return Ok(()) };
                cards
            };
            // an optional target player told to put them who was not chosen puts nothing
            // (601.2c, 608.2b)
            let by = match by { Some(who) => match f.actor(*who) { Some(player) => Some(player), None => return Ok(()) }, None => None };
            let (position, destination) = match position {
                // refused when the card is compiled
                ir::Position::BeneathTopX => return Err(Halt::Unsupported(crate::Unsupported("library destination: beneath the top X cards".into()))),
                ir::Position::One(position) => (*position, None),
                ir::Position::Choice { between, .. } => {
                    let chooser = match chooser {
                        Some(chooser) => match action::players(game, f, chooser).as_slice() {
                            [player] => Some(*player),
                            [] => return Ok(()),
                            _ => return Err(Halt::Unsupported(crate::Unsupported("library destination requires one chooser".into()))),
                        },
                        None => None,
                    };
                    (between[0], Some((*between, chooser)))
                },
            };
            place(game, f, cards, position, *order, by, destination, *before_shuffle)?;
        }
        Instr::Search { action, by, whose, own, out } => {
            // what is found is seen by who found it from then on, while the others search
            // (701.23e, 701.23i)
            if f.pending.is_none() { f.shown.push((*out, if *own { Audience::Owners } else { Audience::Only(*by) })); }
            let mut pending = match f.pending.take() {
                Some(Pending::Search(pending)) => pending,
                // each of these players, in APNAP order, their own library (701.23i); one who
                // has left the game searches nothing (800.4a)
                None if *own => action.begin(whose.as_ref().map_or_else(Vec::new, |players| action::players(game, f, players))
                    .into_iter().filter(|p| !game.players[*p].lost).map(|p| (p, p)).collect()),
                None => {
                    let searcher = f.player(*by);
                    action.begin(whose.as_ref().map_or_else(|| vec![searcher], |players| action::players(game, f, players))
                        .into_iter().map(|p| (searcher, p)).collect())
                }
                _ => unreachable!("instruction resumes its own procedure"),
            };
            // the player looks through each library as its search begins (701.23a)
            if let Some(cards) = pending.open(action, game, f) {
                game.disclose(crate::observation::Kind::Looked, pending.player(), &cards);
            }
            f.slots[*out] = Some(Value::Subjects(pending.result().iter().map(|o| Target::Object(*o)).collect()));
            if let Some(decision) = pending.decision() {
                f.pending = Some(Pending::Search(pending));
                return Err(Halt::Ask(decision));
            }
        }
        Instr::Shuffle { players, found } => {
            let found = found.map(|s| action::objects(game, f, &[action::Operand::Reference(s)])).unwrap_or_default();
            let known = f.shown.iter().flat_map(|(s, _)| match f.slot(*s) {
                Some(Value::Cards(cards)) => cards.clone(),
                Some(Value::Subjects(subjects)) => subjects.iter().filter_map(|t| if let Target::Object(o) = t { Some(*o) } else { None }).collect(),
                _ => Vec::new(),
            }).collect::<Vec<_>>();
            for player in action::players(game, f, players) { game.shuffle_except(player, &found, &known); }
        }
        Instr::Gather { objects, out } => {
            f.slots[*out] = Some(Value::Cards(action::moved(game, f, std::slice::from_ref(objects))));
        }
        Instr::Observe { objects, by, out } => {
            let cards = action::objects(game, f, std::slice::from_ref(objects));
            let shown = game.reveal(f.player(*by), cards);
            f.slots[*out] = Some(Value::Cards(shown));
            f.shown.push((*out, Audience::Everyone));
        }
        Instr::Subjects { cards, out, still_in } => f.slots[*out] = Some(Value::Subjects(f.cards(*cards).into_iter().filter(|card| still_in.is_none_or(|zone| still_there(game, *card, zone))).map(Target::Object).collect())),
        Instr::Proliferate { by } => {
            let Some(player) = f.actor(*by).filter(|p| !game.players[*p].lost) else { return Ok(()); };
            let among = (0..game.players.len()).map(|i| Target::Player((game.active + i) % game.players.len()))
                .chain(game.battlefield.iter().copied().map(Target::Object))
                .filter(|t| game.holds_counters(*t) && game.counters(*t).values().any(|n| *n > 0))
                .collect::<Vec<_>>();
            if among.is_empty() { return Ok(()); }
            let Answer::Holders(chosen) = f.ask(player, Choice::Holders { verb: Verb::Proliferate, among: among.clone() })? else {
                unreachable!("play validates holder answers before dispatch")
            };
            // An answer is a set: execution follows canonical offer/kind order.
            let batch = among.into_iter().filter(|t| chosen.contains(t)).flat_map(|on| {
                game.counters(on).iter().filter(|(_,n)| **n > 0).map(move |(kind,_)| crate::operation::Placement { on, kind: kind.clone(), count: 1, by: player })
            }).collect::<Vec<_>>();
            if batch.is_empty() { return Ok(()); }
            match crate::perform::run(game, crate::perform::Action::PutCounters(batch))? {
                Ok(result) => f.operation_result(game,result, None),
                Err(decision) => {
                    f.pending = Some(Pending::Operation { result: None, created: None });
                    return Err(Halt::Ask(decision));
                }
            }
        }
        Instr::Connive { objects } => {
            let mut pending=match f.pending.take(){Some(Pending::Connive(p))=>p,None=>connive::Pending::new(game,f,objects),_=>unreachable!("connive resumes its own operation")};
            if let Some(decision)=pending.advance(game,f)? {f.pending=Some(Pending::Connive(pending));return Err(Halt::Ask(decision));}
        }
        Instr::Explore { objects } => {
            let mut pending=match f.pending.take(){Some(Pending::Explore(p))=>p,None=>explore::Pending::new(game,f,objects),_=>unreachable!("explore resumes its own operation")};
            if let Some(decision)=pending.advance(game,f)? {f.pending=Some(Pending::Explore(pending));return Err(Halt::Ask(decision));}
        }
        Instr::Arrange { players, verb, amount, optional } => {
            let mut pending = match f.pending.take() {
                Some(Pending::Arrange(procedure)) => procedure,
                None => {
                    let amount = amount.checked(game, &f.ctx, f)?;
                    arrange::Pending::new(game, action::players(game, f, players), *verb, amount).optional(*optional)
                },
                _ => unreachable!("instruction resumes its own procedure"),
            };
            if let Some(decision) = pending.advance(game)? {
                // each player looks at their cards as they are asked where those go
                // (701.22a, 701.25a): asked once, since only a valid answer comes back here
                if let Decision::Choose { player, choice: Choice::Arrange { cards, .. } } = &decision {
                    game.disclose(crate::observation::Kind::Looked, *player, cards);
                }
                f.pending = Some(Pending::Arrange(pending));
                return Err(Halt::Ask(decision));
            }
        }
        Instr::Choice { action, optional, payment, paid, possible, chosen, outcomes, actual, discarded_before } => {
            let (mut pending, published) = match f.pending.take() {
                Some(Pending::Objects { procedure, published }) => (procedure, published),
                None => (choice::Pending::new(game, f, action).offer(*optional, *payment), 0),
                _ => unreachable!("instruction resumes its own procedure"),
            };
            let decision = pending.advance(game)?;
            let public = pending.public();
            f.choices.extend_from_slice(&public[published..]);
            if let Some(decision) = decision {
                f.pending = Some(Pending::Objects { procedure: pending, published: public.len() });
                return Err(Halt::Ask(decision));
            }
            f.transitions.extend_from_slice(pending.transitions());
            if let Some(out) = actual { f.slots[*out] = Some(Value::Cards(pending.actual().to_vec())); }
            if let Some(out) = discarded_before { f.slots[*out] = Some(Value::Cards(pending.discarded_before())); }
            if let Some(outputs) = outcomes {
                for (slot, group) in outputs.iter().zip(pending.outcomes()) { f.slots[*slot] = Some(Value::Subjects(group)); }
            }
            f.slots[*paid] = Some(Value::Consent(pending.started()));
            f.slots[*possible] = Some(Value::Consent(pending.possible()));
            if let Some(out) = chosen { f.slots[*out] = Some(Value::Subjects(pending.chosen().into_iter().map(Target::Object).collect())); }
        }
        Instr::Input { input, out } => {
            let subjects = f.inputs.iter().find(|(key, _)| key == input)
                .expect("occurrence supplies every input guaranteed by its compiled schema").1.clone();
            f.slots[*out] = Some(Value::Subjects(subjects));
        }
        Instr::Capture { operand, out } => f.slots[*out] = Some(Value::Subjects(action::found(game, f, operand))),
        Instr::ControlTurn { player, out } => {
            let player=action::players(game,f,player).first().copied();
            f.slots[*out]=Some(Value::Turn(player.map(|player|game.control_next_turn(player,f.ctx.controller))));
        }
        Instr::ExtraTurns { players, count, after,out } => {
            let anchor=after.map(|slot| match f.slot(slot) { Some(Value::Turn(anchor))=>*anchor, _=>unreachable!("verified turn anchor") });
            let mut turns=Vec::new();
            if anchor != Some(None) {
                let count=count.checked(game,&f.ctx,f)?;
                for player in action::players(game,f,players) {
                    for _ in 0..count { if let Some(id)=game.extra_turn(player,anchor.flatten()) { turns.push(id); } }
                }
            }
            f.slots[*out]=Some(Value::CreatedTurns(turns));
        }
        Instr::RuleDuringCreatedTurns { turns,rule } => {
            let Some(Value::CreatedTurns(turns))=f.slot(*turns) else { unreachable!("verified created-turn slot") };
            for id in turns { game.rule_during_created_turn(*id,rule.clone()); }
        }
        Instr::SkipTurns { players,count } => {
            let count=count.checked(game,&f.ctx,f)?;
            let players=action::players(game,f,players);
            for _ in 0..count { game.add_skip(&players,crate::turn::Occurrence::Turn,false,f.ctx.source,f.ctx.ability.clone()); }
        }
        Instr::Skip { players, what, this_turn } => {
            let players = action::players(game, f, players);
            game.add_skip(&players, *what, *this_turn, f.ctx.source, f.ctx.ability.clone());
        }
        Instr::AddPhases { after, kinds, count } => {
            let count = count.checked(game, &f.ctx, f)?;
            game.add_phases(*after, kinds, count)?;
        }
        Instr::Copy { objects, from, turn } => {
            let affected = action::objects(game, f, objects);
            if let [Target::Object(source)] = action::found(game, f, from).as_slice() {
                let values = game.copiable(*source);
                game.become_copy(affected, values, *turn);
            }
        }
        Instr::Continuous { objects, changes, each, duration } => {
            let times = each.as_ref().map_or(Ok(1), |n| n.checked(game, &f.ctx, f))?;
            let changes = changes.iter().map(|c| c.evaluate(game, &f.ctx, f).checked_times(times)).collect::<Res<Vec<_>>>()?;
            let affected = action::objects(game, f, objects);
            // Freeze proven plain printed provenance now, never from a later face/copy.
            let producer = f.ctx.source.and_then(|source| {
                let object=game.obj(source);
                let definition=object.card?;
                let def=&game.defs[definition];
                let key=f.ctx.ability.as_ref()?;
                if !matches!(key.origin,crate::binding::Origin::Own) || !matches!(game.copy_origin(source),crate::binding::Origin::Own)
                    || object.copy_acquisition.is_some() || game.face_down(source)
                    || def.back.is_some() || !def.halves.is_empty() || def.is_room()
                    || !same_instance(&definition_program(def,key.index)?.ok()?,&f.program) { return None; }
                Some(std::sync::Arc::new(crate::layers::ResolvedProducer {source,definition,
                    paragraph:def.ability_row(key.index)+1,controller:f.ctx.controller}))
            });
            game.begin_effect_from(changes, affected, duration.evaluate(&f.ctx), producer);
        }
        Instr::Register { subject, rule, lifetime } => {
            fn affected(game: &Game, f: &Frame, subject: &action::Operand) -> crate::statement::Affected {
                use crate::statement::Affected;
                match subject {
                    action::Operand::All(set) => Affected::Compiled { set: set.clone(), locals: f.slots.clone() },
                    action::Operand::Union(items) => Affected::Union(items.iter().map(|s| affected(game, f, s)).collect()),
                    other => Affected::Things(action::found(game, f, other)),
                }
            }
            game.keep(f.ctx.clone(), affected(game, f, subject), rule.clone(), *lifetime, f.slots.clone());
        }
        Instr::Mana { players, production } => {
            let players = if f.pending.is_none() { action::players(game, f, players) } else { Vec::new() };
            mana::to_players(game, f, production, players)?;
        }
        Instr::Branch { condition, end, remember, skipped_values } => {
            let held = condition.evaluate(game, &f.ctx, f);
            if let Some(slot) = remember { f.slots[*slot] = Some(Value::Consent(held)); }
            if !held {
                for (slot, value) in skipped_values { f.slots[*slot] = Some(value.clone()); }
                f.pc = end - 1;
            }
        }
        Instr::SetX(amount) => f.ctx.x = Some(amount.checked(game, &f.ctx, f)?),
        // The next time through names its own cards. A card revealed before stays revealed:
        // it was shown to everyone, and the process moved it to a public zone.
        Instr::Repeat { locals, .. } => {
            for slot in locals { f.slots[*slot] = None; }
            f.shown.retain(|(slot, _)| !locals.contains(slot));
        }
        Instr::Permit { player, cards, play, until } => {
            // the card as the effect left it in exile, and only there (400.7)
            let cards: Vec<ObjId> = action::moved(game, f, cards).into_iter().filter(|now| game.obj(*now).zone == Zone::Exile).collect();
            for player in action::players(game, f, player) {
                for card in &cards { game.permit_until(player, *card, *play, *until); }
            }
        }
        Instr::ChangeTargets { object, policy } => {
            let choice = match &f.pending {
                Some(Pending::Answer { decision: Decision::Choose { choice: choice @ Choice::ChangeTargets { .. }, .. }, .. }) => choice.clone(),
                None => {
                    let Some(object) = action::objects(game, f, std::slice::from_ref(object)).first().copied() else { return Ok(()); };
                    let Some(choice) = game.change_target_offer(object, *policy) else { return Ok(()); };
                    choice
                }
                _ => unreachable!("target change resumes its retained question"),
            };
            let Choice::ChangeTargets { object, original, .. } = &choice else { unreachable!() };
            let (object, original)=(*object,original.clone());
            match f.ask(f.ctx.controller, choice)? {
                Answer::Targets(targets) => game.commit_changed_targets(object, &original, targets),
                Answer::No => {},
                _ => unreachable!("target-change answer validated before dispatch"),
            }
        }
        Instr::CopyStack { cards, times, retarget, out } => {
            let mut pending = match f.pending.take() {
                Some(Pending::Copies(pending)) => pending,
                None => {
                    let times = times.checked(game, &f.ctx, f)?;
                    crate::copy::StackCopies::new(action::named(game, f, cards).into_iter().filter_map(|t| if let Target::Object(o) = t { Some(o) } else { None }).collect(), times, f.ctx.controller, *retarget)
                },
                _ => unreachable!("spell copy owns its target choice"),
            };
            match pending.advance(game) {
                Ok(copies) => f.slots[*out] = Some(Value::Subjects(copies.into_iter().map(Target::Object).collect())),
                Err(decision) => { f.pending = Some(Pending::Copies(pending)); return Err(Halt::Ask(decision)); }
            }
        }
        Instr::CopyCards { cards, times, out } => {
            let times = times.checked(game, &f.ctx, f)?;
            let player = f.ctx.controller;
            let mut copies = Vec::new();
            // the card where the effect left it: a card that is gone, or a spell or permanent
            // it has become, is not what was chosen (400.7)
            for card in action::moved(game, f, std::slice::from_ref(cards)) {
                if matches!(game.obj(card).zone, Zone::Stack | Zone::Battlefield | Zone::Gone) { continue; }
                for _ in 0..times { copies.push(Target::Object(game.copy_card(card, player))); }
            }
            f.cast_results.retain(|(slot, _)| slot != out);
            f.cast_results.push((*out, copies.iter().filter_map(|target| match target { Target::Object(card) => Some(*card), _ => None }).collect()));
            f.slots[*out] = Some(Value::Subjects(copies));
        }
        Instr::Cast { player, cards, offered, costs, cast, possible, from, rider, one, ceiling, haste } => {
            // a spell was cast, or its cast abandoned: what is left is offered again. Whether
            // it was cast is its payment's result: a cast reversed (733.1) was not made.
            if let Some(Pending::Payment { result }) = &f.pending {
                let Some(made) = *result else { return Err(Halt::Unsupported(crate::Unsupported("a parent cannot advance before its cast returns".into()))) };
                if made {
                    f.slots[*cast] = Some(Value::Consent(true));
                    if f.program.action_limit { game.use_ability(&f.ctx, crate::ability_limit::Limit::Action); }
                }
                f.pending = None;
            }
            let Some(player) = action::players(game, f, player).first().copied() else {
                f.slots[*cast] = Some(Value::Consent(false));
                f.slots[*possible] = Some(Value::Consent(false));
                return Ok(())
            };
            if f.slots[*offered].is_none() {
                // Bind only the retained result incarnation. Successful movement results
                // capture their actual destination; explicit source constraints stay fixed.
                let cards: Vec<ObjId> = action::moved(game, f, cards).into_iter().filter(|now| {
                    game.obj(*now).zone != Zone::Gone && match from {
                        CastOrigin::Fixed(zone) => game.obj(*now).zone == *zone
                            && (*zone != Zone::Graveyard || game.players[player].graveyard.contains(now)),
                        CastOrigin::Produced(slot) => f.cast_results.iter().any(|(out, arrived)| out == slot && arrived.contains(now)),
                        CastOrigin::Retained { original, .. } => action::moved(game, f, &[action::Operand::Reference(*original)]).contains(now),
                    }
                }).collect();
                f.cast_origins.retain(|(slot, _, _)| slot != offered);
                f.cast_origins.extend(cards.iter().map(|card| (*offered, *card, game.obj(*card).zone)));
                f.slots[*possible] = Some(Value::Consent(!cards.is_empty()));
                f.slots[*cast] = Some(Value::Consent(false));
                f.slots[*offered] = Some(Value::Cards(cards));
            }
            // This offer is of its own cards: one made meanwhile, by a mana ability activated
            // to pay for a cast of these, neither shows them nor ends this one.
            let Some(Value::Cards(all)) = f.slots[*offered].clone() else { unreachable!("the offer was made") };
            // a card cast is on the stack; one whose cast was abandoned is back (733.1). A
            // land is not cast (305.9).
            // "cast a spell from your graveyard": one cast, then the offer is over
            let done = *one && matches!(f.slots[*cast], Some(Value::Consent(true)));
            let left: Vec<ObjId> = all.iter().copied().filter(|c| !done && game.obj(*c).zone != Zone::Gone && f.cast_origins.iter().any(|(slot, card, zone)| slot == offered && card == c && game.obj(*c).zone == *zone) && !game.obj(*c).is(ir::CardType::Land)).collect();
            if f.pending.is_none() {
                for card in &left { if !game.offered(player, *card) { game.offer(player, *card, *costs, rider.clone().map(|r| crate::permission::Rider { by: Some((f.ctx.source, f.ctx.ability.clone())), ..r }), ceiling.map(|slot| match f.slots[slot] { Some(Value::Number(n)) => n, _ => unreachable!("typed cast ceiling") })).haste = *haste; } }
            }
            // A cast is its own move and arrives as a child; an answer here declines.
            if !left.is_empty() { f.ask(player, Choice::Cast { cards: left })?; }
            for card in all { game.revoke(player, card); }
            f.cast_origins.retain(|(slot, _, _)| slot != offered);
        }
        Instr::Pay { payer, cost, each, paid } => {
            let outcome = |f: &mut Frame, made: bool| {
                f.slots[*paid] = Some(Value::Consent(made));
            };
            // the child has reported: nothing of this instruction happens twice
            if let Some(Pending::Payment { result }) = &f.pending {
                let Some(made) = *result else { return Err(Halt::Unsupported(crate::Unsupported("a parent cannot advance before its payment returns".into()))) };
                f.pending = None;
                outcome(f, made);
                if made && f.program.action_limit { game.use_ability(&f.ctx, crate::ability_limit::Limit::Action); }
                return Ok(());
            }
            let Some(player) = action::players(game, f, payer).first().copied() else { outcome(f, false); return Ok(()) };
            let source = f.ctx.object_source();
            let cost = if cost.x()>0 { cost.clone().announced(f.ctx.x).map_err(crate::Unsupported)? } else { cost.clone() };
            let cost = match each { Some(n) => cost.repeated_generic(n.checked(game, &f.ctx, f)?)?, None => cost };
            let mana = cost.mana();
            // life is paid only by a player whose life total is at least that (119.4)
            let Some(life) = cost.life(game, &f.ctx).filter(|life| game.can_pay_life(player, *life)) else { outcome(f, false); return Ok(()) };
            // Waterbend exposes a rules domain, never selected objects or mana sources.
            let waterbend = if let Some((index, generic)) = cost.waterbend() {
                let (choices, _) = game.cost_choices(Ctx::new(player, source), &cost).map_err(|e| crate::Unsupported(e.to_string()))?;
                let among = choices.get(index).ok_or_else(|| crate::Unsupported("waterbend payment has no matching choice group".into()))?.0.clone();
                let most = generic.min(among.len() as u32);
                Some(crate::state::Waterbend { among, generic, most })
            } else { None };
            let (steps, with, tapped) = match f.ask(player, Choice::Pay { cost: mana, life, waterbend })? {
                Answer::Yes => return Err(crate::Unsupported("submit explicit payment instructions or decline (118.12)".into()).into()),
                Answer::Pay { plan, with, tapped } => {
                    if !plan.is_empty() && !cost.has_mana() {
                        return Err(crate::Unsupported("there is no mana payment during which to activate mana abilities (605.3a)".into()).into());
                    }
                    (plan, with, tapped)
                },
                _ => { outcome(f, false); return Ok(()) }
            };
            // the objects tapped, as the group of the cost that chooses them
            let paying = match cost.waterbend() {
                Some((index, _)) => { let mut paying = vec![Vec::new(); index + 1]; paying[index] = tapped; paying }
                None => Vec::new(),
            };
            game.procedures.push(crate::procedure::Procedure::Payment(crate::payment::Payment::effect(player, source, cost.clone(), steps, paying, with)));
            f.pending = Some(Pending::Payment { result: None });
            return Err(Halt::Child);
        }
        Instr::SkipUnless { consent, count } => {
            if matches!(f.slot(*consent), Some(Value::Consent(false))) {
                let empty: Vec<_> = f.program.instrs[f.pc+1..f.pc+count+1].iter().flat_map(Instr::skipped_result).collect();
                for (slot,value) in empty { f.slots[slot]=Some(value); }
                f.pc += count;
            }
        }
        Instr::Action { action, created, attempt } => {
            if matches!(action, action::Action::Prevent{..}) && (f.occurrence.is_none() || f.ctx.ability.is_none()) {
                return Err(crate::Unsupported("prevention requires a retained stack occurrence and ability identity".into()).into());
            }
            let Some(Request::Action(request)) = offer(game, f, attempt.as_ref(), *created, |game,f| action::evaluate(game,f,action).map(Request::Action))? else { return Ok(()) };
            match crate::perform::run(game, request)? {
                Ok(result) => f.operation_result(game,result, *created),
                Err(decision) => {
                    f.pending = Some(Pending::Operation { result: None, created: *created });
                    return Err(Halt::Ask(decision));
                }
            }
        },
        Instr::Alternative { actions, options } => {
            let Answer::Alternative(index) = f.ask(f.ctx.controller, Choice::Alternative { among: options.clone(), optional: false })? else {
                unreachable!("validated instruction alternative answer");
            };
            f.choices.push(crate::state::PublicChoice::Alternative { player: f.ctx.controller,
                source: f.ctx.object_source(), ability: f.ctx.ability.clone(), instruction: f.occurrence_base + f.pc, index });
            let request = action::evaluate(game, f, &actions[index])?;
            match crate::perform::run(game, request)? {
                Ok(result) => f.operation_result(game,result, None),
                Err(decision) => {
                    f.pending = Some(Pending::Operation { result: None, created: None });
                    return Err(Halt::Ask(decision));
                }
            }
        }
        Instr::FlipCoin { out } => {
            let Answer::CoinCall(call) = f.ask(f.ctx.controller, Choice::CoinCall)? else {
                return Err(crate::Unsupported("a called coin flip requires a call".into()).into());
            };
            let player = f.ctx.controller;
            let flip = crate::start::CoinFlip {player, call: Some(call), face: game.flip_coin(player)};
            game.log.push(crate::Event::CoinFlipped {source: f.ctx.object_source(), result: flip});
            f.slots[*out] = Some(Value::Coin(flip));
        }
        Instr::Choose { kind, who, out } => {
            let Answer::Value(value) = f.ask(f.player(*who), kind.choice())? else {
                return Err(crate::Unsupported("expected a scalar choice answer".into()).into());
            };
            if !crate::binding::accepts(*kind, &value) {
                return Err(crate::Unsupported("scalar choice answer is outside its domain".into()).into());
            }
            f.choices.push(crate::state::PublicChoice::Value {
                player: f.player(*who), source: f.ctx.object_source(), ability: f.ctx.ability.clone(), slot: *out, value: value.clone(),
            });
            f.slots[*out] = Some(value);
        }
        Instr::Endure { permanent,amount } => {
            let pending=match f.pending.take() {
                Some(Pending::Endure(pending))=>pending,
                None=>{ let Some(pending)=endure::Pending::new(game,f,permanent,amount.checked(game,&f.ctx,f)?)? else { return Ok(()) }; pending },
                _=>unreachable!("Endure owns its retained cost"),
            };
            if let Some(decision)=pending.decision() { f.pending=Some(Pending::Endure(pending));return Err(Halt::Ask(decision)); }
            if let Some((player,paid))=pending.payment() { if paid { f.choices.push(crate::state::PublicChoice::Alternative { player,source:f.ctx.object_source(),ability:f.ctx.ability.clone(),instruction:f.occurrence_base+f.pc,index:0 }); } }
            match crate::perform::run(game,pending.request()?)? {
                Ok(result)=>f.operation_result(game,result,None),
                Err(decision)=>{ f.pending=Some(Pending::Operation{result:None,created:None});return Err(Halt::Ask(decision)); }
            }
        }
        Instr::LibraryScan { by, amount, limit, all, hit, designated, bound } => {
            let mut pending = match f.pending.take() {
                Some(Pending::LibraryScan(pending)) => pending,
                None => {
                    let value = amount.evaluate(game, &f.ctx, f);
                    let Some(player) = f.actor(*by) else {
                        for slot in [all, hit, designated] { f.slots[*slot] = Some(Value::Cards(Vec::new())); }
                        f.slots[*bound] = Some(Value::Number(limit.ceiling(value)));
                        return Ok(());
                    };
                    library_scan::Pending::new(player, value, *limit, &f.ctx)
                },
                _ => unreachable!("a library scan resumes its own retained continuation"),
            };
            if let Some(decision) = pending.advance(game)? {
                f.pending = Some(Pending::LibraryScan(pending));
                return Err(Halt::Ask(decision));
            }
            f.slots[*all] = Some(Value::Cards(pending.exiled));
            f.slots[*hit] = Some(Value::Cards(pending.hit.into_iter().collect()));
            f.slots[*designated] = Some(Value::Cards(pending.designated.into_iter().collect()));
            f.slots[*bound] = Some(Value::Number(pending.limit.ceiling(pending.bound)));
        }
        Instr::Discovered { by, bound } => {
            let Some(Value::Number(value)) = f.slots[*bound] else { unreachable!("typed discover value") };
            if let Some(player) = f.actor(*by) { game.log.push(crate::Event::Discovered { player, value }); }
        }
        Instr::TakeUntil { set, whose, out, matched } => {
            let mut cards = Vec::new();
            let mut found = Vec::new();
            if let Some(player) = action::players(game, f, whose).first() {
                for card in game.players[*player].library.iter().rev().copied() {
                    cards.push(card);
                    if set.contains(game, &f.ctx, f, card) { found.push(card); break; }
                }
            }
            f.slots[*out] = Some(Value::Cards(cards));
            f.slots[*matched] = Some(Value::Cards(found));
        }
        Instr::TakeTop { n, whose, out } => {
            let n = n.checked(game, &f.ctx, f)? as usize;
            let cards = action::players(game,f,whose).first().map_or_else(Vec::new,|player| game.top_cards(*player,n));
            f.slots[*out] = Some(Value::Cards(cards));
        }
        Instr::Show { from, by, to, out } => {
            let cards = f.cards(*from);
            let shown = match to {
                Audience::Everyone => game.reveal(f.player(*by), cards),
                Audience::Only(who) => {
                    if let Some(player) = f.actor(*who) { game.disclose(crate::observation::Kind::Looked, player, &cards); }
                    cards
                }
                Audience::Owners => {
                    for card in &cards { game.disclose(crate::observation::Kind::Looked, game.obj(*card).owner, &[*card]); }
                    cards
                }
            };
            f.slots[*out] = Some(Value::Cards(shown));
            f.shown.push((*out, *to));
        }
        Instr::Gate { who, probe, out, possible } => {
            // no player to do it: nothing is done, and no one is asked
            let Some(player) = f.actor(*who) else {
                if let Some(slot) = possible { f.slots[*slot] = Some(Value::Consent(false)); }
                f.slots[*out] = Some(Value::Consent(false));
                return Ok(());
            };
            let mut feasible = true;
            if let Some(slot) = possible { f.slots[*slot] = Some(Value::Consent(true)); }
            if let Some(p) = probe {
                let ctx = Ctx { controller: player, ..f.ctx.clone() };
                let cards = f.cards(p.from);
                let private = f.concealed_pool(game, player, cards.iter().copied().filter(|o| still_there(game, *o, p.still_in)));
                let mut fitting = 0;
                for card in cards {
                    if still_there(game, card, p.still_in) && p.set.contains(game, &ctx, f, card) {
                        fitting += 1;
                    }
                }
                // a player can't choose an option that is impossible (608.2d)
                feasible = fitting >= p.count.evaluate(game, &f.ctx, f) as usize;
                if !feasible {
                    if let Some(slot) = possible { f.slots[*slot] = Some(Value::Consent(false)); }
                    if !private {
                        f.slots[*out] = Some(Value::Consent(false));
                        return Ok(());
                    }
                }
            }
            let yes = f.ask(player, Choice::May { possible: feasible })? == Answer::Yes;
            f.slots[*out] = Some(Value::Consent(yes));
        }
        Instr::Select { from, set, pick, chooser, gate, still_in, out } => {
            if let Pick::Optional { paid, possible, .. } = pick {
                f.slots[*paid] = Some(Value::Consent(false));
                f.slots[*possible] = Some(Value::Consent(false));
            }
            let declined = matches!(gate.and_then(|g| f.slot(g)), Some(Value::Consent(false)));
            let mut chosen = Vec::new();
            if !declined {
                let Some(player) = f.actor(*chooser) else {
                    f.slots[*out] = Some(Value::Cards(Vec::new()));
                    return Ok(());
                };
                let ctx = Ctx { controller: player, ..f.ctx.clone() };
                // what the chosen object is chosen for: "put a counter on", "a copy of",
                // "that player discards that card", and where a moved one goes (608.2d)
                let purpose = purpose::chosen_for(&f.program.instrs, f.pc, *out);
                let (verb, to) = match (set, purpose) {
                    (SelectionFilter::Action { verb, .. }, Some((Verb::Move, to))) if *verb == Verb::Move => (Verb::Move, to),
                    (SelectionFilter::Action { verb, .. }, _) => (*verb, None),
                    (_, Some(purpose)) => purpose,
                    _ => (Verb::Choose, None),
                };
                let cards = f.cards(*from);
                let private = matches!(pick, Pick::Optional { .. } | Pick::AnyNumber | Pick::UpTo(_))
                    && f.concealed_pool(game, player, cards.iter().copied().filter(|o| still_there(game, *o, *still_in)));
                let mut fitting = Vec::new();
                for card in cards {
                    if still_there(game, card, *still_in) && set.contains(game, &ctx, f, card) {
                        fitting.push(card);
                    }
                }
                let evaluated;
                let pick = if let Pick::Quantity(n) = pick {
                    evaluated = Pick::Exactly(n.checked(game, &f.ctx, f)? as usize);
                    &evaluated
                } else { pick };
                chosen = match pick {
                    Pick::Optional { count, paid, possible } => {
                        let feasible = fitting.len() >= *count;
                        f.slots[*possible] = Some(Value::Consent(feasible));
                        if feasible || private {
                            let Answer::OptionalObjects(answer) = f.ask(player, Choice::OptionalObjects { verb, to, among: fitting, count: *count })?
                                else { unreachable!("validated optional objects"); };
                            f.slots[*paid] = Some(Value::Consent(answer.is_some()));
                            answer.unwrap_or_default()
                        } else { Vec::new() }
                    },
                    Pick::AnyNumber if !fitting.is_empty() || private => match f.ask(player, Choice::Select { min: 0, max: fitting.len(), among: fitting })? {
                        Answer::Objects(cards) => cards,
                        _ => unreachable!("validated subset selection"),
                    },
                    Pick::UpTo(n) if *n > 0 && (!fitting.is_empty() || private) => match f.ask(player, Choice::Select { min: 0, max: (*n).min(fitting.len()), among: fitting })? {
                        Answer::Objects(cards) => cards,
                        _ => unreachable!("validated bounded selection"),
                    },
                    Pick::UpTo(_) => Vec::new(),
                    Pick::Exactly(0) | Pick::Whole(0) => Vec::new(),
                    Pick::Whole(n) if fitting.len() < *n => Vec::new(),
                    Pick::Whole(n) if fitting.len() > *n => match f.ask(player, Choice::Objects { verb, to, among: fitting, count: *n })? {
                        Answer::Objects(c) => c,
                        _ => Vec::new(),
                    },
                    Pick::Exactly(n) if fitting.len() > *n => match f.ask(player, Choice::Objects { verb, to, among: fitting, count: *n })? {
                        Answer::Objects(c) => c,
                        _ => Vec::new(),
                    },
                    _ => fitting,
                };
            }
            f.slots[*out] = Some(Value::Cards(chosen));
        }
        Instr::Remainder { of, minus, still_in, out } => {
            let taken: Vec<ObjId> = minus.iter().flat_map(|m| f.cards(*m)).collect();
            let rest = f.cards(*of).into_iter().filter(|c| still_there(game, *c, *still_in) && !taken.contains(c)).collect();
            f.slots[*out] = Some(Value::Cards(rest));
        }
        Instr::Piles { from, still_in, opponent, chooser, one, other } => {
            let pending = match f.pending.take() {
                Some(Pending::Piles(pending)) => pending,
                None => {
                    let cards = f.cards(*from).into_iter().filter(|card| still_there(game, *card, *still_in)).collect();
                    let (controller, n) = (f.ctx.controller, game.players.len());
                    // the opponents, in turn order from the controller
                    let among = if *opponent { (1..n).map(|i| (controller + i) % n).filter(|p| !game.players[*p].lost).collect() } else { vec![controller] };
                    let players = action::players(game, f, chooser);
                    let [chooser] = players.as_slice() else {
                        if players.is_empty() {
                            f.slots[*one] = Some(Value::Cards(Vec::new()));
                            f.slots[*other] = Some(Value::Cards(Vec::new()));
                            return Ok(());
                        }
                        return Err(crate::Unsupported("piles: one chooser required".into()).into());
                    };
                    piles::Pending::new(cards, *chooser, among)
                }
                _ => unreachable!("instruction resumes its own procedure"),
            };
            if let Some(decision) = pending.decision() {
                f.pending = Some(Pending::Piles(pending));
                return Err(Halt::Ask(decision));
            }
            let (chosen, rest) = pending.result();
            f.slots[*one] = Some(Value::Cards(chosen));
            f.slots[*other] = Some(Value::Cards(rest));
        }
        Instr::Exile { from, out, hidden } => {
            let lookers: Option<Vec<PlayerId>> = hidden.as_ref().map(|said| said.lookers.iter().filter_map(|who| f.actor(*who)).collect());
            // the permanent whose controller may look at it (702.75a)
            let watcher = hidden.as_ref().filter(|h| h.watched).and_then(|_| f.ctx.source);
            let mut arrived = Vec::new();
            for card in f.cards(*from) {
                let now = match &lookers {
                    Some(lookers) => game.exile_hidden(card, lookers, watcher),
                    None => game.exile_result(card),
                };
                // exiled with this object, as by its ability (607.2a)
                if let (Some(now), Some(source)) = (now, f.ctx.source) && game.obj(now).zone == Zone::Exile {
                    game.exiled_with.push((source, f.ctx.ability.clone(), now, f.ctx.link_generation));
                }
                if let Some(now) = now {
                    arrived.push(now);
                }
            }
            f.cast_results.retain(|(slot, _)| slot != out);
            f.cast_results.push((*out, arrived.iter().copied().filter(|card| matches!(game.obj(*card).zone, Zone::Battlefield | Zone::Graveyard | Zone::Exile | Zone::Stack)).collect()));
            f.slots[*out] = Some(Value::Cards(arrived));
        }
        Instr::MoveTogether { parts, .. } => {
            let mut movements = Vec::new();
            let known = f.known_cards();
            for (src,to) in parts {
                match to {
                    Dest::Hand | Dest::Graveyard => {
                        let zone=if *to==Dest::Hand {Zone::Hand}else{Zone::Graveyard};
                        movements.extend(f.cards(*src).into_iter().map(|object|crate::perform::Movement::Zone {object,zone}));
                    }
                    Dest::Library {position,order} => movements.extend(f.cards(*src).into_iter().map(|object|
                        crate::perform::Movement::Library {object,position:*position,order:*order,known:known.contains(&object)})),
                    Dest::Battlefield(to) => {
                        let action=action::Action::Move {groups:vec![(vec![action::Operand::Reference(*src)],to.clone())],said:ir::action::MoveVerb::Put};
                        let crate::perform::Action::Move(entries)=action::evaluate(game,f,&action)? else {unreachable!("evaluated movement")};
                        movements.extend(entries);
                    }
                }
            }
            match crate::perform::run(game,crate::perform::Action::Move(movements))? {
                Ok(result)=>f.operation_result(game,result,None),
                Err(decision)=>{f.pending=Some(Pending::Operation {result:None,created:None});return Err(Halt::Ask(decision));}
            }
        }
        Instr::Move { src, to, when, .. } => {
            if when.as_ref().is_some_and(|condition| !condition.evaluate(game, &f.ctx, f)) { return Ok(()); }
            let cards = f.cards(*src);
            match to {
                Dest::Hand | Dest::Graveyard => {
                    let zone = if *to == Dest::Hand { Zone::Hand } else { Zone::Graveyard };
                    if let Err(decision) = game.departure_batch(cards.into_iter().map(|card| (card, crate::departure::Kind::Move(zone))).collect())? {
                        f.pending = Some(Pending::Operation { result: None, created: None });
                        return Err(Halt::Ask(decision));
                    }
                }
                Dest::Library { position, order } => place(game, f, cards, *position, *order, None, None, false)?,
                Dest::Battlefield(_) => unreachable!("entry is a grouped result movement"),
            }
        }
    }
    Ok(())
}

/// Owners privately order the groups going to the same library position, all of them asked
/// before anything moves (401.4); an explicit random order asks no one.
fn place(game: &mut Game, f: &mut Frame, cards: Vec<ObjId>, position: LibraryPosition, order: Option<Order>, by: Option<PlayerId>, destination: Option<([LibraryPosition; 2], Option<PlayerId>)>, before_shuffle: bool) -> Result<(), Halt> {
    let mut pending = match f.pending.take() {
        Some(Pending::Library(pending)) => pending,
        None => {
            let known = f.shown.iter().flat_map(|(slot, _)| match f.slot(*slot) {
                Some(Value::Cards(cards)) => cards.clone(),
                Some(Value::Subjects(subjects)) => subjects.iter().filter_map(|t| if let Target::Object(o) = t { Some(*o) } else { None }).collect(),
                _ => Vec::new(),
            }).collect();
            crate::library::Placement::new(game, cards, position, order, known).before_shuffle(before_shuffle).ordered_by(by).destination(game, destination).moving(game)
        },
        _ => unreachable!("instruction resumes its own procedure"),
    };
    if let Some(decision) = pending.advance(game) {
        f.pending = Some(Pending::Library(pending));
        return Err(Halt::Ask(decision));
    }
    Ok(())
}


