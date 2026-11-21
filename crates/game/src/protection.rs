//! Protection's stated quality is shared by targeting, attachment, damage and blocking
//! (702.16b–f). Each quality is lowered once with the keyword that states it ([`Protection`])
//! and asked of a source as that source is: its colors as layer 5 leaves them, its types,
//! its controller.
use crate::binding::Value;
use crate::layers::Layer;
use crate::program::expr::{self, Bind, Locals};
use crate::select::{Ctx, Support};
use crate::{Game, ObjId, PlayerId, Res, Target, Zone, unsupported};
use ir::token::{KeywordInstance, KeywordParam, Quality};
use ir::{Atom, Filter, ObjectSet};

/// Is this description only of characteristics a source has wherever it is: card types,
/// subtypes, supertypes and colors (702.16a: "the ability applies to sources that are
/// permanents with that card type, subtype, or supertype and to any sources not on the
/// battlefield that are of that card type, subtype, or supertype")?
fn characteristic(f: &Filter) -> bool {
    match f {
        Filter::All(parts) | Filter::Any(parts) => !parts.is_empty() && parts.iter().all(characteristic),
        Filter::Not(x) => characteristic(x),
        Filter::Atom(Atom::Type(_) | Atom::Subtype(_) | Atom::Supertype(_) | Atom::Color(_) | Atom::Colorless
            | Atom::Multicolored | Atom::Monocolored) => true,
        Filter::Atom(Atom::Reference(r)) => r.form == ir::reference::RefForm::Choice,
        _ => false,
    }
}

/// "Goblins", "non-Spirit creatures", "legendary creatures": a bare noun's default permanent
/// domain, qualified by characteristics alone.
fn of_kind(set: &ObjectSet) -> bool {
    set.domain == ir::Domain::Permanent && set.controller.is_none() && set.owner.is_none() && set.caster.is_none()
        && set.zone.is_none() && !set.other && characteristic(&set.filter)
}

/// Can a keyword declaration say this protection? What it says is lowered with the text
/// that states it ([`Protection::compile`]): a printed one binds a chosen value through its
/// definition's links, a granted one through what chose it before.
pub(crate) fn known(item: &KeywordInstance) -> bool {
    item.keyword.name() == "Protection"
        && matches!(&item.param,
        // "and": the qualities all apply (702.16g); "or" is a choice made as an effect resolves
        KeywordParam::Protection(qualities) if !qualities.items.is_empty()
            && qualities.conj == ir::Conj::And
            && qualities.items.iter().all(|q| match q {
                Quality::Color(_) | Quality::EachColor | Quality::Multicolored | Quality::Monocolored
                    | Quality::Everything | Quality::ChosenColor | Quality::Set(_) | Quality::Player(_) => true,
                Quality::Filter(f) => characteristic(f),
                // chosen as the effect that grants it resolves: see [`choice_of_color`]
                Quality::ColorOfChoice | Quality::ChosenPlayer => false,
                // not run yet
                Quality::ColorOfControllersChoice | Quality::EachOfItsColors | Quality::EachColorAmong(_) => false,
            }))
}

/// "Protection from the color of your choice" said by a resolving effect: the keyword with
/// that quality as "the chosen color", for the color its controller chooses as it resolves,
/// just before it applies. None if it says no such quality.
pub(crate) fn choice_of_color(item: &KeywordInstance) -> Option<KeywordInstance> {
    let KeywordParam::Protection(qualities) = &item.param else { return None };
    if item.keyword.name() != "Protection" || !qualities.items.contains(&Quality::ColorOfChoice) { return None; }
    let mut chosen = qualities.clone();
    for q in &mut chosen.items {
        if *q == Quality::ColorOfChoice { *q = Quality::ChosenColor; }
    }
    Some(KeywordInstance { keyword: item.keyword, param: KeywordParam::Protection(chosen) })
}

/// One quality (702.16a), lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Protects {
    /// 702.16j: every object.
    Everything,
    /// Characteristics a source has wherever it is: a color, a type, "the chosen color".
    Kind(expr::Filter),
    /// Any other description: a source is one it describes.
    Set(expr::Set),
    /// 702.16k: what a player controls, or owns and no other player controls.
    Players(Players),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Players {
    Said(expr::Subject),
    All(expr::Set),
    /// As they were when the effect saying them applied.
    Fixed(Vec<PlayerId>),
}

/// Protection from these qualities, each a separate ability (702.16g–i); several instances
/// of one quality are redundant (702.16m).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protection {
    from: Vec<Protects>,
    /// What chosen values its qualities read and whose "you" they say, captured as the effect
    /// that grants it applies; for a printed keyword, none: what it reads is its holder's.
    values: Vec<Option<Value>>,
    controller: Option<PlayerId>,
}

impl Protection {
    /// The qualities, lowered with `bind` binding a value chosen elsewhere.
    pub(crate) fn compile(item: &KeywordInstance, support: &Support<'_>, bind: Bind<'_>) -> Res<Protection> {
        let KeywordParam::Protection(qualities) = &item.param else { return unsupported("protection: no quality") };
        let kind = |f: Filter, bind: Bind<'_>| -> Res<Protects> {
            let f = expr::Filter::compile(&f, support, bind)?;
            if !f.captures_all() { return unsupported("protection: a quality that reads a value it can't keep"); }
            Ok(Protects::Kind(f))
        };
        let color = |c: ir::Color| Filter::Atom(Atom::Color(c));
        let mut from = Vec::new();
        for q in &qualities.items {
            from.push(match q {
                Quality::Color(c) => kind(color(*c), &mut *bind)?,
                // 702.16h: protection from each color, so from any colored source
                Quality::EachColor => kind(Filter::Any(ir::Color::ALL.iter().copied().map(color).collect()), &mut *bind)?,
                Quality::Multicolored => kind(Filter::Atom(Atom::Multicolored), &mut *bind)?,
                Quality::Monocolored => kind(Filter::Atom(Atom::Monocolored), &mut *bind)?,
                Quality::Everything => Protects::Everything,
                Quality::ChosenColor => kind(Filter::Atom(Atom::Reference(ir::reference::UnresolvedRef::choice(ir::reference::Kind::Color))), &mut *bind)?,
                Quality::Filter(f) if characteristic(f) => kind(f.clone(), &mut *bind)?,
                Quality::Set(set) if of_kind(set) => kind(set.filter.clone(), &mut *bind)?,
                Quality::Set(set) => {
                    let set = expr::Set::compile(set, support, &mut *bind)?;
                    if !set.scalars().is_empty() { return unsupported("protection: a description that reads a chosen value"); }
                    Protects::Set(set)
                }
                Quality::Player(who) => Protects::Players(Players::compile(who, support, &mut *bind)?),
                other => return unsupported(format!("protection: from {}", crate::select::name(other).to_lowercase())),
            });
        }
        Ok(Protection { from, values: Vec::new(), controller: None })
    }

    /// The chosen values its qualities read, as they are (for checking a producer comes first).
    pub(crate) fn scalars(&self) -> Vec<crate::binding::Scalar> {
        let mut out = Vec::new();
        for p in &self.from {
            match p {
                Protects::Kind(f) => f.scalars(&mut out),
                Protects::Players(Players::Said(s)) => s.scalars(&mut out),
                Protects::Players(Players::All(set)) => out.extend(set.scalars()),
                Protects::Everything | Protects::Set(_) | Protects::Players(Players::Fixed(_)) => {}
            }
        }
        out
    }

    /// What its descriptions read of the game ([`expr::Reads`]).


    /// Protection as the effect granting it says it now: the values it reads and the players
    /// it names, and whose "you" its descriptions say (608.2h, 611.3a).
    pub(crate) fn capture(&self, game: &Game, ctx: &Ctx, locals: &dyn Locals) -> Protection {
        let mut values = Vec::new();
        let from = self.from.iter().map(|p| match p {
            Protects::Kind(f) => Protects::Kind(f.capture(game, ctx, locals, &mut values)),
            Protects::Players(Players::Said(s)) => Protects::Players(Players::Fixed(players(s.evaluate(game, ctx, locals)))),
            Protects::Players(Players::All(set)) => Protects::Players(Players::Fixed(players(set.members(game, ctx, locals)))),
            other => other.clone(),
        }).collect();
        Protection { from, values, controller: Some(ctx.controller) }
    }

    /// Does it protect the permanent `holder` from the source `source` names?
    fn from(&self, game: &Game, holder: ObjId, source: &Ctx) -> bool {
        // What the qualities say of "you" and "this" is of the holder, or of what granted it.
        let controller = self.controller.unwrap_or_else(|| game.controller(holder));
        self.protects(game, &Ctx::new(controller, holder), &self.values, source)
    }

    /// Does it protect from the source `source` names, its descriptions said in the frame
    /// `said` ("this", "you", the ability whose links they read) and reading `values`? A
    /// player's protection is said by the effect or static ability that gives it.
    pub(crate) fn protects(&self, game: &Game, said: &Ctx, values: &dyn Locals, source: &Ctx) -> bool {
        let Some(object) = source.source else {
            return self.from.contains(&Protects::Everything);
        };
        // A source's colors are settled in layer 5; asking only through that layer avoids
        // irrelevant ability dependencies. A departed source is as it last existed. The
        // source's own ability and frame say nothing of the holder's links or values.
        let ctx = Ctx {
            layer: Some(Layer::Abilities),
            source: said.source,
            controller: said.controller,
            ability: said.ability.clone(),
            effect: said.effect,
            was: None,
            x: None,
            face_then: None,
            then: None,
            ..*source
        };
        self.from.iter().any(|p| match p {
            Protects::Everything => true,
            Protects::Kind(f) => f.matches(game, &ctx, values, Target::Object(object)),
            Protects::Set(set) => set.contains(game, &ctx, values, Target::Object(object)),
            // a source's controller is its owner where no one else controls it (108.4a)
            Protects::Players(who) => {
                let named = match who {
                    Players::Said(s) => players(s.evaluate(game, &ctx, values)),
                    Players::All(set) => players(set.members(game, &ctx, values)),
                    Players::Fixed(players) => players.clone(),
                };
                named.contains(&source.controller)
            }
        })
    }
}

impl Players {
    /// "that player", "your opponents", "each of your opponents" (702.16i: protection from
    /// each of them).
    fn compile(who: &ir::Selector, support: &Support<'_>, bind: Bind<'_>) -> Res<Players> {
        if let Filter::All(parts) = &who.set.filter
            && let [Filter::Atom(Atom::Among(inner))] = parts.as_slice()
            && who.set.domain != ir::Domain::Player {
            return Players::compile(inner, support, bind);
        }
        Ok(match who.quant {
            ir::Quant::Definite(_) => Players::Said(expr::Subject::compile(who, bind)?),
            _ => Players::All(expr::Set::compile(&who.set, support, bind)?),
        })
    }
}

fn players(targets: Vec<Target>) -> Vec<PlayerId> {
    targets.into_iter().filter_map(|t| match t { Target::Player(p) => Some(p), Target::Object(_) => None }).collect()
}

impl Game {
    /// Is `target` protected from the source `source` names (702.16b–f)? A player has
    /// protection the rules in force give it (702.16b–c, e).
    pub(crate) fn protected_from(&self, target: Target, source: &Ctx) -> bool {
        let object = match target {
            Target::Object(object) => object,
            Target::Player(player) => return self.player_protected_from(player, source),
        };
        // Protection functions on the battlefield, not on a creature spell or graveyard card.
        if self.obj(object).zone != Zone::Battlefield {
            return false;
        }
        let target = self.seen_for(object, crate::layers::Aspect::Abilities);
        target.keywords.iter().filter_map(crate::keyword::Held::protection).any(|p| p.from(self, object, source))
    }
}
