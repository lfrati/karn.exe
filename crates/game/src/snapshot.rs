//! The game as one viewer may see it now (CR 400.2), as values: what a host shows a seat,
//! or all of it for a harness that sets games up and compares them.
//!
//! A [`Snapshot`] is taken from an unchanged game and holds nothing of it: who controls
//! what, what each object is after every continuous effect (613), which faces this viewer
//! may look at ([`Game::sees`]). Taking it is the work; a host that is asked again about a
//! game that has not changed gives the one it has ([`crate::simulation::Simulation::snapshot`]).
//!
//! Not here, because they are not the state of the game: what is asked and the moves that
//! answer it (a request, and a search made only when asked for), what happened (events),
//! what a seat was shown ([`crate::observation`]) and what the engine answered for a seat
//! ([`crate::policy`]). Each of those is delivered once; a snapshot may be read any number
//! of times.

use crate::binding::Record;
use crate::event::Target;
use crate::state::{Game, ObjId, PlayerId, PublicChoice, Zone};
use ir::mana::ManaSymbol;
use ir::{Ability, CardType, Part};
use std::collections::BTreeMap;

/// What only a permanent has (110.5, 120, 122, 302.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permanent {
    pub attached_to: Option<Target>,
    pub tapped: bool,
    /// A double-faced permanent with its back face up (701.27g).
    pub transformed: bool,
    pub damage: i32,
    pub counters: BTreeMap<String, u32>,
    pub sick: bool,
    /// Its level: what it was given, or 1 for a Class without one (716.2b, 716.2d).
    pub level: Option<u32>,
    /// Of a Room, which of its doors are unlocked, the left first (709.5c).
    pub unlocked: Option<[bool; 2]>,
    /// Monstrous and renowned (701.37b, 702.112b).
    pub monstrous: bool,
    pub renowned: bool,
    /// Case solved designation (719.3b).
    pub solved: bool,
    pub prepared: bool,
    /// Saddled, until end of turn (702.171b).
    pub saddled: bool,
    /// Which of its abilities are activated abilities, in the order printed.
    pub activated: Vec<usize>,
    /// The values chosen for it, each under the ability that asked.
    pub choices: Vec<Record>,
}

/// What only an object on the stack has (112.1, 601.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnStack {
    pub ability: bool,
    /// The value announced for {X} in its cost, known to every player (107.3a).
    pub x: Option<u32>,
    pub targets: Vec<Vec<Target>>,
    /// Mana was spent on it that says it can't be countered (106.6).
    pub uncounterable: bool,
    /// The opponent its gift was promised to as it was cast, known to every player (702.174a).
    pub gift: Option<PlayerId>,
    /// The alternative of each choice among alternatives its cost has that its controller
    /// announced paying (601.2b, 602.2b), as it is said: "sacrifice a Food".
    pub announced: Vec<String>,
}

/// One object whose face the viewer may look at. An activated or triggered ability on the
/// stack has no characteristics but its text (405.4): its types, colors, power, toughness
/// and loyalty are none, and its text is the prompt's ([`crate::prompt::Entity`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub id: ObjId,
    /// Its name. Of an ability on the stack, its source's, as the ability was put on the
    /// stack with it: a name shown by the harness's convention, which no rule gives it, not a
    /// characteristic of the ability (405.4); empty for a face-down source, which has none
    /// (708.2a), and for a rule's ability, which has no source (702.179d, 725.2).
    pub name: String,
    pub owner: PlayerId,
    pub controller: PlayerId,
    /// Its card types, subtypes and supertypes as every effect leaves them (205, 613.1d).
    pub types: Vec<CardType>,
    pub subtypes: Vec<ir::Subtype>,
    pub supertypes: Vec<ir::Supertype>,
    /// Its colors as every effect leaves them (105.2, 613.1e); none for a colorless object.
    pub colors: Vec<ir::Color>,
    /// Its power and toughness, if it has them (208.3): a permanent's only while it is a
    /// creature; off the battlefield those printed on it or those a characteristic-defining
    /// ability sets in every zone (604.3, 208.2a), as effects leave them.
    pub pt: Option<(i32, i32)>,
    /// A planeswalker card's loyalty off the battlefield, the number printed on it (306.5a,
    /// 209.1). On the battlefield its loyalty is the loyalty counters on it (306.5c).
    pub loyalty: Option<u32>,
    pub token: bool,
    pub is_card: bool,
    pub commander: bool,
    /// The abilities of an emblem (114.1), which no card shows.
    pub emblem: Option<Vec<Ability>>,
    /// An exiled card an effect lets this player play, and whether as a land too.
    pub may_play: Option<(PlayerId, bool)>,
    /// When that player may no longer play it by any grant, said of them
    /// ([`Game::permission_until`]).
    pub may_play_until: Option<&'static str>,
    /// Each of that player's grants, when they differ: whether as a land too, and until when
    /// ([`Game::permission_grants`]). Empty for a single grant.
    pub may_play_grants: Vec<(bool, Option<&'static str>)>,
    /// Turn on which this exile incarnation was plotted (702.170c-d).
    pub plotted: Option<u32>,
    /// Turn in which this exile incarnation was warped, castable by its owner after it
    /// (702.185a–b).
    pub warped: Option<u32>,
    /// Turn on which this exile incarnation was foretold, which is public (702.143e).
    pub foretold: Option<u32>,
    /// A face-down permanent (708.2): its name, types and power are the face-down ones.
    pub face_down: Option<FaceDown>,
    /// A card exiled face down (406.3): its name and types are none (406.3a).
    pub hidden: Option<Hidden>,
    pub permanent: Option<Permanent>,
    pub on_stack: Option<OnStack>,
}

/// What every player knows of a face-down permanent, and what its card is for a viewer who
/// may look at it now: its controller (708.5), or everyone in the harness's view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDown {
    /// How it came to be face down, which is public (708.6).
    pub by: crate::face::Concealment,
    /// The card it is, while this viewer may look at it. Not a past disclosure: a player
    /// who loses control of it no longer has it.
    pub card: Option<CardIdentity>,
}

/// What every player knows of a card exiled face down, and what it is for a viewer who may
/// look at it now (406.3), or everyone in the harness's view. No decision to look is asked:
/// a player who may look at it is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hidden {
    /// Who may look at it, which is public.
    pub lookers: Vec<PlayerId>,
    /// The permanent whose controller may look at it too (702.75a), which is public.
    pub watcher: Option<ObjId>,
    /// The card it is, while this viewer may look at it.
    pub card: Option<CardIdentity>,
}

/// A physical card, as its printed front face names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardIdentity {
    /// Its definition among [`Game::defs`].
    pub card: usize,
    pub name: String,
}

/// Mana whose text restricts how it can be spent (106.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restricted {
    pub provenance: Option<serde_json::Value>,
    pub mana: ManaSymbol,
    /// The object whose text it is, and that object's name.
    pub source: ObjId,
    pub from: String,
    pub uncounterable: bool,
    /// Until when it isn't lost as steps and phases end (106.4, 702.189a).
    pub kept: Option<crate::state::Kept>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub counters: std::collections::BTreeMap<String,u32>,
    /// The seat entitled to make this player's decisions now (723.5).
    pub decision_maker: PlayerId,
    pub life: i32,
    /// How many cards: all another player may know of a hand or a library (402.3, 401.3).
    pub hand_size: usize,
    /// None when an effective rule gives this player no maximum (402.2).
    pub maximum_hand_size: Option<usize>,
    pub companion: Option<(String,bool)>,
    /// Public designation and scalar history. No hidden-zone location or physical ID.
    pub commanders: Vec<Commander>,
    pub library: usize,
    /// Current top identity only when this viewer may look at it (401.5).
    pub top_card: Option<ObjId>,
    pub graveyard: Vec<ObjId>,
    pub colors: [u32; 5],
    pub colorless: u32,
    pub restricted: Vec<Restricted>,
    pub land_played: bool,
    pub lost: bool,
    /// None until a rule or effect sets it (702.179b).
    pub speed: Option<u8>,
    /// The hand itself, for its own player and for the harness.
    pub hand: Option<Vec<ObjId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commander {
    pub name: String,
    pub identity: Vec<ir::Color>,
    pub casts: Vec<u32>,
    pub damage: Vec<u32>,
}

/// The game as `viewer` may see it; None: all of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub viewer: Option<PlayerId>,
    pub turn: u32,
    pub monarch: Option<PlayerId>,
    pub part: Part,
    pub active: PlayerId,
    pub priority: PlayerId,
    pub winner: Option<PlayerId>,
    pub finished: bool,
    /// Has the first turn begun (103.8)? Before it, the mulligans and the opening-hand
    /// actions (103.5, 103.6), while `turn` and `part` already say the first turn's.
    pub begun: bool,
    pub choices_made: Vec<PublicChoice>,
    /// Hidden-zone cards currently kept revealed by stack occurrences, not past disclosures.
    pub publicly_revealed: Vec<ObjId>,
    pub players: Vec<Player>,
    pub battlefield: Vec<ObjId>,
    pub exile: Vec<ObjId>,
    pub command: Vec<ObjId>,
    pub stack: Vec<ObjId>,
    pub attacking: Vec<ObjId>,
    /// What each attacking creature attacks: a player, planeswalker or battle (506.4). One whose
    /// planeswalker or battle is gone attacks nothing, and is only `attacking` (506.4c).
    pub attacks: Vec<(ObjId, Target)>,
    /// Each object once, in no order a host may rely on.
    pub objects: Vec<Object>,
    /// The phases of this turn (500.1, 500.8): the occurrence in progress ("this phase",
    /// "this combat"), and those still to come, each with whether an effect added it.
    pub phase: Option<(crate::turn::PhaseId, bool)>,
    pub upcoming: Vec<(crate::turn::PhaseId, ir::turn::Phase, bool)>,
    /// Phases were added to this turn or skipped in it.
    pub turn_modified: bool,
    /// The skip effects waiting (614.10), oldest first; public, as the effects of resolved
    /// spells and abilities are.
    pub skips: Vec<crate::turn::Skip>,
    /// What would begin while its player chooses which skip applies (616.1).
    pub skipping: Option<crate::turn::Occurrence>,
}

impl Snapshot {
    /// Take a snapshot of the current game for this viewer.
    pub fn of(g: &Game, viewer: Option<PlayerId>) -> Snapshot {
        use crate::state::{Choice, Decision};
        // and the cards a choice is among, wherever they are, for the player who chooses
        let offered: &[ObjId] = match &g.decision {
            Some(Decision::Choose { player, choice: Choice::Objects { among, .. } | Choice::OptionalObjects { among, .. } | Choice::OrderCards { cards: among, .. } | Choice::Arrange { cards: among, .. } | Choice::Search { cards: among, .. } })
                if viewer.is_none_or(|v| g.shares_information(v,*player)) => among,
            _ => &[],
        };
        // A revealed or found library card can remain relevant at a later choice (701.20a,e).
        // Enumerate only library faces some player currently knows, then apply the viewer filter.
        let known_library = g.players.iter().flat_map(|p| &p.library)
            .filter(|id| (0..g.players.len()).any(|p| g.sees(p, **id)));
        let shown = g.players.iter().flat_map(|p| p.hand.iter().chain(&p.graveyard)).chain(&g.battlefield).chain(&g.exile).chain(&g.command).chain(&g.stack).chain(known_library);
        let mut ids: Vec<ObjId> = Vec::new();
        // a face-down permanent is there for everyone, its face for whoever may look (708.5)
        for id in shown.copied().filter(|id| viewer.is_none_or(|v| g.perceives(v, *id))).chain(offered.iter().copied()) {
            if !ids.contains(&id) { ids.push(id); }
        }
        let objects = ids.into_iter().map(|id| Snapshot::object(g, id, viewer)).collect();
        let players = g.players.iter().enumerate().map(|(i, p)| {
            let cards = |zone: &[ObjId]| zone.iter().filter(|o| g.is(Target::Object(**o), ir::Sort::Card)).count();
            Player {
                counters: p.counters.clone(),
                decision_maker: g.decision_maker(i),
                life: p.life,
                hand_size: cards(&p.hand),
                maximum_hand_size: g.maximum_hand_size(i),
                companion: g.companion(i).map(|c|(g.defs[c.definition].name().to_owned(),c.taken)),
                commanders: g.commanders.iter().filter(|c| c.owner == i).map(|c| Commander {
                    name: g.defs[c.definition].name().to_owned(), identity: c.identity.clone(), casts: c.casts.clone(), damage: c.damage.clone(),
                }).collect(),
                library: cards(&p.library),
                top_card:g.library_top(i).filter(|card|viewer.is_none_or(|v|g.sees(v,*card))),
                graveyard: p.graveyard.clone(),
                colors: p.pool.colors,
                colorless: p.pool.colorless,
                restricted: p.pool.restricted.iter().map(|m| Restricted {
                    provenance: m.provenance.as_deref().and_then(crate::audit::RestrictedManaProvenance::audit).cloned(), mana: m.mana, source: m.source, from: g.obj(m.source).chars.name.clone(), uncounterable: m.uncounterable, kept: m.kept,
                }).collect(),
                land_played: p.land_played,
                speed: p.speed,
                lost: p.lost,
                hand: viewer.is_none_or(|v| g.shares_information(v,i)).then(|| p.hand.clone()),
            }
        }).collect();
        let mut publicly_revealed: Vec<_> = g.reveal_grants()
            .filter(|id| matches!(g.obj(*id).zone, Zone::Hand | Zone::Library)).collect();
        publicly_revealed.sort_unstable();
        publicly_revealed.dedup();
        let plan = g.turn_plan();
        Snapshot {
            viewer,
            publicly_revealed,
            turn: g.turn,
            monarch: g.monarch,
            part: g.part,
            active: g.active,
            priority: g.priority,
            winner: g.winner(),
            finished: g.finished(),
            begun: g.begun(),
            choices_made: g.public_choices().cloned().collect(),
            players,
            battlefield: g.battlefield.clone(),
            exile: g.exile.clone(),
            command: g.command.clone(),
            stack: g.stack.clone(),
            attacking: g.combat.attackers.iter().map(|(a, _)| *a).collect(),
            attacks: g.combat.attackers.iter().copied().filter(|(_, at)| g.still_attacked(*at)).collect(),
            objects,
            phase: plan.phase().map(|run| (run.id, run.added)),
            upcoming: plan.upcoming().map(|run| (run.id, run.phase, run.added)).collect(),
            turn_modified: plan.modified(),
            skips: g.skips.clone(),
            skipping: plan.skipping(),
        }
    }

    fn object(g: &Game, id: ObjId, viewer: Option<PlayerId>) -> Object {
        let o = g.obj(id);
        // its characteristics, if it has any to show: an ability on the stack has none (405.4)
        let shown = g.characteristics(id);
        let def = shown.as_ref().and_then(Characteristics::def);
        let seen = shown.as_ref().map(Characteristics::seen);
        let face_down = match o.face {
            crate::face::Face::Down(down) => Some(FaceDown {
                by: down.by,
                card: viewer.is_none_or(|v| g.sees(v, id)).then(|| g.physical_card(id))
                    .flatten().map(|(card, def)| CardIdentity { card, name: def.name().to_owned() }),
            }),
            crate::face::Face::Up => None,
        };
        let hidden = o.hidden.as_ref().map(|hidden| Hidden {
            lookers: hidden.lookers.clone(),
            watcher: hidden.watcher,
            card: viewer.is_none_or(|v| g.sees(v, id)).then(|| g.physical_card(id))
                .flatten().map(|(card, def)| CardIdentity { card, name: def.name().to_owned() }),
        });
        let permanent = (o.zone == Zone::Battlefield).then(|| {
            let abilities = g.abilities(id);
            let mut activated: Vec<usize> = abilities.iter().enumerate().filter(|(_, a)| matches!(&***a, Ability::Activated { .. })).map(|(i, _)| i).collect();
            activated.sort_by_key(|i| def.map_or(*i, |d| d.ability_row(*i)));
            Permanent {
                attached_to: g.attached_to(id),
                tapped: o.tapped,
                transformed: o.transformed,
                damage: o.damage,
                counters: o.counters.clone(),
                sick: g.summoning_sick(id),
                level: o.level.or_else(|| seen.as_ref().is_some_and(|s| s.subtypes.contains(&ir::Subtype::class())).then_some(1)),
                unlocked: g.room_card(id).is_some().then_some(o.unlocked),
                monstrous: o.monstrous,
                saddled: o.saddled,
                renowned: o.renowned,
                solved: o.solved,
                prepared: o.prepared,
                activated,
                choices: o.choices.clone(),
            }
        });
        let on_stack = (o.zone == Zone::Stack).then(|| OnStack {
            ability: o.ability.is_some(), x: o.x, targets: o.targets.clone(), uncounterable: o.uncounterable, gift: o.gift, announced: o.announced.iter().map(|row|row.text.clone()).collect(),
        });
        let (types, subtypes, supertypes, colors, pt) = seen.map(|s| (s.types, s.subtypes, s.supertypes, s.colors, s.pt)).unwrap_or_default();
        Object {
            id,
            name: o.chars.name.clone(),
            owner: o.owner,
            controller: g.controller(id),
            types,
            subtypes,
            supertypes,
            colors,
            pt,
            loyalty: (o.zone != Zone::Battlefield).then(|| def.and_then(|d| d.loyalty)).flatten(),
            token: o.token(),
            is_card: g.is(Target::Object(id), ir::Sort::Card),
            commander: g.is_commander(id),
            emblem: o.emblem().then(|| g.abilities(id).into_iter().map(|a| (*a).clone()).collect()),
            may_play: g.permission(id),
            may_play_until: g.permission_until(id),
            may_play_grants: {
                let shown = g.permission(id).map(|(p, _)| p);
                let grants: Vec<_> = g.permission_grants(id).into_iter().filter(|x| Some(x.0) == shown).map(|x| (x.1, x.2)).collect();
                if grants.len() > 1 { grants } else { Vec::new() }
            },
            plotted: g.plotted(id),
            warped: g.warped(id),
            foretold: g.foretold(id).map(|f| f.turn),
            face_down,
            hidden,
            permanent,
            on_stack,
        }
    }
}

/// The gate that says whether an object has characteristics (109.3) to show every viewer:
/// [`Game::characteristics`] is None for one that has none, an ability on the stack (405.4),
/// and both the snapshot's [`Object`] and the prompt's [`crate::prompt::Entity`] then show
/// none. Through it they read the definition and the layered values. Not everything shown
/// goes through it: the object's name (which an ability keeps for display) is read from the
/// object, and the prompt reads rules text and abilities from the game itself after the gate
/// has let the object through. A new field taken from [`Game::def`] or [`Game::abilities`]
/// must be read after the same check.
pub(crate) struct Characteristics<'g> {
    game: &'g Game,
    id: ObjId,
}

impl<'g> Characteristics<'g> {
    /// The definition of what it is now ([`Game::def`]): its mana cost, inset and printed
    /// loyalty.
    pub(crate) fn def(&self) -> Option<&'g crate::CardDef> { self.game.def(self.id) }

    /// What every continuous effect leaves of them (613): its types, colors, power and
    /// toughness.
    pub(crate) fn seen(&self) -> crate::layers::Seen { self.game.seen(self.id, None) }
}

impl Game {
    /// An object's characteristics, as every viewer is shown them, its controller included.
    /// None for an activated or triggered ability on the stack, a delayed one and a copy of
    /// one included (603.7, 707.10): it "has the text of the ability that created it and no
    /// other characteristics" (405.4, 602.2a, 603.3). Its object is made with its source's
    /// card and characteristics ([`Game::def`] and [`Game::abilities`] answer the rules from
    /// them), and none of those is the ability's: shown as its own, they would show a
    /// face-down source's card to a player who can't look at it (708.5).
    pub(crate) fn characteristics(&self, id: ObjId) -> Option<Characteristics<'_>> {
        self.obj(id).ability.is_none().then_some(Characteristics { game: self, id })
    }
}
