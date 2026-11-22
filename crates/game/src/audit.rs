//! Opt-in privileged evidence. Never called while making a normal player snapshot.
use crate::{Game, ObjId};
use crate::binding::{AbilityKey, Origin};
use serde_json::{json, Value};

/// Frozen evidence for a restricted mana production (106.6, 607.2d).
/// Ordinary speculative payment retains typed identity and choices; wording and JSON
/// are materialized only when evidence is requested, never from the later source.
#[derive(Clone)]
pub struct RestrictedManaProvenance {
    physical: usize,
    source: ObjId,
    definition: crate::ability::FrozenDefinition,
    index: usize,
    name: String,
    chosen: Vec<(&'static str, crate::binding::Value)>,
    projection: std::sync::OnceLock<Option<Value>>,
}
impl std::fmt::Debug for RestrictedManaProvenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestrictedManaProvenance")
            .field("physical", &self.physical).field("source", &self.source)
            .field("definition", &self.definition).field("index", &self.index)
            .field("name", &self.name).field("chosen", &self.chosen).finish()
    }
}
// Equality is scoped to games sharing their immutable definition catalog, as is
// FrozenDefinition::eq. Catalog-local face indices are not cross-game identities.
impl PartialEq for RestrictedManaProvenance {
    fn eq(&self, other: &Self) -> bool {
        self.physical == other.physical && self.source == other.source
            && self.definition == other.definition && self.index == other.index
            && self.chosen == other.chosen
            // A name matters only where the original printed wording uses it.
            && (self.name == other.name || crate::prompt::frozen_ability_text(self.definition.def(), self.index, &self.name)
                == crate::prompt::frozen_ability_text(other.definition.def(), other.index, &other.name))
    }
}
impl Eq for RestrictedManaProvenance {}
impl RestrictedManaProvenance {
    pub(crate) fn freeze(game: &Game, source: ObjId, key: &AbilityKey) -> Option<std::sync::Arc<Self>> {
        let object = game.obj(source);
        let physical = object.card?;
        if !matches!(key.origin, Origin::Own) || object.copy_acquisition.is_some() { return None; }
        let definition = game.frozen_definition(source, &key.origin)?;
        use crate::ability::FrozenDefinition;
        use crate::copy::Definition;
        let FrozenDefinition::Face { face: Definition::Card(card, _) | Definition::Back(card), .. } = &definition else { return None; };
        if *card != physical { return None; }
        let mut chosen = Vec::new();
        for record in &object.choices {
            if record.link.0.origin != key.origin { continue; }
            use crate::binding::Value as V;
            let kind = match &record.value {
                V::CreatureType(_) => "creature_type", V::BasicLandType(_) => "basic_land_type",
                V::CardType(_) => "card_type", V::CardName(_) => "card_name",
                V::Color(_) => "color", V::Number(_) => "number", _ => return None,
            };
            if chosen.iter().any(|(existing, _)| *existing == kind) { return None; }
            chosen.push((kind, record.value.clone()));
        }
        // JSON maps compared by key, so choice record order must not affect identity.
        chosen.sort_by_key(|(kind, _)| *kind);
        Some(std::sync::Arc::new(Self { physical, source, definition, index: key.index,
            name: object.chars.name.clone(), chosen, projection: Default::default() }))
    }

    /// The original wire vocabulary, including refusal when wording is unavailable.
    pub fn audit(&self) -> Option<&Value> {
        self.projection.get_or_init(|| {
            let def = self.definition.def();
            let text = crate::prompt::frozen_ability_text(def, self.index, &self.name)?;
            let row = def.ability_row(self.index);
            let keyword = self.index.checked_sub(def.abilities.len()).and_then(|index| def.keyword_abilities.get(index))
                .and_then(|(row, _)| match &def.abilities[*row] {
                    ir::Ability::Keywords(items) if items.len() == 1 => Some(items[0].keyword.name()), _ => None,
                });
            let mut chosen = serde_json::Map::new();
            for (kind, value) in &self.chosen {
                use crate::binding::Value as V;
                let value = match value {
                    V::CreatureType(t) | V::BasicLandType(t) => json!(t.name()),
                    V::CardType(t) | V::CardName(t) => json!(t),
                    V::Color(c) => json!(c.word()), V::Number(n) => json!(n), _ => unreachable!("validated frozen choice"),
                };
                chosen.insert((*kind).to_owned(), value);
            }
            Some(json!({"physical_definition":self.physical,"ability":{
                "source":self.source.0,"origin":{"kind":"own"},"definition":def.name(),
                "paragraph":row+1,"declaration_index":self.index,"definition_identity":self.definition.identity(),
                "keyword":keyword,"text":text},"restriction":text,"chosen":chosen}))
        }).as_ref()
    }
}

/// A count's reset boundary, independent of effect expiry or the acting player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActivationUseLifetime { Turn, Incarnation }

/// Bounded fixture premise for one unchanged printed activated ability (602.5b).
#[derive(Debug, Clone)]
pub struct InitialActivationUse {
    pub source: ObjId,
    pub paragraph: usize,
    pub lifetime: ActivationUseLifetime,
    pub count: u32,
}

/// A keyword ability as its text says it, with its parameter, its first letter lowercase:
/// "first strike", "ward {2}", "protection from red", "enchant creature" (702).
fn keyword_text(instance:&ir::KeywordInstance) -> String {
    grammar::keyword::keyword().print_text(instance,false).unwrap_or_else(||instance.keyword.name().to_lowercase())
}

/// The keyword abilities an object holds (`passive`, [`crate::keyword::Held`]) as text,
/// then those of these rows that are only the activated, triggered or spell ability their
/// row is ("equip {3}"), which no [`crate::keyword::Held`] stands for.
fn keyword_texts<'a>(passive:&[crate::keyword::Held], rows:impl Iterator<Item=&'a ir::Ability>) -> Vec<String> {
    let held=passive.iter().map(|held|ir::KeywordInstance{keyword:held.keyword,param:held.param.clone()});
    let others=rows.flat_map(|row|match row {
        ir::Ability::Keywords(items) | ir::Ability::KeywordsWith{keywords:items,..}=>items.as_slice(), _=>&[],
    }).filter(|item|!crate::keyword::is_held(item)).cloned();
    held.chain(others).map(|item|keyword_text(&item)).collect()
}

impl Game {
    /// Immutable copiable token faces before entry or continuous effects (701.7b).
    pub(crate) fn audit_creation_specification(&self, token:&crate::copy::Token)->Value {
        let face=|value:&crate::copy::Values| {
            let c=&value.chars;
            let definition=value.definition(self);
            json!({"name":c.name,"types":c.types,"supertypes":c.supertypes,
                "subtypes":c.subtypes.iter().map(|s|s.name()).collect::<Vec<_>>(),
                "colors":c.colors,"power_toughness":c.pt,"mana_value":c.mana_value,
                "mana_cost":definition.and_then(|d|d.cost.as_ref()).map(|cost|cost.0.iter().map(|symbol|symbol.text()).collect::<String>()),
                "keywords":c.keywords.iter().map(|k|k.name()).collect::<Vec<_>>(),
                "predefined":c.predefined.map(|k|k.name()),
                "rows":definition.map(|d|d.abilities.iter().map(|ability|grammar::ability::abilities().ability.print_text(ability,true)).collect::<Vec<_>>()).unwrap_or_default()})
        };
        json!({"front":face(&token.faces.front),"back":token.faces.back.as_ref().map(face),"transformed":token.transformed})
    }

    /// Stable ability provenance for the currently offered creation replacement.
    /// The caller translates the source handle to its public seat label.
    pub fn audit_creation_replacement(&self, effect:&crate::operation::ReplacementId)->Value {
        match &effect.effect {
            crate::operation::ReplacementEffect::Ability {key,..} => self.audit_ability(Some(effect.source),key),
            _=>json!({"unavailable":"creation replacement has no ability producer"}),
        }
    }

    /// A resolved grant keeps the paragraph that gave it, even when that source
    /// changes or leaves (611.2a). The granted template's own row is not that
    /// paragraph. Authenticate the exact effect part and template, not its text.
    fn audit_grant_origin(&self, grant:&crate::binding::Grant, what:crate::ability::Template) -> Value {
        let mut origin=json!({"kind":"given","by":grant.by.map(|id|id.0),"acquisition":grant.stamp,"part":grant.part});
        if grant.by.is_none() {
            let effect=self.continuous.iter().find(|effect|effect.stamp==grant.stamp && effect.part==(grant.part>>16));
            if let Some(effect)=effect {
                if let (Some(producer),crate::layers::Change::Gives(templates))=(&effect.producer,&effect.change) {
                    if templates.get((grant.part & 0xffff) as usize)==Some(&what) {
                        origin["producer"]=json!({"source":producer.source.0,
                            "definition":self.defs[producer.definition].name(),
                            "definition_identity":{"card":producer.definition,"face":"Whole"},
                            "face":0,"paragraph":producer.paragraph});
                    }
                }
            }
        }
        origin
    }

    pub(crate) fn audit_ability(&self, source: Option<ObjId>, key: &AbilityKey) -> Value {
        let origin = match &key.origin {
            Origin::Own => json!({"kind":"own"}),
            Origin::Copied(stamp) => json!({"kind":"copied","acquisition":stamp}),
            Origin::Rule(_) => json!({"kind":"rule"}),
            Origin::Given {grant,what} => self.audit_grant_origin(grant,*what),
            Origin::Inherited(family) => json!({"kind":"inherited","family":family.identity()}),
        };
        let definition = source.and_then(|source|self.definition(source, &key.origin));
        let row = definition.map(|def|def.ability_row(key.index));
        let keyword = definition.and_then(|def|key.index.checked_sub(def.abilities.len()).and_then(|index|def.keyword_abilities.get(index)))
            .and_then(|(row,_)|match &definition.unwrap().abilities[*row] {
                ir::Ability::Keywords(items) if items.len()==1 => Some(items[0].keyword.name()), _=>None,
            });
        json!({"source":source.map(|id|id.0),"origin":origin,"definition":definition.map(|def|def.name()),
            "paragraph":row.map(|row|row+1),"declaration_index":key.index,
            "definition_identity":source.and_then(|source|self.frozen_definition(source,&key.origin)).map(|definition|definition.identity()),
            "keyword":keyword,"text":self.ability_text(source,key.clone())})
    }

    fn audit_cost_branches(&self, receiving:ObjId) -> Value {
        let mut rows=Vec::new();
        for selected in &self.obj(receiving).announced {
            let (Some(owner),Some(definition),Some(branch))=(&selected.owner,&selected.definition,&selected.branch) else {
                return json!({"unavailable":"retained cost branch has no frozen declaring definition"});
            };
            let Some(def)=definition.printed_cost_definition() else {
                return json!({"unavailable":"retained cost branch definition is not a whole printed face"});
            };
            if owner.index>=def.abilities.len()+def.keyword_abilities.len() {return json!({"unavailable":"retained cost owner row unavailable"});}
            let source=selected.source.unwrap_or(receiving);
            let row=def.ability_row(owner.index);
            let keyword=owner.index.checked_sub(def.abilities.len()).and_then(|index|def.keyword_abilities.get(index))
                .and_then(|(row,_)|match &def.abilities[*row] {ir::Ability::Keywords(items) if items.len()==1=>Some(items[0].keyword.name()),_=>None});
            let origin=match owner.origin {Origin::Own=>json!({"kind":"own"}),_=>return json!({"unavailable":"retained cost owner origin unsupported"})};
            let text=def.abilities.get(row).and_then(|ability|grammar::ability::abilities().ability.print_text(ability,true));
            rows.push(json!({"ability":{"source":source.0,"origin":origin,"definition":def.name(),"paragraph":row+1,"declaration_index":owner.index,
                "definition_identity":definition.identity(),"keyword":keyword,"text":text},"branch":branch}));
        }
        json!(rows)
    }
    /// Inventory of compiled choice sites, not a reconstruction of a legal payment.
    fn audit_cost_branch_choices(&self, source:ObjId) -> Value {
        let mut sites=Vec::new();
        let mut append=|plan:&crate::cost::Plan,ctx:&crate::select::Ctx| -> Result<(),String> {
            for site in plan.branch_choices(self,ctx)? {if !sites.contains(&site) {sites.push(site);}} Ok(())
        };
        let result=(|| -> Result<(),String> {
            if let Some(def)=self.def(source) {
                let ctx=crate::select::Ctx::for_spell(self.obj(source).owner,source);
                for plan in [&def.costs.normal,&def.costs.flashback,&def.costs.escape,&def.costs.harmonize,&def.costs.foretell].into_iter().flatten().filter_map(|result|result.as_ref().ok()) {append(plan,&ctx)?;}
                if let Ok(plan)=&def.costs.additional {append(plan,&ctx)?;}
                for (_,_,plan) in &def.costs.optional {if let Ok(plan)=plan {append(plan,&ctx)?;}}
                for (_,alternative) in &def.costs.alternatives {if let Ok(alternative)=alternative {append(&alternative.plan,&ctx)?;}}
            }
            for (index,member) in self.members(source).iter().enumerate() {
                if let Ok(plan)=self.ability_cost(source,member.key.clone()) {
                    let plan=self.reduce_power_up(source,member.key.clone(),plan);
                    append(&plan,&self.ability_ctx(self.obj(source).owner,source,index))?;
                }
            }
            Ok(())
        })();
        match result {Ok(())=>json!(sites),Err(reason)=>json!({"unavailable":reason})}
    }

    /// Active in-resolution offers for the native default/mana cost selector (608.2g).
    /// This is permission evidence, not legal-move enumeration or a retained payment
    /// claim. Alternative-only offers (e.g. madness) cannot authenticate a free cast.
    /// Reuse the casting query so zone/incarnation, player, face and discover ceiling
    /// checks agree with announcement; overlapping grants use its actual precedence.
    fn audit_offered_casts(&self, source:ObjId) -> Value {
        use crate::announcement::CastCost;
        use crate::card::CastAs;
        use crate::permission::{Costs, Window};
        let Some(object)=self.get_object(source) else {return json!([])};
        if object.zone==crate::Zone::Gone {return json!([]);}
        let Some(def)=self.def(source) else {return json!([])};
        let mut choices=def.cast_choices(object.zone==crate::Zone::Hand);
        // The native wire default is Whole. Modal front faces also accept it,
        // even though ordinary move listing spells that face as Face(0).
        // Query this selector separately: the permission need not alias its face.
        if !choices.contains(&CastAs::Whole) && def.spell(CastAs::Whole).is_ok() {
            choices.push(CastAs::Whole);
        }
        let mut rows=Vec::new();
        for player in 0..self.players.len() {
            for choice in choices.iter().copied() {
                if !def.spell(choice).is_ok_and(|spell|!spell.is(ir::CardType::Land)) {continue;}
                let Ok(Some(permission))=self.cast_permission(player,source,choice,CastCost::Mana) else {continue};
                if permission.window!=Window::Offered {continue;}
                let cast_as=match choice {
                    CastAs::Whole=>json!("whole"),CastAs::Half(index)=>json!(index),
                    CastAs::Fused=>json!("fused"),CastAs::Adventure=>json!("adventure"),
                    CastAs::Omen=>json!("omen"),CastAs::Face(index)=>json!({"face":index}),
                };
                rows.push(json!({"player":player,"cast_as":cast_as,
                    "without_paying":permission.costs==Costs::Free,"during_resolution":true}));
            }
        }
        json!(rows)
    }

    fn audit_casting_definition(&self, source:ObjId) -> Result<&crate::CardDef,&'static str> {
        self.audit_printed_casting_definition(source,false)
    }
    fn audit_printed_casting_definition(&self, source:ObjId, retained:bool) -> Result<&crate::CardDef,&'static str> {
        let object=self.get_object(source).ok_or("casting source incarnation unavailable")?;
        let original=object.card.map(|index|&self.defs[index]).ok_or("casting costs need an unchanged printed definition")?;
        let retained_face_down=retained && object.cast_announcement.is_some()
            && object.alternative.is_some_and(|row|original.costs.face_down.iter().any(|(at,_,_)|*at==row));
        if object.copied.is_some() || object.copy_acquisition.is_some() || object.token_faces.is_some()
            || !matches!(self.copy_origin(source),Origin::Own) || (self.face_down(source) && !retained_face_down)
            || object.hidden.is_some() || object.transformed || object.cast_as!=crate::card::CastAs::Whole
            || original.back.is_some() || !original.halves.is_empty() || original.is_room() {
            return Err("casting costs need an unchanged printed definition");
        }
        Ok(original)
    }

    /// A printed cost's owner remains the physical declaring row when a Morph
    /// spell's current definition is the blank face-down creature (708.4).
    fn audit_printed_cost_ability(&self, source:ObjId, def:&crate::CardDef, row:usize) -> Value {
        json!({"source":source.0,"origin":{"kind":"own"},"definition":def.name(),
            "definition_identity":{"card":self.obj(source).card,"face":"Whole"},
            "paragraph":def.ability_row(row)+1,"declaration_index":row,"keyword":null,
            "text":crate::prompt::frozen_ability_text(def,row,def.name())})
    }

    /// Definition metadata for authenticating a selected cost; not an enumeration of
    /// currently legal casts. Printed rows stay distinct from generated keyword abilities.
    fn audit_casting_costs(&self, source:ObjId) -> Value {
        let def=match self.audit_casting_definition(source) {Ok(def)=>def,Err(reason)=>return json!({"unavailable":reason})};
        if def.costs.optional.len()>1 || def.costs.alternatives.iter().enumerate().any(|(i,(row,_))|
            def.costs.alternatives[..i].iter().any(|(other,_)|other==row)) {
            return json!({"unavailable":"casting cost row does not identify one supported choice"});
        }
        let alternative_rows=def.costs.alternatives.iter().map(|(row,_)|*row)
            .chain(def.costs.face_down.iter().map(|(row,_,_)|*row)).collect::<Vec<_>>();
        if alternative_rows.iter().enumerate().any(|(i,row)|alternative_rows[..i].contains(row)) {
            return json!({"unavailable":"casting cost row does not identify one supported choice"});
        }
        json!({"optional":def.costs.optional.iter().map(|(row,kind,_)|json!({
            "ability":self.audit_ability(Some(source),&AbilityKey::printed(*row)),"kind":kind.name(),
        })).collect::<Vec<_>>(),"alternatives":alternative_rows.iter().map(|row|
            self.audit_printed_cost_ability(source,def,*row)).collect::<Vec<_>>()})
    }

    /// A generated spell owns these immutable copiable values (707.2b/707.10).
    /// Catalog identity authenticates a printed face; the donor's current zone,
    /// name or face is never used. Derived definitions need their full values.
    fn audit_spell_copy_definition(&self, source: ObjId) -> Value {
        use crate::copy::Definition;
        let object = self.obj(source);
        if object.stack_copy_of.is_none() || object.ability.is_some() { return Value::Null; }
        let values = self.copiable(source);
        match values.definition {
            Some(Definition::Card(card, face)) => json!({"card":card,"name":self.defs[card].name(),"face":format!("{face:?}")}),
            Some(Definition::Back(card)) => json!({"card":card,"name":self.defs[card].name(),"face":"back"}),
            _ => json!({"unavailable":"nonprinted spell copy needs complete intrinsic definition evidence"}),
        }
    }

    /// Actual finite announcement facts, including information retained by a resolving
    /// permanent (400.7d). A copy need not have been cast (707.10); absent cast_by is
    /// never evidence that its costs were unselected. Unsupported provenance stays unknown.
    fn audit_announced_costs(&self, source:ObjId) -> Value {
        let object=self.obj(source);
        let mut value=json!({"kicks":object.kicked,"optional":null,"alternative":null,
            "buyback":object.buyback_paid,"entwine":object.entwined,
            "replicate":object.replicated,"gift":object.gift});
        if object.kicked==0 && !object.optional_paid && object.alternative.is_none()
            && !object.buyback_paid && !object.entwined && object.replicated==0 && object.gift.is_none() {
            return value;
        }
        // A spell copy owns a frozen token definition (707.2b/707.10), not a
        // casting permission. Audit its inherited choices from that definition;
        // following the donor's current incarnation would lose its spell face.
        let definition = if object.stack_copy_of.is_some() && object.card_copy
            && object.ability.is_none() && !self.face_down(source) {
            self.def(source).ok_or("copied spell's frozen definition unavailable")
        } else { self.audit_printed_casting_definition(source,true) };
        let def=match definition {Ok(def)=>def,Err(reason)=>return json!({"unavailable":reason})};
        if object.kicked>0 && (crate::keyword::kicker_count(def)!=1
            || crate::keyword::kicker(def).is_none_or(|(_,multiple)|!multiple && object.kicked>1))
            || object.buyback_paid && crate::keyword::buyback_count(def)!=1
            || object.entwined && crate::keyword::entwine_count(def)!=1
            || object.replicated>0 && crate::keyword::replicate_count(def)!=1
            || object.gift.is_some_and(|player|player>=self.players.len() || crate::keyword::gift(def).is_none()) {
            return json!({"unavailable":"retained casting announcement has no unique printed cost"});
        }
        if object.optional_paid {
            let [(row,kind,_)]=def.costs.optional.as_slice() else {
                return json!({"unavailable":"retained optional cost has no unique printed owner"});
            };
            value["optional"]=json!({"ability":self.audit_ability(Some(source),&AbilityKey::printed(*row)),"kind":kind.name()});
        }
        if let Some(row)=object.alternative {
            // This is only the card's printed alternative cost. None says nothing about
            // a separate route's free/flashback cost or permission (118.9,601.2b).
            if def.costs.alternatives.iter().filter(|(at,_)|*at==row).count()
                + def.costs.face_down.iter().filter(|(at,_,_)|*at==row).count()!=1 {
                return json!({"unavailable":"retained alternative cost is not a unique printed cost"});
            }
            value["alternative"]=if def.costs.face_down.iter().any(|(at,_,_)|*at==row) {
                self.audit_printed_cost_ability(source,def,row)
            } else {self.audit_ability(Some(source),&AbilityKey::printed(row))};
        }
        value
    }

    /// The accepted route and completed substitutions, never an echo of an input action.
    fn audit_cast_announcement(&self, source: ObjId) -> Value {
        use crate::announcement::CastRoute;
        let Some(announcement) = &self.obj(source).cast_announcement else { return Value::Null };
        let route = match announcement.route {
            CastRoute::Normal => json!({"kind":"normal"}),
            CastRoute::Flashback => match announcement.flashback_cost {
                Some(index) => json!({"kind":"flashback","cost_index":index}),
                None => json!({"kind":"flashback"}),
            },
            CastRoute::Retrace => json!({"kind":"retrace"}),
            CastRoute::Escape => json!({"kind":"escape"}),
            CastRoute::Foretell => json!({"kind":"foretell"}),
            CastRoute::Harmonize { tapped } => json!({"kind":"harmonize","tapped":tapped.map(|id|id.0)}),
            CastRoute::Permission { source, ability } => json!({"kind":"permission","source":source.0,"ability":ability}),
        };
        // Freeze the declaring definition at acceptance. Looking up a current face here
        // could attribute an old permission to a new copy or a reclaimed incarnation.
        let permission = match &announcement.permission {
            Some((usage, Some(definition))) => {
                let def=definition.def();
                if usage.key.index >= def.abilities.len()+def.keyword_abilities.len() {
                    json!({"unavailable":"casting permission frozen owner row unavailable"})
                } else {
                    let row=def.ability_row(usage.key.index);
                    let origin=match &usage.key.origin {
                        Origin::Own=>json!({"kind":"own"}),
                        Origin::Copied(stamp)=>json!({"kind":"copied","acquisition":stamp}),
                        Origin::Rule(_)=>json!({"kind":"rule"}),
                        Origin::Given{grant,..}=>json!({"kind":"given","by":grant.by.map(|id|id.0),"acquisition":grant.stamp,"part":grant.part}),
                        Origin::Inherited(family)=>json!({"kind":"inherited","family":family.identity()}),
                    };
                    let keyword=usage.key.index.checked_sub(def.abilities.len()).and_then(|index|def.keyword_abilities.get(index))
                        .and_then(|(row,_)|match &def.abilities[*row] {ir::Ability::Keywords(items) if items.len()==1=>Some(items[0].keyword.name()),_=>None});
                    json!({"source":usage.source.0,"origin":origin,"definition":def.name(),
                        "paragraph":row+1,"declaration_index":usage.key.index,"definition_identity":definition.identity(),
                        "keyword":keyword,"text":def.abilities.get(row).and_then(|ability|grammar::ability::abilities().ability.print_text(ability,true))})
                }
            },
            _ if matches!(announcement.route, CastRoute::Permission { .. }) => json!({"unavailable":"casting permission exact frozen owner unavailable"}),
            _ => Value::Null,
        };
        let color = |color: ir::Color| match color { ir::Color::White=>"W",ir::Color::Blue=>"U",ir::Color::Black=>"B",ir::Color::Red=>"R",ir::Color::Green=>"G" };
        let payment=announcement.payment.as_ref().map(|payment|json!({
            "convoke":payment.convoke.iter().map(|(object,allocation)|json!({"object":object.0,"color":allocation.map(color)})).collect::<Vec<_>>(),
            "delve":payment.delve.iter().map(|object|object.0).collect::<Vec<_>>(),
        }));
        json!({"route":route,"permission":permission,"completed":payment.is_some(),"payment":payment})
    }

    fn audit_activation_ability(&self, source:ObjId, key:&AbilityKey) -> Result<Value,String> {
        // Activation keys are identity labels, not retention roots. Never follow a
        // reclaimed handle, or substitute the physical card's later incarnation.
        let object=self.get_object(source).ok_or("activation source incarnation no longer retained")?;
        match &key.origin {
            Origin::Own if object.card.is_some() && object.copy_acquisition.is_none()
                && matches!(self.copy_origin(source),Origin::Own) && !self.face_down(source)
                && !object.card.is_some_and(|index| {
                    let def=&self.defs[index];def.back.is_some() || !def.halves.is_empty() || def.is_room()
                }) => {},
            Origin::Rule(what) if key.index==0 && crate::activate::basic_mana_template(*what) => {},
            _=>return Err("activation use needs an unchanged printed or intrinsic basic-mana ability".into()),
        }
        if !matches!(self.ability_of(source,key),Some(ir::Ability::Activated{..})) {
            return Err("activation use does not name a represented activated ability".into());
        }
        Ok(self.audit_ability(Some(source),key))
    }

    fn activation_use_inventory(&self) -> Result<Vec<(ObjId,AbilityKey,ActivationUseLifetime,u32)>,String> {
        let mut uses:Vec<(ObjId,AbilityKey,ActivationUseLifetime,u32)>=Vec::new();
        let entries=self.activated.iter().map(|(source,key)|(*source,key,ActivationUseLifetime::Turn))
            .chain(self.audit_retained_objects().flat_map(|object|object.activation_uses.iter()
                .map(move |key|(object.id,key,ActivationUseLifetime::Incarnation))));
        for (source,key,lifetime) in entries {
            if let Some((_,_,_,count))=uses.iter_mut().find(|(s,k,l,_)|*s==source && k==key && *l==lifetime) {
                *count=count.checked_add(1).ok_or("activation count overflow")?;
            } else {
                self.audit_activation_ability(source,key)?;
                uses.push((source,key.clone(),lifetime,1));
            }
        }
        Ok(uses)
    }

    /// Complete actual counter inventory, including unrestricted activations.
    /// Partial reservations and unsupported identities never masquerade as no uses.
    fn audit_activation_uses(&self) -> Value {
        if !self.procedures.is_empty() || self.decision.is_some() {
            return json!({"unavailable":"activation inventory requires a settled priority boundary"});
        }
        let uses=match self.activation_use_inventory() {
            Ok(uses)=>uses,Err(reason)=>return json!({"unavailable":reason}),
        };
        let mut rows:Vec<_>=uses.into_iter().map(|(source,key,lifetime,count)|json!({
            "ability":self.audit_ability(Some(source),&key),"count":count,
            "lifetime":match lifetime {ActivationUseLifetime::Turn=>json!({"kind":"turn","turn":self.turn}),
                ActivationUseLifetime::Incarnation=>json!({"kind":"incarnation"})},
        })).collect();
        rows.sort_by_cached_key(|row|(row["ability"].to_string(),row["lifetime"].to_string()));
        json!(rows)
    }

    fn initial_activation_limits(&self, source:ObjId, paragraph:usize) -> Result<(AbilityKey,Option<u32>,bool),String> {
        if !self.battlefield.contains(&source) {return Err("initial activation source must be a live battlefield incarnation".into());}
        let key=AbilityKey::printed(paragraph.checked_sub(1).ok_or("invalid activation paragraph")?);
        self.audit_activation_ability(source,&key)?;
        if self.activation_keyword(source,key.clone()).is_some_and(|keyword|keyword.name()=="Power-up") {
            return Err("initial Power-up needs unrepresented entry-turn history".into());
        }
        let def=self.def(source).ok_or("initial activation definition unavailable")?;
        let program=crate::program::definition_program(def,key.index)
            .ok_or("initial activation printed program unavailable")?.map_err(|error|error.0)?;
        let mut turn=None;let mut incarnation=false;
        for limit in &program.announcement.limits {
            let count=match limit {
                crate::announcement::Limit::Once=>Some(1),crate::announcement::Limit::Times(n)=>Some(*n),
                crate::announcement::Limit::Lifetime=>{incarnation=true;None},_=>None,
            };
            if let Some(count)=count {turn=Some(turn.map_or(count,|old:u32|old.min(count)));}
        }
        if turn.is_none() && !incarnation {return Err("initial activation requires a fixed printed use limit".into());}
        Ok((key,turn,incarnation))
    }

    fn validate_initial_activation_uses(&self, uses:&[InitialActivationUse]) -> Result<(),String> {
        let mut identities=std::collections::BTreeSet::new();
        for row in uses {
            if row.count==0 || !identities.insert((row.source,row.paragraph,row.lifetime)) {
                return Err("initial activation counts must be positive and unique".into());
            }
            let (_,turn,incarnation)=self.initial_activation_limits(row.source,row.paragraph)?;
            let bound=match row.lifetime {
                ActivationUseLifetime::Turn=>turn.or(incarnation.then_some(1)),
                ActivationUseLifetime::Incarnation=>incarnation.then_some(1),
            }.ok_or("initial activation lifetime differs from its printed limit")?;
            if row.count>bound {return Err("initial activation count exceeds the unmodified printed limit".into());}
            if row.lifetime==ActivationUseLifetime::Turn && incarnation
                && !uses.iter().any(|total|total.source==row.source && total.paragraph==row.paragraph
                    && total.lifetime==ActivationUseLifetime::Incarnation && total.count>=row.count) {
                return Err("initial turn activation uses need their incarnation total".into());
            }
        }
        Ok(())
    }

    /// Install consumed limits without inventing activation actors, order, payments,
    /// events or triggered/action uses. Future reservations retain their own indices.
    pub fn set_initial_activation_uses(&mut self, uses:&[InitialActivationUse]) -> Result<(),String> {
        if !self.activated.is_empty() || !self.activation_started.is_empty()
            || self.audit_retained_objects().any(|object|!object.activation_uses.is_empty()) {
            return Err("initial activation uses require an empty usage inventory".into());
        }
        self.validate_initial_activation_uses(uses)?;
        for row in uses {
            let key=AbilityKey::printed(row.paragraph-1);
            for _ in 0..row.count {match row.lifetime {
                ActivationUseLifetime::Turn=>self.activated.push((row.source,key.clone())),
                ActivationUseLifetime::Incarnation=>self.obj_mut(row.source).activation_uses.push(key.clone()),
            }}
        }
        Ok(())
    }

    /// Small actual bookkeeping profile, independently of the much narrower
    /// executable initial-history profile. Missing facts never imply zero uses.
    fn audit_turn_bookkeeping(&self) -> Value {
        if self.players.len()!=2 || self.players.iter().any(|player|player.lost)
            || self.finished() || self.decision.is_some() || self.priority>=2 || self.passes>1 {
            return json!({"unavailable":"ordinary two-player priority boundary required"});
        }
        if !self.ordinary_turn_suffix() {
            return json!({"unavailable":"nonordinary turn schedule, phase suffix, skip or turn control"});
        }
        if self.players[1-self.active].land_played {
            return json!({"unavailable":"inactive land-play consumption outside ordinary profile"});
        }
        json!({"consecutive_passes":self.passes,
            "land_plays_this_turn":{"A":usize::from(self.players[0].land_played),"B":usize::from(self.players[1].land_played)},
            "schedule":{"kind":"ordinary_two_player","order":["A","B"],"next_turn":if self.active==0 {"B"}else{"A"}}})
    }

    /// Actual causal facts required by the bounded priority constructor. This is
    /// available only while they are still representable by the initial premise.
    pub fn audit_initial_history(&self) -> Result<Value,String> {
        if self.players.len()!=2 || self.turn==0 || self.active>=2
            || !self.ordinary_turn_suffix() || self.plan!=crate::turn::TurnPlan::at(self.turn,self.part,0)
            || self.active!=(self.turn as usize-1)%2
            || self.this_turn!=crate::history::ThisTurn::default()
            || self.log.start()!=self.log.end()
            || !self.activation_started.is_empty() || !self.ability_uses.is_empty()
            || !self.until.is_empty() || !self.lasting.is_empty() || !self.skips.is_empty() || !self.shields.is_empty()
            || !self.triggered.is_empty() || !self.delayed.is_empty() || !self.procedures.is_empty()
            || self.decision.is_some() || !self.stack.is_empty()
            || self.audit_retained_objects().any(|object|object.monstrous || object.renowned || object.solved)
            || !self.permissions.is_empty() || !self.linked_acquisitions.is_empty()
            || !self.commanders.is_empty() || self.companions.iter().any(Option::is_some)
            || self.exiled_with.iter().any(|(source,key,exiled,generation)|
                self.obj(*exiled).zone==crate::Zone::Exile && (!self.battlefield.contains(source)
                    || self.obj(*source).copy_acquisition.is_some()
                    || generation.is_some() || key.as_ref().is_none_or(|key|!matches!(key.origin,Origin::Own)))) {
            return Err("initial causal history no longer representable".into());
        }
        if self.audit_preventions().iter().any(|effect|effect.get("unavailable").is_some()) {
            return Err("initial prevention facts unavailable".into());
        }
        if self.audit_continuous().iter().any(|effect|effect.get("unavailable").is_some()) {
            return Err("initial continuous facts unavailable".into());
        }
        self.verify_initial_continuous_inventory()?;
        let activation_uses=self.activation_use_inventory()?.into_iter().map(|(source,key,lifetime,count)| {
            if !matches!(key.origin,Origin::Own) {return Err("initial activation origin unavailable".to_string());}
            Ok(InitialActivationUse{source,paragraph:key.index+1,lifetime,count})
        }).collect::<Result<Vec<_>,String>>()?;
        self.validate_initial_activation_uses(&activation_uses)?;
        let mut control=serde_json::Map::new();
        for id in &self.battlefield {
            let since=match self.obj(*id).since {0=>"before_turn",n if n==self.turn=>"this_turn",
                _=>return Err("initial control history unavailable".into())};
            control.insert(id.0.to_string(),json!(since));
        }
        Ok(json!({"turn_structure":"ordinary_A_first","pending":"none","effects":if self.prevention.is_empty() && self.continuous.is_empty(){"intrinsic_only"}else{"represented_only"},
            "turn_events":"empty","previous_turn_spells":0,"land_plays_this_turn":{"A":usize::from(self.players[0].land_played),"B":usize::from(self.players[1].land_played)},"ability_uses":if activation_uses.is_empty(){"none"}else{"represented_activations_only"},"consecutive_passes":self.passes,"control_since":control}))
    }

    /// Privileged fixture premise, not an exile event (607.1, 607.2a).
    /// Only an own printed writer linked by a compiled reader is constructible.
    pub fn set_initial_linked_exile(&mut self, source:ObjId, paragraph:usize, exiled:ObjId) -> Result<(),String> {
        if !self.battlefield.contains(&source) || !self.exile.contains(&exiled)
            || self.obj(source).copy_acquisition.is_some() || self.obj(exiled).token()
            || self.obj(exiled).hidden.is_some() || self.obj(exiled).foretold.is_some()
            || self.exiled_with.iter().any(|(_,_,id,_)|*id==exiled) {
            return Err("unsupported initial linked-exile endpoints".into());
        }
        let def=self.def(source).ok_or("linked source definition unavailable")?;
        let writer=paragraph.checked_sub(1).filter(|row|*row<def.abilities.len())
            .ok_or("linked writer paragraph unavailable")?;
        if !def.programs.iter().any(|row|row.linked_exile_writers.contains(&writer)) {
            return Err("initial linked-exile producer is not a printed linked writer".into());
        }
        self.state_mut().exiled_with.push((source,Some(AbilityKey{origin:Origin::Own,index:writer}),exiled,None));
        Ok(())
    }

    fn audit_preventions(&self) -> Vec<Value> {
        self.prevention.iter().map(|effect| {
            let (Some(source),Some(key),Some(remaining),Some(crate::statement::Affected::Things(to)))=
                (effect.ctx.source,effect.ctx.ability.as_ref(),effect.remaining,effect.to.as_ref())
                else {return json!({"unavailable":"only fixed-recipient finite prevention is represented"})};
            if effect.by.is_some() || effect.to_or_by || !matches!(key.origin,Origin::Own)
                || !matches!(self.copy_origin(source),Origin::Own) || self.face_down(source)
                || self.obj(source).copy_acquisition.is_some()
                || self.obj(source).card.is_some_and(|index| {
                    let def=&self.defs[index];def.back.is_some() || !def.halves.is_empty() || def.is_room()
                }) {
                return json!({"unavailable":"prevention producer or source restrictions unavailable"});
            }
            let target=|to:&crate::Target|match to {
                crate::Target::Player(player)=>json!({"player":player}),
                crate::Target::Object(object)=>json!({"object":object.0}),
            };
            json!({"identity":{"occurrence":effect.id.source.0,"part":effect.id.part()},
                "producer":self.audit_ability(Some(source),key),"controller":effect.ctx.controller,
                "recipients":to.iter().map(target).collect::<Vec<_>>(),
                "damage":match effect.kind {ir::event::DamageKind::Any=>"any",ir::event::DamageKind::Combat=>"combat",ir::event::DamageKind::Noncombat=>"noncombat"},
                "remaining":remaining,"expires":{"kind":"end_of_turn","turn":self.turn}})
        }).collect()
    }

    fn audit_continuous_change(change:&crate::layers::Change) -> Option<Value> {
        match change {
            crate::layers::Change::Control(player)=>Some(json!({"controller":player})),
            crate::layers::Change::Pt(power,toughness)=>Some(json!({"add_pt":{"power":power,"toughness":toughness}})),
            crate::layers::Change::BasePt(power,toughness)=>Some(json!({"base_pt":{"power":power,"toughness":toughness}})),
            crate::layers::Change::Gains(held) if held.len()==1 && held[0]==crate::keyword::Held::plain(held[0].keyword)
                =>Some(json!({"grant":held[0].keyword.name().to_lowercase()})),
            _=>None,
        }
    }

    /// Ordered fixed-subject resolved effects; unsupported causes remain explicit.
    fn audit_continuous(&self) -> Vec<Value> {
        let mut groups:Vec<Vec<&crate::layers::Continuous>>=Vec::new();
        for effect in &self.continuous {
            if let Some(group)=groups.iter_mut().find(|g|g[0].stamp==effect.stamp) {group.push(effect);}
            else {groups.push(vec![effect]);}
        }
        groups.sort_by_key(|group|group[0].stamp);
        groups.into_iter().map(|mut group| {
            group.sort_by_key(|effect|effect.part);
            let first=group[0];
            let unavailable=|reason:&str|json!({"identity":first.stamp,"unavailable":reason});
            let Some(producer)=first.producer.as_ref() else {return unavailable("unchanged plain printed producer unavailable")};
            let Some(changes)=group.iter().map(|effect|Self::audit_continuous_change(&effect.change)).collect::<Option<Vec<_>>>()
                else {return unavailable("only resolved control and passive haste changes are represented")};
            if group.iter().any(|effect|effect.producer!=first.producer || effect.affected!=first.affected || effect.lifetime!=first.lifetime) {
                return unavailable("effect group parts have different producer, subjects or duration");
            }
            let expires=match first.lifetime {
                crate::layers::Lifetime::Turn=>json!({"kind":"end_of_turn","turn":self.turn}),
                crate::layers::Lifetime::Forever=>json!({"kind":"indefinite"}),
                _=>return unavailable("conditional continuous lifetime unavailable"),
            };
            json!({"identity":first.stamp,"producer":{"source":producer.source.0,
                "definition":self.defs[producer.definition].name(),"paragraph":producer.paragraph},
                "controller":producer.controller,
                "subjects":first.affected.iter().map(|id|id.0).collect::<Vec<_>>(),"changes":changes,"expires":expires})
        }).collect()
    }

    fn initial_fixed_target(program:&crate::program::Program, operand:&crate::program::action::Operand, depth:usize) -> bool {
        use crate::program::{Instr,action::Operand};
        if depth>program.instrs.len() {return false;}
        match operand {
            Operand::Targets(0)=>true,
            Operand::Reference(slot)=>program.instrs.iter().any(|i|match i {
                Instr::Capture{operand,out} if out==slot=>Self::initial_fixed_target(program,operand,depth+1),_=>false,
            }),
            _=>false,
        }
    }

    /// Allocate an identity label for a declared departed spell producer. It carries
    /// no historical VM, cast/payment facts, targets or executable continuation.
    pub fn initial_continuous_source(&mut self, physical:ObjId) -> ObjId {
        let object=self.obj(physical);
        let (card,chars,owner,card_id)=(object.card,object.chars.clone(),object.owner,object.card_id);
        let source=self.new_object(card,chars,owner,owner,crate::Zone::Gone);
        self.obj_mut(source).card_id=card_id;
        self.obj_mut(source).left=Some(crate::Zone::Stack);
        source
    }

    /// Install one resolved printed spell effect without replaying instructions,
    /// untapping, ETBs, targeting events, control transitions or turn history.
    pub fn set_initial_continuous(&mut self, source:ObjId, paragraph:usize,
        controller:usize, subjects:Vec<ObjId>, declared_changes:&Value, expires:&Value) -> Result<(),String> {
        use crate::program::{Instr,change};
        use crate::layers::{Change,Continuous,Lifetime,ResolvedProducer};
        if self.obj(source).zone!=crate::Zone::Gone || controller>=self.players.len() || subjects.len()!=1
            || !self.battlefield.contains(&subjects[0]) || !self.seen(subjects[0],None).types.contains(&ir::CardType::Creature) {
            return Err("initial continuous effect requires one live creature subject".into());
        }
        let definition=self.obj(source).card.ok_or("continuous producer definition unavailable")?;
        let def=&self.defs[definition];
        let index=paragraph.checked_sub(1).ok_or("invalid continuous producer paragraph")?;
        if !def.chars().types.iter().any(|t|matches!(t,ir::CardType::Instant|ir::CardType::Sorcery))
            || def.back.is_some() || !def.halves.is_empty() || def.is_room()
            || !matches!(def.abilities.get(index),Some(ir::Ability::Text(_))) {
            return Err("initial continuous producer requires an immutable printed spell".into());
        }
        let program=crate::program::definition_program(def,index).ok_or("continuous printed program unavailable")?.map_err(|e|e.0)?;
        if program.targets.len()!=1 {return Err("initial continuous producer needs one fixed target group".into());}
        // Only the frozen first target or its direct captured reference is admitted.
        // Other producer instructions are not replayed by this premise.
        let mut matching=Vec::new();
        for instruction in &program.instrs {
            let Instr::Continuous{objects,changes:compiled,each,duration}=instruction else {continue};
            if objects.len()!=1 || !Self::initial_fixed_target(&program,&objects[0],0) || each.is_some() {continue;}
            let changes=compiled.iter().map(|change|change.initial_fixed(controller)
                .filter(|change|Self::audit_continuous_change(change).is_some())).collect::<Option<Vec<Change>>>();
            let Some(changes)=changes else {continue};
            let observed:Vec<_>=changes.iter().map(|c|Self::audit_continuous_change(c).unwrap()).collect();
            if &json!(observed)!=declared_changes {continue;}
            let lifetime=match duration {
                change::Duration::Turn if expires==&json!({"kind":"end_of_turn","turn":self.turn})=>Lifetime::Turn,
                change::Duration::Forever if expires==&json!({"kind":"indefinite"})=>Lifetime::Forever,
                _=>continue,
            };
            matching.push((changes,lifetime));
        }
        if matching.len()!=1 {return Err("initial continuous semantics must match one unambiguous printed effect".into());}
        let (changes,lifetime)=matching.pop().unwrap();
        let producer=Some(std::sync::Arc::new(ResolvedProducer{source,definition,paragraph,controller}));
        let stamp=self.tick();
        for (part,change) in changes.into_iter().enumerate() {
            self.continuous.push(Continuous{producer:producer.clone(),change,affected:subjects.clone(),stamp,part:part as u32,lifetime});
        }
        Ok(())
    }

    /// Complete inventory for the bounded plain spell family: no omitted siblings,
    /// extra multiplicity, changed subjects/controller or reordered creation parts.
    pub fn verify_initial_continuous_inventory(&self) -> Result<(),String> {
        use crate::program::{Instr,action::{Action,Operand},change};
        let groups=self.audit_continuous();
        let mut sources=std::collections::BTreeSet::new();
        for first in &self.continuous {
            let producer=first.producer.as_ref().ok_or("initial continuous producer unavailable")?;
            if !sources.insert(producer.source) {continue;}
            let def=&self.defs[producer.definition];
            if def.programs.len()!=1 {return Err("initial continuous inventory requires one plain printed spell row".into());}
            let program=crate::program::definition_program(def,producer.paragraph-1)
                .ok_or("initial continuous inventory program unavailable")?.map_err(|e|e.0)?;
            let fixed=|objects:&[Operand]|objects.len()==1 && Self::initial_fixed_target(&program,&objects[0],0);
            let mut expected=Vec::new();
            for instruction in &program.instrs {
                match instruction {
                    Instr::Capture{operand,..} if Self::initial_fixed_target(&program,operand,0)=>{},
                    Instr::Action{action:Action::Tap{objects,tapped:false},..} if fixed(objects)=>{},
                    Instr::Continuous{objects,changes,each:None,duration} if fixed(objects)=>{
                        let changes=changes.iter().map(|change|change.initial_fixed(producer.controller)
                            .filter(|change|Self::audit_continuous_change(change).is_some()))
                            .collect::<Option<Vec<_>>>().ok_or("unsupported initial continuous sibling change")?;
                        let changes:Vec<_>=changes.iter().map(|change|Self::audit_continuous_change(change).unwrap()).collect();
                        let expires=match duration {
                            change::Duration::Turn=>json!({"kind":"end_of_turn","turn":self.turn}),
                            change::Duration::Forever=>json!({"kind":"indefinite"}),
                            _=>return Err("unsupported initial continuous sibling lifetime".into()),
                        };
                        expected.push(json!({"changes":changes,"expires":expires,
                            "subjects":first.affected.iter().map(|id|id.0).collect::<Vec<_>>()}));
                    },
                    _=>return Err("initial continuous inventory requires unconditional fixed control/haste and untap only".into()),
                }
            }
            let own:Vec<_>=groups.iter().enumerate().filter(|(_,group)|group["producer"]["source"]==producer.source.0).collect();
            if own.windows(2).any(|pair|pair[1].0!=pair[0].0+1)
                || self.continuous.iter().filter(|effect|effect.producer.as_ref().is_some_and(|p|p.source==producer.source))
                    .any(|effect|effect.producer.as_ref()!=Some(producer)) {
                return Err("initial continuous producer groups are not one ordered resolution".into());
            }
            let actual:Vec<_>=own.iter().map(|(_,group)|json!({"changes":group["changes"],"expires":group["expires"],"subjects":group["subjects"]})).collect();
            if actual!=expected {return Err("incomplete or inconsistent initial continuous producer inventory".into());}
        }
        Ok(())
    }

    /// Install the captured result of a prior resolution, without replaying its
    /// exile, targeting, casting or payment. The retained identity is fixed (603.7c).
    pub fn set_initial_delayed_exile(&mut self, source:ObjId, paragraph:usize,
        controller:usize, clause:usize, exiled:&[ObjId]) -> Result<(),String> {
        let delayed=crate::program::initial::delayed(self,source,paragraph,controller,clause,exiled).map_err(|e|e.0)?;
        if self.delayed.iter().any(|prior|prior.source==source
            && crate::program::same_instance(prior.captured.program(),delayed.captured.program())) {
            return Err("duplicate initial delayed instruction occurrence".into());
        }
        self.delayed.push(delayed);
        Ok(())
    }

    pub fn verify_initial_delayed_inventory(&self)->Result<(),String> {
        crate::program::initial::verify_delayed_inventory(self).map_err(|error|error.0)
    }

    fn audit_delayed(&self) -> Vec<Value> {
        self.delayed.iter().map(|delayed| {
            let Some((paragraph,clause,exiled))=crate::program::initial::delayed_exile_facts(self,delayed)
                else {return json!({"unavailable":"delayed producer/bindings outside represented fixed exile family"})};
            let Some(definition)=self.obj(delayed.source).card else {return json!({"unavailable":"delayed printed definition unavailable"})};
            let lifetime=match delayed.lasts {
                crate::state::Lasts::Indefinite=>json!({"kind":"indefinite"}),
                crate::state::Lasts::ThisTurn=>json!({"kind":"end_of_turn","turn":self.turn}),
                crate::state::Lasts::UntilYourNextTurn=>json!({"kind":"until_your_next_turn","player":delayed.controller}),
            };
            json!({"producer":{"source":delayed.source.0,"definition":self.defs[definition].name(),"paragraph":paragraph},
                "controller":delayed.controller,"clause":clause,"retained":[{"kind":"exiled","objects":exiled.iter().map(|id|id.0).collect::<Vec<_>>()}],
                "when":{"kind":"step","step":delayed.pattern.initial_step(),"whose":"any","next":true},
                "once":delayed.once,"expires":lifetime})
        }).collect()
    }

    /// Actual waiting occurrences, only at the 603.3b ordering boundary. The
    /// order of this audit array is native presentation, never portable identity.
    pub fn audit_pending_trigger_order(&self) -> Value {
        let Some(crate::state::Decision::Order { abilities, .. }) = self.decision.as_ref() else {
            return json!({"occurrences":[]});
        };
        let occurrences:Vec<_>=abilities.iter().map(|trigger| {
            self.audit_pending_trigger(trigger).unwrap_or_else(|reason|json!({"unavailable":reason}))
        }).collect();
        json!({"occurrences":occurrences})
    }

    /// Authenticate immutable rule templates, never a matching text/name.
    fn audit_source_free_rule(&self, key:&AbilityKey)->Option<Value> {
        let Origin::Rule(definition)=&key.origin else {return None;};
        if key.index!=0 {return None;}
        if *definition==crate::monarch::Rule::EndStep.definition() {Some(json!({"rule":"725.2","sub":1}))}
        else if *definition==crate::monarch::Rule::CombatDamage.definition() {Some(json!({"rule":"725.2","sub":2}))}
        else if *definition==crate::speed::rule() {Some(json!({"rule":"702.179d"}))}
        else {None}
    }

    fn audit_rule_subjects(&self, bound:&crate::state::Bound)->Value {
        let subjects:Vec<_>=bound.inputs.iter().filter(|(kind,_)|*kind==crate::program::input::Input::Subject)
            .flat_map(|(_,subjects)|subjects.iter()).map(|subject|match subject {
                crate::Target::Object(id)=>json!({"object":id.0}),crate::Target::Player(player)=>json!({"player":player})
            }).collect();
        json!({"inputs":[{"kind":"subject","subjects":subjects}]})
    }

    fn audit_pending_trigger(&self, trigger:&crate::state::Triggered) -> Result<Value,String> {
        if trigger.source.is_none() {
            let key=AbilityKey{origin:trigger.origin.clone(),index:trigger.index};
            let rule=self.audit_source_free_rule(&key).ok_or("unknown source-free rule")?;
            let expected=crate::program::capture(self,crate::select::Ctx::rule(trigger.controller,key));
            if trigger.delayed.is_some() || trigger.captured.as_ref()!=Some(&expected) {
                return Err("source-free pending executable differs from the rule definition".into());
            }
            return Ok(json!({"ability":{"rule_ref":rule},"controller":trigger.controller,
                "bound":self.audit_rule_subjects(&trigger.bound)}));
        }
        let source=trigger.source.ok_or("source-free pending occurrence")?;
        if trigger.delayed.is_some() || !matches!(trigger.origin,Origin::Own) {
            return Err("delayed or nonprinted pending occurrence".into());
        }
        let key=AbilityKey{origin:trigger.origin.clone(),index:trigger.index};
        let ctx=crate::select::Ctx::new(trigger.controller,source).with_key(key.clone());
        let captured=trigger.captured.as_ref().and_then(|result|result.as_ref().ok())
            .ok_or("pending captured executable unavailable")?;
        // Equality checks every retained context/value as well as executable identity.
        // A new face, modified origin, retained X, linked generation, modal body or
        // reflexive captured values cannot be relabelled from the live source.
        let ordinary=crate::program::capture(self,ctx).map_err(|_|"pending printed executable unavailable")?;
        if captured!=&ordinary || captured.context().link_generation.is_some() || captured.choices().is_some() {
            return Err("pending captured context/program is not an unchanged plain printed ability".into());
        }
        let bound=self.audit_occurrence_bound(&trigger.bound,source,&key)?;
        Ok(json!({"ability":self.audit_ability(Some(source),&key),"controller":trigger.controller,"bound":bound}))
    }

    fn audit_stack_bound(&self, invocation:&crate::program::Invocation, bound:&crate::state::Bound) -> Result<Value,String> {
        let source=invocation.ctx.source.ok_or("source-free stack occurrence")?;
        let key=invocation.ctx.ability.as_ref().ok_or("stack ability unavailable")?;
        if !invocation.triggered || !matches!(key.origin,Origin::Own) {
            return Err("not an unchanged plain printed trigger".into());
        }
        let ctx=crate::select::Ctx::new(invocation.ctx.controller,source).with_key(key.clone());
        let ordinary=crate::program::capture(self,ctx).map_err(|_|"stack printed executable unavailable")?;
        if ordinary.context().link_generation.is_some() || ordinary.choices().is_some()
            || !ordinary.select(&[]).is_ok_and(|invocations|invocations.as_slice()==std::slice::from_ref(invocation)) {
            return Err("stack captured context/program is not an unchanged plain printed ability".into());
        }
        self.audit_occurrence_bound(bound,source,key)
    }

    fn audit_occurrence_bound(&self, bound:&crate::state::Bound, source:ObjId, key:&AbilityKey) -> Result<Value,String> {
        use crate::program::input::Input;
        if !bound.revealed.is_empty() || bound.linked_ability.is_some() || !bound.cost_exiled.is_empty() || bound.crew.is_some() {
            return Err("pending occurrence has unsupported retained bindings".into());
        }
        // Enter's controller has a stable event meaning (603.6a), unlike a
        // generic Argument(n). Other native argument families deliberately refuse.
        let definition=self.definition(source,&key.origin).ok_or("pending definition unavailable")?;
        let entry_argument=match definition.abilities.get(key.index) {
            Some(ir::Ability::Triggered{trigger:ir::event::Trigger::Event{pattern,..},..})
                if !pattern.subjects.is_empty() && pattern.actions.len()==1
                && pattern.actions[0].reading().is_some_and(|(verb,_)|verb==ir::Verb::Enter) => {
                    let action=&pattern.actions[0];
                    let roles:Vec<_>=crate::trigger::arguments(action).iter().map(|(role,_)|Some(*role))
                        .chain(crate::trigger::unsaid(action,true).into_iter().map(|(role,_)|role)).collect();
                    (roles==[Some(ir::Role::Under)]).then_some(0)
                },
            _=>None,
        };
        let target=|target:&crate::Target|match target {
            crate::Target::Object(id)=>json!({"object":id.0}),crate::Target::Player(player)=>json!({"player":player}),
        };
        let mut inputs=Vec::new();
        for (input,subjects) in &bound.inputs {
            let kind=match input {
                Input::Subject=>"subject",
                Input::Argument(index) if Some(*index)==entry_argument=>"entry_controller",
                _=>return Err("pending occurrence has unsupported event role".into()),
            };
            if subjects.len()>1 || (kind=="entry_controller" && subjects.iter().any(|subject|!matches!(subject,crate::Target::Player(_)))) {
                return Err("pending occurrence has unsupported grouped subjects".into());
            }
            inputs.push(json!({"kind":kind,"subjects":subjects.iter().map(target).collect::<Vec<_>>()}));
        }
        let transitions:Vec<_>=bound.became.iter().map(|(from,to)|json!({"from":{"object":from.0},"to":{"object":to.0}})).collect();
        Ok(json!({"inputs":inputs,"event_amount":bound.event_amount,"transitions":transitions}))
    }

    pub fn audit_game_objects(&self) -> Value {
        let mut objects = serde_json::Map::new();
        for object in self.audit_retained_objects() {
            let id = object.id;
            let members=self.members(id);
            let abilities: Vec<_> = members.iter().enumerate().map(|(index,member)| {
                let mut value=self.audit_ability(Some(id),&member.key);
                value["index"]=json!(index); value
            }).collect();
            let occurrences: Vec<_> = object.invocations.as_ref().and_then(|result|result.as_ref().ok()).into_iter().flatten().map(|invocation| {
                if invocation.ctx.source.is_none() {
                    if let Some(key)=invocation.ctx.ability.as_ref() {
                        if let Some(rule)=self.audit_source_free_rule(key) {
                            let exact=crate::program::capture(self,crate::select::Ctx::rule(invocation.ctx.controller,key.clone()))
                                .is_ok_and(|captured|captured.select(&[]).is_ok_and(|expected|expected.iter().any(|expected|expected==invocation)));
                            if exact && invocation.triggered {
                                return json!({"source":null,"rule_ref":rule,"origin":{"kind":"rule"},
                                    "controller":invocation.ctx.controller,"triggered":true,"bound":self.audit_rule_subjects(&object.bound)});
                            }
                        }
                    }
                }
                // A live source lookup is evidence only if it still names the exact
                // captured executable. Never relabel an old occurrence from a new face
                // or an expired copy definition at the same ability row.
                let ordinary=invocation.ctx.source.zip(invocation.ctx.ability.as_ref()).is_some_and(|(source,key)|
                    self.definition(source,&key.origin)
                        .and_then(|definition|crate::program::definition_selected(definition,key.index,
                            &object.modes,crate::program::ModeCount{entwined:object.entwined,
                                kicked:object.kicked>0,optional:object.optional_paid}))
                        .and_then(Result::ok).is_some_and(|expected|{
                            // Match the complete row group, including printed order
                            // and multiplicity; plain rows remain separate. They must be the
                            // very instances, not equal programs of another row or face.
                            let actual:Vec<_>=object.invocations.as_ref().and_then(|result|result.as_ref().ok())
                                .into_iter().flatten().filter(|other|
                                    other.ctx.source==invocation.ctx.source && other.ctx.ability==invocation.ctx.ability)
                                .map(|other|&other.program).collect();
                            expected.len()==actual.len() && expected.iter().zip(actual).all(|(e,a)|crate::program::same_instance(e,a))
                        }));
                // The occurrence may execute a nested delayed/reflexive body,
                // not the parent paragraph (603.7, 603.12). Authenticate exact
                // compiled ancestry; a coincident name/row or changed source
                // definition is never enough. Ordinary modal rows still need
                // the full ordered selection proof above.
                let created=invocation.triggered && invocation.ctx.source.zip(invocation.ctx.ability.as_ref())
                    .is_some_and(|(source,key)|self.definition(source,&key.origin)
                        .is_some_and(|def|crate::program::definition_creates_program(def,key.index,&invocation.program)));
                // Keywords create rule-defined delayed bodies, rather than a
                // compiled child of the keyword row. Its frozen producer must
                // still be the exact definition at the retained source.
                let keyword=invocation.keyword_producer.as_deref().filter(|(frozen,key)|
                    invocation.triggered && invocation.ctx.ability.as_ref().is_none_or(|declared|declared==key)
                    && crate::program::keyword_delayed_identity(&invocation.program).is_some_and(|(keyword,_,_)|
                        matches!(frozen.def().abilities.get(frozen.def().ability_row(key.index)),
                            Some(ir::Ability::Keywords(items)) if items.iter().any(|item|item.keyword.name()==keyword)))
                    && invocation.ctx.source.is_some_and(|source|self.frozen_definition(source,&key.origin).as_ref()==Some(frozen)));
                let verified=ordinary || created;
                let mut value=if verified { invocation.ctx.ability.as_ref().map(|key|self.audit_ability(invocation.ctx.source,key)).unwrap() }
                    else {json!({"source":invocation.ctx.source.map(|source|source.0),"unavailable":"captured definition differs from available source definition"})};
                if let Some((_,key))=keyword {
                    value=self.audit_ability(invocation.ctx.source,key);
                    let (keyword,rule,text)=crate::program::keyword_delayed_identity(&invocation.program).expect("authenticated rule body");
                    value["keyword"]=json!(keyword);
                    value["rule"]=json!(rule);
                    value["text"]=json!(text);
                }
                if !value.is_null() { value["triggered"]=json!(invocation.triggered); }
                if verified {
                    match self.audit_stack_bound(invocation,&object.bound) {
                        Ok(bound)=>{value["bound"]=bound;value["controller"]=json!(invocation.ctx.controller);},
                        Err(reason)=>value["bound_unavailable"]=json!(reason),
                    }
                }
                value
            }).collect();
            let seen=self.seen(id,None);
            let printed=self.printed_seen(id);
            let rows:Vec<_>=match self.def(id) {
                Some(def)=>def.abilities.iter().map(|ability|grammar::ability::abilities().ability.print_text(ability,true)).collect(),
                // A token defined by its characteristics alone has only the keyword
                // abilities it was created with as its text (111.4).
                None if object.chars.keywords.is_empty()=>Vec::new(),
                None=>vec![grammar::ability::abilities().ability.print_text(&ir::Ability::Keywords(object.chars.keywords.iter()
                    .map(|keyword|ir::KeywordInstance{keyword:*keyword,param:ir::token::KeywordParam::None}).collect()),true)],
            };
            let base=json!({"name":object.chars.name,
                "colors":printed.colors.iter().map(|value|value.word()).collect::<Vec<_>>(),
                "supertypes":printed.supertypes.iter().map(|value|format!("{value:?}")).collect::<Vec<_>>(),
                "types":printed.types.iter().map(|value|format!("{value:?}")).collect::<Vec<_>>(),
                "subtypes":printed.subtypes.iter().map(|value|value.name()).collect::<Vec<_>>(),
                "power":printed.pt.map(|pt|pt.0),"toughness":printed.pt.map(|pt|pt.1),
                "keywords":keyword_texts(&printed.keywords,self.def(id).into_iter().flat_map(|def|def.abilities.iter())),"rows":rows});
            objects.insert(id.0.to_string(),json!({"timestamp":object.stamp,"historical_physical":self.physical_card_identity(id),"physical":if self.is(crate::Target::Object(id),ir::Sort::Card) {self.physical_card_identity(id)}else{None},
                "is_card":self.is(crate::Target::Object(id),ir::Sort::Card),
                // Retained engine facts, never a replay action echoed back. Cost
                // alternatives (`Obj::announced`) are distinct from this mapping.
                "announced":{"cast":self.audit_cast_announcement(id),"x":object.x,"spell_x":object.spell_x,"modes":object.modes,
                    "division":object.division,"cast_by":object.cast_by,
                    "cast_from":object.cast_from.map(|zone|format!("{zone:?}").to_lowercase()),
                    "cost_alternatives":object.announced.iter().map(|row|&row.text).collect::<Vec<_>>(),"cost_branches":self.audit_cost_branches(id),"costs":self.audit_announced_costs(id)},
                "casting_costs":self.audit_casting_costs(id),"offered_casts":self.audit_offered_casts(id),"cost_branch_choices":self.audit_cost_branch_choices(id),
                "properties":{"name":object.chars.name,"owner":object.owner,"controller":seen.controller,
                    "supertypes":seen.supertypes.iter().map(|value|format!("{value:?}")).collect::<Vec<_>>(),
                    "types":seen.types.iter().map(|value|format!("{value:?}")).collect::<Vec<_>>(),
                    "subtypes":seen.subtypes.iter().map(|value|value.name()).collect::<Vec<_>>(),
                    "colors":seen.colors.iter().map(|value|value.word()).collect::<Vec<_>>(),
                    "power":seen.pt.map(|pt|pt.0),"toughness":seen.pt.map(|pt|pt.1),
                    "tapped":object.tapped,"damage":object.damage,"counters":object.counters,"solved":object.solved},
                "stack_copy_of":object.stack_copy_of.map(|id|id.0),
                "spell_copy_definition":self.audit_spell_copy_definition(id),
                "cast_as":if object.cast_as==crate::card::CastAs::Fused {json!("fused")} else {Value::Null},
                "prepared":object.prepared,"prepared_copy":object.prepared_copy.map(|id|id.0),
                "prepare_source":object.prepare_source.map(|id|id.0),
                "copy_acquisition":object.copy_acquisition.as_ref().map(|acquisition|json!({
                    "donor":acquisition.donor.0,"producer":acquisition.producer.as_ref().map(|key|key.index+1),
                    "definition":acquisition.before.definition(self).map(|def|def.name()),
                    "original_rows":acquisition.before.definition(self).map(|def|def.abilities.iter().map(|ability|grammar::ability::abilities().ability.print_text(ability,true)).collect::<Vec<_>>())})),
                "definition_rows":self.def(id).map(|def|def.abilities.iter().map(|ability|grammar::ability::abilities().ability.print_text(ability,true)).collect::<Vec<_>>()),
                "definition":object.card.map(|index|self.defs[index].name()),"physical_definition":object.card,"owner":object.owner,
                "zone":format!("{:?}",object.zone),"token":object.token(),"abilities":abilities,
                "keywords":seen.keywords.iter().map(|held|held.keyword.name().to_lowercase()).collect::<Vec<_>>(),
                "held":keyword_texts(&seen.keywords,members.iter().map(|member|&**member)),
                "base":base,"abilities_removed":!seen.base_abilities,
                "occurrences":occurrences}));
        }
        let links:Vec<_>=self.exiled_with.iter().filter(|(_,_,exiled,_)|self.obj(*exiled).zone==crate::Zone::Exile).map(|(source,key,exiled,generation)| {
            match key {
                Some(key) if matches!(key.origin,Origin::Own) && generation.is_none()
                    // Entry-copy provenance is immutable and projected by the existing
                    // copied/copy-exception selector. A later resolving copy or
                    // face-changing source cannot authenticate an old Own row.
                    && matches!(self.copy_origin(*source),Origin::Own)
                    && !self.face_down(*source)
                    && !self.obj(*source).card.is_some_and(|index| {
                        let physical=&self.defs[index];
                        physical.back.is_some() || !physical.halves.is_empty() || physical.is_room()
                    }) =>
                    json!({"source":source.0,"ability":self.audit_ability(Some(*source),key),"exiled":exiled.0}),
                _=>json!({"source":source.0,"exiled":exiled.0,"unavailable":"linked-exile producer definition changed or origin is unsupported"}),
            }
        }).collect();
        let mut audit=json!({"casts_this_turn":self.audit_casts_this_turn(),"turn_bookkeeping":self.audit_turn_bookkeeping(),"activation_uses":self.audit_activation_uses(),"objects":objects,"linked_exile":links,"prevention":self.audit_preventions(),"continuous_effects":self.audit_continuous(),"delayed_triggers":self.audit_delayed(),
            "base_controllers":self.continuous.iter().flat_map(|c|c.affected.iter()).filter(|id|self.battlefield.contains(id))
                .map(|id|(id.0.to_string(),json!(self.obj(*id).controller))).collect::<serde_json::Map<_,_>>(),"combat":{
            "attackers":self.combat.attackers.iter().map(|(attacker,defender)|json!({"attacker":attacker.0,"defender":match defender {crate::Target::Player(player)=>json!({"player":player}),crate::Target::Object(object)=>json!({"object":object.0})}})).collect::<Vec<_>>(),
            "blockers":self.combat.blockers.iter().map(|(blocker,attacker)|json!([blocker.0,attacker.0])).collect::<Vec<_>>(),
            "blocked":self.combat.blocked.iter().map(|id|id.0).collect::<Vec<_>>(),"declared":self.combat.blockers_declared,
            // 510.4: a second combat damage step follows the first when these had first
            // strike or double strike as it began.
            "damage_steps":self.combat.damage_steps,
            "first_strikers":self.combat.first_strikers.as_ref().map(|ids|ids.iter().map(|id|id.0).collect::<Vec<_>>())}});
        if matches!(&self.decision,Some(crate::state::Decision::Order{..})) {
            audit["pending_triggers"]=self.audit_pending_trigger_order()["occurrences"].clone();
        }
        audit
    }
}

#[cfg(test)]
mod pending_tests {
    use super::*;
    #[test]
    fn pending_export_refuses_unrepresented_bindings_and_captured_context() {
        let definition=crate::CardDef::new("Audit witness","","Enchantment",
            "Whenever a creature you control enters, draw a card.",None).unwrap();
        let mut game=Game::new(vec![definition],2,20);
        let source=game.put(0,0,crate::Zone::Battlefield);
        let ctx=crate::select::Ctx::new(0,source).with_ability(0);
        let ordinary=crate::state::Triggered {source:Some(source),controller:0,origin:Origin::Own,index:0,
            bound:crate::state::Bound::default(),delayed:None,
            captured:Some(crate::program::capture(&game,ctx.clone()))};
        assert!(game.audit_pending_trigger(&ordinary).is_ok());
        let mut hidden=ordinary.clone();hidden.bound.revealed.push(source);
        assert!(game.audit_pending_trigger(&hidden).unwrap_err().contains("unsupported retained bindings"));
        let mut changed=ordinary.clone();let mut ctx=ctx;ctx.x=Some(7);
        changed.captured=Some(crate::program::capture(&game,ctx));
        assert!(game.audit_pending_trigger(&changed).unwrap_err().contains("captured context/program"));
        let mut unknown_role=ordinary;unknown_role.bound.inputs.push((crate::program::input::Input::Cost(0),vec![crate::Target::Object(source)]));
        assert!(game.audit_pending_trigger(&unknown_role).unwrap_err().contains("unsupported event role"));
    }
}

#[cfg(test)]
mod announced_modes_tests {
    use crate::{CardDef, Game, Move, Zone};

    #[test]
    fn retained_modal_program_must_match_an_actually_selected_mode() {
        let definition=CardDef::new("Modal proof","{0}","Instant",
            "Choose one or both —\n• You gain 1 life.\n• You gain 2 life.",None).unwrap();
        crate::play::check(&definition).unwrap();
        let mut game=Game::new(vec![definition],2,20);
        let card=game.put(0,0,Zone::Hand);
        game.act(0,Move::Modes(vec![0,1],Box::new(Move::Cast{card,targets:vec![]}))).unwrap();
        let spell=game.stack[0];
        let occurrence=|game:&Game|game.audit_game_objects()["objects"][spell.0.to_string()]["occurrences"][0].clone();
        assert!(occurrence(&game).get("unavailable").is_none());
        // A checker regression that accepts any modal program would falsely pass
        // this mismatch between retained selection and immutable executable.
        game.obj_mut(spell).modes=vec![1];
        assert!(occurrence(&game).get("unavailable").is_some());
        game.obj_mut(spell).modes=vec![0,1];
        let captured=game.obj(spell).invocations.clone().unwrap().unwrap();
        let mut missing=captured.clone();missing.pop();
        game.obj_mut(spell).invocations=Some(Ok(missing));
        assert!(occurrence(&game).get("unavailable").is_some(),"missing selected mode must not authenticate");
        game.obj_mut(spell).invocations=Some(Ok(vec![captured[0].clone(),captured[0].clone()]));
        assert!(occurrence(&game).get("unavailable").is_some(),"duplicated selected ID must not replace another mode");
        game.obj_mut(spell).invocations=Some(Ok(captured.into_iter().rev().collect()));
        assert!(occurrence(&game).get("unavailable").is_some(),"modal execution order must stay printed order");
    }
}

#[cfg(test)]
mod created_occurrence_tests {
    use crate::{CardDef, Game, Zone};
    use crate::program::{self, Instr};

    #[test]
    fn r603_7_and_603_12_child_bodies_name_their_declaring_paragraph() {
        for text in ["Draw a card. When you next cast a creature spell this turn, you draw a card.",
                     "You may discard a card. When you do, draw a card."] {
            let definition=CardDef::new("Occurrence witness","{0}","Sorcery",text,None).unwrap();
            crate::play::check(&definition).unwrap();
            let other=CardDef::new("Different definition","{0}","Sorcery",text,None).unwrap();
            let mut game=Game::new(vec![definition,other],2,20);
            let source=game.put(0,0,Zone::Graveyard);
            let parent=program::definition_program(&game.defs[0],0).unwrap().unwrap();
            let child=parent.instrs.iter().find_map(|instruction|match instruction {
                Instr::Delayed{program,..}|Instr::Reflexive{program,..}=>Some(program.clone()), _=>None,
            }).expect("the printed paragraph creates a triggered body");
            let ctx=crate::select::Ctx::new(0,source).with_ability(0);
            let captured=program::reflexive_capture(ctx,child);
            let stack=game.put(0,0,Zone::Stack);
            game.obj_mut(stack).ability=Some((Some(source),0));
            game.obj_mut(stack).invocations=Some(captured.select(&[]));
            let occurrence=|g:&Game|g.audit_game_objects()["objects"][stack.0.to_string()]["occurrences"][0].clone();
            let row=occurrence(&game);
            assert!(row.get("unavailable").is_none(),"{row}");
            assert_eq!(row["source"],source.0);
            assert_eq!(row["paragraph"],1);
            // Matching words in another compiled definition do not prove that
            // it created this occurrence (113.7a); never relabel it by row/name.
            game.obj_mut(source).card=Some(1);
            assert!(occurrence(&game).get("unavailable").is_some());
        }
    }
}

#[cfg(test)]
mod offered_cast_audit_tests {
    use super::*;
    use crate::{CardDef, Zone};
    use crate::permission::{Costs, Until};

    fn offer(game:&Game,card:ObjId) -> Value {
        game.audit_game_objects()["objects"][card.0.to_string()]["offered_casts"].clone()
    }

    #[test]
    fn r702_37c_morph_cost_evidence_names_its_physical_declaring_row_after_casting_face_down() {
        // 702.37c/708.4: Morph's {3} alternative is declared by the printed
        // keyword, while the spell on the stack has a blank 2/2 definition.
        let def=CardDef::new("Morph witness","{4}{W}{W}","Creature — Angel","Flying\nMorph {2}{W}{W}",Some((4,5))).unwrap();
        let mut game=Game::new(vec![def],2,20);let card=game.put(0,0,Zone::Hand);
        let before=game.audit_casting_costs(card);
        assert_eq!(before["alternatives"].as_array().unwrap().len(),1);
        assert_eq!(before["alternatives"][0]["paragraph"],2);
        game.players[0].pool.colorless=3;
        let pool=game.players[0].pool.clone();
        game.act(0,crate::Move::Paying(pool,Box::new(crate::Move::CastFor(crate::announcement::CastCost::Alternative(1),
            Box::new(crate::Move::Cast{card,targets:vec![]}))))).unwrap();
        let spell=game.stack[0];
        assert!(game.face_down(spell));
        let costs=game.audit_announced_costs(spell);
        assert!(costs.get("unavailable").is_none(),"{costs}");
        assert_eq!(costs["alternative"]["source"],spell.0);
        assert_eq!(costs["alternative"]["paragraph"],2);
        assert_eq!(costs["alternative"]["definition_identity"],json!({"card":0,"face":"Whole"}));
        assert!(costs["alternative"]["text"].as_str().unwrap().starts_with("Morph "));
    }

    #[test]
    fn r608_2g_offered_cast_evidence_distinguishes_free_paid_and_no_offer() {
        // 118.9/118.9b: a free effect supplies its alternative cost; ordinary
        // permission and a zero printed cost do not establish that alternative.
        let mut game=Game::new(vec![CardDef::new("Proof","{0}","Instant","You gain 1 life.",None).unwrap()],2,20);
        let card=game.put(0,0,Zone::Exile);
        assert_eq!(offer(&game,card),json!([]));
        game.permit(0,card,false,Until::EndOfTurn);
        assert_eq!(offer(&game,card),json!([]));
        game.offer(1,card,Costs::Ordinary,None,None);
        assert_eq!(offer(&game,card),json!([{"player":1,"cast_as":"whole","without_paying":false,"during_resolution":true}]));
        game.revoke(1,card);
        game.offer(0,card,Costs::Free,None,None);
        assert_eq!(offer(&game,card),json!([{"player":0,"cast_as":"whole","without_paying":true,"during_resolution":true}]));
        game.revoke(0,card);
        assert_eq!(offer(&game,card),json!([]),"608.2g: declining ends this offer");
    }

    #[test]
    fn r400_7_offer_does_not_follow_a_card_out_and_back_into_exile() {
        let mut game=Game::new(vec![CardDef::new("Proof","{1}","Instant","You gain 1 life.",None).unwrap()],2,20);
        let card=game.put(0,0,Zone::Exile);
        game.offer(0,card,Costs::Free,None,None);
        assert_eq!(offer(&game,card).as_array().unwrap().len(),1);
        game.move_object(card,Zone::Graveyard);
        let graveyard=game.players[0].graveyard[0];
        assert_eq!(offer(&game,card),json!([]));
        assert_eq!(offer(&game,graveyard),json!([]));
        game.move_object(graveyard,Zone::Exile);
        let returned=*game.exile.last().unwrap();
        assert_eq!(game.physical_card_identity(card),game.physical_card_identity(returned));
        assert_eq!(offer(&game,returned),json!([]),"new incarnation needs its own grant");
    }

    #[test]
    fn r701_57a_offer_evidence_uses_the_selected_spell_faces_mana_value() {
        // Discover's bound applies to the resulting spell, not the card's other face.
        let front=CardDef::new("Small","{1}","Sorcery","You gain 1 life.",None).unwrap();
        let back=CardDef::new("Large","{4}","Sorcery","You gain 2 life.",None).unwrap();
        let mut game=Game::new(vec![CardDef::modal(front,back).unwrap()],2,20);
        let card=game.put(0,0,Zone::Exile);
        game.offer(0,card,Costs::Free,None,Some(2));
        assert_eq!(offer(&game,card),json!([
            {"player":0,"cast_as":{"face":0},"without_paying":true,"during_resolution":true},
            {"player":0,"cast_as":"whole","without_paying":true,"during_resolution":true}]));
        game.revoke(0,card);
        game.offer(0,card,Costs::Free,None,Some(5));
        assert_eq!(offer(&game,card),json!([
            {"player":0,"cast_as":{"face":0},"without_paying":true,"during_resolution":true},
            {"player":0,"cast_as":{"face":1},"without_paying":true,"during_resolution":true},
            {"player":0,"cast_as":"whole","without_paying":true,"during_resolution":true}]));
    }

    #[test]
    fn r305_9_modal_land_front_never_authenticates_the_whole_cast_alias() {
        // 305.9/712.11b: the spell back can be cast; the front is a land,
        // including when the wire's omitted cast_as selects Whole.
        let front=CardDef::new("Land front","","Land","",None).unwrap();
        let back=CardDef::new("Spell back","{1}","Sorcery","You gain 1 life.",None).unwrap();
        let mut game=Game::new(vec![CardDef::modal(front,back).unwrap()],2,20);
        let card=game.put(0,0,Zone::Exile);
        game.offer(0,card,Costs::Free,None,None);
        assert_eq!(offer(&game,card),json!([
            {"player":0,"cast_as":{"face":1},"without_paying":true,"during_resolution":true}]));
    }

    #[test]
    fn r118_9a_offer_evidence_obeys_the_cast_cost_selector_and_grant_precedence() {
        use crate::announcement::CastCost;
        let mut game=Game::new(vec![CardDef::new("Proof","{1}","Instant","You gain 1 life.",None).unwrap()],2,20);
        let card=game.put(0,0,Zone::Exile);
        game.offer(0,card,Costs::Only(CastCost::Alternative(0)),None,None);
        assert_eq!(offer(&game,card),json!([]),"an alternative-only offer cannot authenticate the default/free cost selector");
        game.revoke(0,card);
        game.offer(0,card,Costs::Ordinary,None,None);
        game.offer(0,card,Costs::Free,None,None);
        assert_eq!(offer(&game,card),json!([{"player":0,"cast_as":"whole","without_paying":false,"during_resolution":true}]),
            "the same first matching offered grant wins as in casting");
    }
}

#[cfg(test)]
mod turn_bookkeeping_tests {
    use super::*;
    use crate::Move;
    use ir::Part;

    fn ordinary() -> Game {
        let mut game=Game::new(vec![],2,20);
        game.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[]).unwrap();
        game
    }

    #[test]
    fn retained_round_and_budget_have_actual_readback_for_either_holder() {
        // 117.3c/117.4 and 305.2: these facts constrain the next pass/land play.
        for holder in [0,1] {
            let mut game=ordinary();
            game.settle_initial_position(3,Part::PrecombatMain,0,holder,1,&[]).unwrap();
            game.set_initial_land_plays([1,0]).unwrap();
            let actual=game.audit_turn_bookkeeping();
            assert_eq!(actual["consecutive_passes"],1);
            assert_eq!(actual["land_plays_this_turn"],json!({"A":1,"B":0}));
            assert_eq!(game.audit_initial_history().unwrap()["land_plays_this_turn"],json!({"A":1,"B":0}));
            game.act(holder,Move::Pass).unwrap();
            assert_eq!(game.part,Part::BeginningOfCombat);
            assert_eq!(game.audit_turn_bookkeeping()["consecutive_passes"],0);
            assert_eq!(game.audit_turn_bookkeeping()["land_plays_this_turn"],json!({"A":1,"B":0}));
        }
        let mut game=ordinary();
        let before = format!("{game:?}");
        for uses in [[0,1], [2,0]] {
            assert_eq!(game.set_initial_land_plays(uses), Err("unsupported initial land-play consumption".into()));
            assert_eq!(format!("{game:?}"), before);
        }
        assert_eq!(game.audit_turn_bookkeeping()["land_plays_this_turn"],json!({"A":0,"B":0}));
    }

    #[test]
    fn schedule_audit_checks_actual_owners_and_ignores_allocator_ids() {
        let game=ordinary();
        let expected=json!({"kind":"ordinary_two_player","order":["A","B"],"next_turn":"B"});
        assert_eq!(game.audit_turn_bookkeeping()["schedule"],expected);
        for mutation in 0..4 {
            let mut changed=game.clone();
            match mutation {
                0=>{changed.extra_turn(1,None);},
                1=>{changed.control_next_turn(1,0);},
                2=>changed.add_skip(&[0],crate::turn::Occurrence::Part(Part::Draw),false,None,None),
                _=>{changed.add_phases(ir::action::AfterPhase::This,&[ir::turn::Phase::Combat],1);},
            }
            let before = format!("{changed:?}");
            assert_eq!(changed.audit_turn_bookkeeping(), json!({"unavailable":"nonordinary turn schedule, phase suffix, skip or turn control"}));
            assert_eq!(changed.audit_initial_history(), Err("initial causal history no longer representable".into()));
            assert_eq!(format!("{changed:?}"), before);
        }
        let mut allocated=game;
        allocated.state_mut().plan=crate::turn::TurnPlan::at(3,Part::PrecombatMain,900);
        assert_eq!(allocated.audit_turn_bookkeeping()["schedule"],expected);
    }
}

#[cfg(test)]
mod activation_use_tests {
    use super::*;
    use crate::{CardDef,Move,Zone};
    use crate::state::{Answer,Choice,Decision};

    #[test]
    fn basic_mana_counts_aggregate_and_reclaimed_sources_become_unavailable() {
        // 305.6/605.3b: two real unrestricted mana activations remain two uses.
        // 400.7: the departed source must never be relabelled as its successor.
        let def=CardDef::new("Forest","","Basic Land — Forest","",None).unwrap();
        let mut g=Game::new(vec![def],2,20);let forest=g.put(0,0,Zone::Battlefield);
        for _ in 0..2 {
            g.act(0,Move::Activate{source:forest,ability:0,targets:vec![],paying:vec![]}).unwrap();
            g.untap(forest);
        }
        let actual=g.audit_activation_uses();let rows=actual.as_array().unwrap();
        assert_eq!(rows.len(),1);assert_eq!(rows[0]["count"],2);
        assert_eq!(rows[0]["ability"]["source"],forest.0);
        assert_eq!(rows[0]["ability"]["origin"],json!({"kind":"rule"}));
        assert_eq!(rows[0]["ability"]["text"],"{T}: Add {G}.");
        let mut granted=g.clone();
        granted.activated[0].1.origin=Origin::Given{grant:crate::binding::Grant{by:Some(forest),stamp:1,part:0},
            what:match &g.activated[0].1.origin {Origin::Rule(what)=>*what,_=>unreachable!()}};
        assert!(granted.audit_activation_uses().get("unavailable").is_some(),"one unsupported row makes the whole inventory unknown");
        g.move_object(forest,Zone::Graveyard);g.act(0,Move::Pass).unwrap();
        g.retire_events(g.log.end());g.collect_departed();
        assert!(g.get_object(forest).is_none());
        assert!(g.audit_game_objects()["activation_uses"]["unavailable"].as_str().unwrap().contains("no longer retained"));
    }

    #[test]
    fn restored_counts_do_not_shift_suspended_payment_undo_indices() {
        // 602.5b/733.1: reverse the reserved outer activation, retaining a prior
        // declared use and the completed inner mana ability, with no invented actor.
        let defs=vec![
            CardDef::new("Earlier","{0}","Artifact","{0}: Add {C}. Activate only once each turn.",None).unwrap(),
            CardDef::new("Mana","{0}","Creature","{T}: Put a +1/+1 counter on this creature. Add {G}.",Some((1,1))).unwrap(),
            CardDef::new("Scales","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap(),
            CardDef::new("Evolution","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None).unwrap(),
            CardDef::new("Outer","{0}","Artifact","Exhaust — {2}: You gain 3 life.",None).unwrap(),
        ];
        for def in &defs {crate::play::check(def).unwrap();}
        let mut g=Game::new(defs,2,20);let earlier=g.put(0,0,Zone::Battlefield);
        g.settle_initial_position(3,ir::Part::PrecombatMain,0,0,0,&[(earlier,0,false)]).unwrap();
        g.set_initial_activation_uses(&[InitialActivationUse{source:earlier,paragraph:1,lifetime:ActivationUseLifetime::Turn,count:1}]).unwrap();
        assert!(g.activation_started.is_empty());
        let mana=g.put(0,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);
        let evolution=g.put(0,3,Zone::Battlefield);let outer=g.put(0,4,Zone::Battlefield);
        let activate=||Move::Activate{source:outer,ability:0,targets:vec![],paying:vec![]};
        g.act(0,Move::WithMana(vec![crate::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source:mana,ability:0,paying:vec![],adding:vec![],chose:vec![]}],Box::new(activate()))).unwrap();
        let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else {panic!("replacement choice")};
        let effect=crate::operation::ReplacementId::from_ability(evolution, AbilityKey::printed(0), 0);
        assert!(among.contains(&effect));
        assert_eq!(g.audit_activation_uses(), json!({"unavailable":"activation inventory requires a settled priority boundary"}));
        g.act(0,Move::Choose(Answer::Replacement(effect))).unwrap();
        assert!(matches!(g.decision,Some(Decision::Payment{..})),"the outer cost remains unpaid after the committed inner ability");
        assert_eq!(g.activated.len(),3);assert_eq!(g.activation_started.len(),2);
        g.act(0,Move::ReversePayment).unwrap();
        assert_eq!(g.activated,vec![(earlier,AbilityKey::printed(0)),(mana,AbilityKey::printed(0))]);
        assert_eq!(g.activation_started,vec![(0,None,true)],"only the real completed mana activation has an actor/event record");
        assert!(g.obj(outer).activation_uses.is_empty());assert_eq!(g.players[0].pool.total(),1);
        assert_eq!(g.audit_activation_uses().as_array().unwrap().len(),2);
        let before = format!("{g:?}");
        assert_eq!(g.audit_initial_history(), Err("initial causal history no longer representable".into()));
        assert_eq!(format!("{g:?}"), before);
        assert!(g.obj(mana).tapped);
        assert_eq!(g.counters(mana).get("+1/+1"), Some(&3));
        g.players[0].pool.colorless=1;
        g.act(0,Move::Paying(crate::state::Pool { colors: [0,0,0,0,1], colorless: 1, ..Default::default() }, Box::new(activate()))).unwrap();
        assert_eq!(g.obj(outer).activation_uses,vec![AbilityKey::printed(0)]);
    }
}

#[cfg(test)]
mod occurrence_origin_tests {
    use super::*;
    use crate::{CardDef, Zone};

    #[test]
    fn r707_10_copy_audit_keeps_the_exact_donor_after_event_history_clears() {
        let definition=CardDef::new("Same name","{0}","Instant","You gain 1 life.",None).unwrap();
        let mut game=Game::new(vec![definition],2,20);
        let original=game.put(0,0,Zone::Stack);
        let other=game.put(0,0,Zone::Stack);
        let copies=crate::copy::StackCopies::new(vec![original],1,0,false).advance(&mut game).unwrap();
        let copy=copies[0];
        game.move_object(original,Zone::Graveyard);
        game.log.clear();
        game.collect_departed();
        assert!(game.get_object(original).is_some(),"the copy retains its exact departed donor");
        let row=game.audit_game_objects()["objects"][copy.0.to_string()].clone();
        assert_eq!(row["stack_copy_of"],original.0);
        assert_ne!(row["stack_copy_of"],other.0);
        assert_eq!(row["spell_copy_definition"], json!({"card":0,"name":"Same name","face":"Whole"}));
        assert!(game.audit_game_objects()["objects"][other.0.to_string()]["stack_copy_of"].is_null());
    }

    #[test]
    fn r707_2b_spell_copy_definition_keeps_the_frozen_alternative_face() {
        use crate::card::CastAs;
        let front=CardDef::new("Donor", "{4}{G}", "Creature — Giant", "", Some((5,5))).unwrap();
        let spell=CardDef::new("Journey", "{0}", "Sorcery — Adventure", "You gain 1 life.", None).unwrap();
        let mut game=Game::new(vec![CardDef::adventurer(front,spell).unwrap()],2,20);
        // A focused audit premise: the donor is its Adventure spell, not a
        // complete casting scenario. 707.2b freezes this definition at copying.
        let original=game.put(0,0,Zone::Stack);
        let chars=game.defs[0].adventure().unwrap().chars();
        game.obj_mut(original).chars=chars;
        game.obj_mut(original).cast_as=CastAs::Adventure;
        let copies=crate::copy::StackCopies::new(vec![original],1,0,false).advance(&mut game).unwrap();
        let copy=copies[0];
        game.move_object(original,Zone::Hand);
        game.log.clear();
        assert_eq!(game.audit_spell_copy_definition(copy),json!({"card":0,"name":"Donor","face":"Adventure"}));
    }

    #[test]
    fn r702_185a_warp_occurrence_names_the_paid_declaring_row_after_source_departure() {
        // 702.185a and 608.3g: paying the second Warp instance creates one
        // end-step ability from that spell, not from its resulting permanent.
        let make=|name|CardDef::new(name,"{0}","Creature — Beast",
            "Flying\nWarp {0}\nWarp {0}",Some((2,2))).unwrap();
        let mut game=Game::new(vec![make("Warp witness"),make("Different definition")],2,20);
        let card=game.put(0,0,Zone::Hand);
        let cast=crate::Move::Cast{card,targets:vec![]};
        game.act(0,crate::Move::CastFor(crate::announcement::CastCost::Alternative(2),Box::new(cast))).unwrap();
        game.act(0,crate::Move::Pass).unwrap();
        game.act(1,crate::Move::Pass).unwrap();
        assert_eq!(game.delayed.len(),1);
        let source=game.delayed[0].source;
        assert_eq!(game.obj(source).zone,Zone::Gone,"the spell became a new permanent (400.7)");
        let captured=game.delayed[0].captured.clone();
        let stack=game.put(0,0,Zone::Stack);
        game.obj_mut(stack).card=None;
        game.obj_mut(stack).ability=Some((Some(source),2));
        game.obj_mut(stack).invocations=Some(captured.select(&[]));
        let occurrence=|game:&Game|game.audit_game_objects()["objects"][stack.0.to_string()]["occurrences"][0].clone();
        let row=occurrence(&game);
        assert!(row.get("unavailable").is_none(),"{row}");
        assert_eq!(row["source"],source.0);
        assert_eq!(row["declaration_index"],2);
        assert_eq!(row["paragraph"],3);
        assert_eq!(row["keyword"],"Warp");
        assert_eq!(row["rule"],"702.185a");
        assert!(row["text"].as_str().unwrap().contains("next end step"));
        // 113.7a / 603.7e: the spell's delayed ability survives a change to
        // the resulting permanent, without adopting its different definition.
        let permanent=game.battlefield[0];
        game.obj_mut(permanent).card=Some(1);
        game.move_object(permanent,Zone::Graveyard);
        assert_eq!(occurrence(&game),row);

        let original=game.obj(stack).invocations.clone();
        fn edit(game:&mut Game,stack:ObjId)->&mut Vec<crate::program::Invocation> { game.obj_mut(stack).invocations.as_mut().unwrap().as_mut().unwrap() }
        edit(&mut game,stack)[0].keyword_producer=None;
        assert!(occurrence(&game).get("unavailable").is_some(),"body identity alone is insufficient");
        game.obj_mut(stack).invocations=original.clone();
        let producer=edit(&mut game,stack)[0].keyword_producer.as_ref().unwrap().clone();
        edit(&mut game,stack)[0].keyword_producer=Some(std::sync::Arc::new((producer.0.clone(),AbilityKey::printed(0))));
        assert!(occurrence(&game).get("unavailable").is_some(),"a non-Warp row cannot declare Warp's rule body");
        game.obj_mut(stack).invocations=original.clone();
        edit(&mut game,stack)[0].keyword_producer=Some(std::sync::Arc::new((producer.0.clone(),AbilityKey::printed(1))));
        assert!(occurrence(&game).get("unavailable").is_some(),"another identical Warp instance was not the paid producer");
        game.obj_mut(stack).invocations=original.clone();
        let rebound=crate::program::rebound_cast(&game,crate::select::Ctx::for_spell(0,source),permanent)
            .select(&[]).unwrap()[0].program.clone();
        edit(&mut game,stack)[0].program=rebound;
        assert!(occurrence(&game).get("unavailable").is_some(),"a Warp producer cannot authenticate Rebound's body");
        game.obj_mut(stack).invocations=original;
        // A same-text definition at the retained source is not the producer.
        game.obj_mut(source).card=Some(1);
        assert!(occurrence(&game).get("unavailable").is_some());
    }

    #[test]
    fn r702_88a_rule_created_occurrence_keeps_its_exact_keyword_producer() {
        let make=|name|CardDef::new(name,"{0}","Instant","You gain 1 life.\nRebound",None).unwrap();
        let mut game=Game::new(vec![make("Rebound witness"),make("Different definition")],2,20);
        let source=game.put(0,0,Zone::Stack);
        let card=game.put(0,0,Zone::Exile);
        game.rebound_exiled(source,card);
        let captured=game.delayed[0].captured.clone();
        let stack=game.put(0,0,Zone::Stack);
        game.obj_mut(stack).card=None;
        game.obj_mut(stack).ability=Some((Some(source),0));
        game.obj_mut(stack).invocations=Some(captured.select(&[]));
        let occurrence=|game:&Game|game.audit_game_objects()["objects"][stack.0.to_string()]["occurrences"][0].clone();
        let row=occurrence(&game);
        assert!(row.get("unavailable").is_none(),"{row}");
        assert_eq!(row["source"],source.0);
        assert_eq!(row["paragraph"],2);
        assert_eq!(row["keyword"],"Rebound");
        assert_eq!(row["rule"],"702.88a");
        // The global rule body cannot authenticate another same-text definition.
        game.obj_mut(source).card=Some(1);
        assert!(occurrence(&game).get("unavailable").is_some());
    }
}

#[cfg(test)]
mod inherited_definition_tests {
    use crate::{CardDef, Game, Move, Zone};

    #[test]
    fn r113_10c_audit_names_the_original_declaration_and_frozen_donor_definition() {
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog=CardDef::catalog(&faces);
        let marvin=CardDef::from_faces(&catalog["Marvin, Murderous Mimic"]).unwrap();
        // Both donors intentionally have identical names and text: neither is identity.
        let donor=||CardDef::new("Same donor","{0}","Creature — Human",
            "{0}: You gain 1 life.\n{0}: You gain 2 life.",Some((1,1))).unwrap();
        let mut game=Game::new(vec![marvin,donor(),donor()],2,20);
        let receiver=game.put(0,0,Zone::Battlefield);
        let first=game.put(0,1,Zone::Battlefield);
        let second=game.put(0,2,Zone::Battlefield);
        let audit=game.audit_game_objects();
        let rows=audit["objects"][receiver.0.to_string()]["abilities"].as_array().unwrap();
        let acquired=rows.iter().find(|row|row["origin"]["family"]["donor"]==second.0
            && row["declaration_index"]==0).unwrap();
        assert_ne!(acquired["index"],acquired["declaration_index"],"presentation ordinal is not the donor's declaration");
        assert_eq!(acquired["paragraph"],1);
        let family=&acquired["origin"]["family"];
        let donor_row=&audit["objects"][second.0.to_string()]["abilities"][0];
        assert_eq!(donor_row["declaration_index"],0);
        assert_eq!(donor_row["definition_identity"],family["definition"]);
        assert_ne!(audit["objects"][first.0.to_string()]["abilities"][0]["definition_identity"],family["definition"],
            "same text from a different frozen definition cannot authenticate the donor");
        assert_eq!(audit["objects"][receiver.0.to_string()]["abilities"][0]["definition_identity"],family["grant"]["definition"]);
        assert_eq!(audit["objects"][receiver.0.to_string()]["abilities"][0]["declaration_index"],family["grant"]["ability"]);

        // CR 113.7a: the activated occurrence retains its acquired family after
        // the live donor changes to another identically worded definition.
        let ability=acquired["index"].as_u64().unwrap() as usize;
        game.act(0,Move::Activate{source:receiver,ability,targets:vec![],paying:vec![]}).unwrap();
        game.obj_mut(second).card=Some(1);
        let audit=game.audit_game_objects();
        let occurrence=&audit["objects"][game.stack[0].0.to_string()]["occurrences"][0];
        assert!(occurrence.get("unavailable").is_none(),"{occurrence}");
        assert_eq!(occurrence["declaration_index"],0);
        assert_eq!(occurrence["definition_identity"],family["definition"]);
        assert_ne!(audit["objects"][second.0.to_string()]["abilities"][0]["definition_identity"],occurrence["definition_identity"],
            "changed live donor evidence must mismatch the retained occurrence");
    }
}

#[cfg(test)]
mod restricted_mana_provenance_tests {
    use super::*;
    use crate::{CardDef, Zone};

    #[test]
    fn frozen_mana_receipt_projects_original_ability_after_departure_and_ignores_cache_warmth() {
        let definition = CardDef::new("Mana witness", "", "Land",
            "{T}: Add {C}{C}. Spend this mana only to cast a creature spell.", None).unwrap();
        let mut game = Game::new(vec![definition], 2, 20);
        let source = game.put(0, 0, Zone::Battlefield);
        let key = AbilityKey::printed(0);
        game.act(0, crate::Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        let proof = game.players[0].pool.restricted[0].provenance.clone().expect("authenticated production");
        let cold = proof.as_ref().clone();
        assert!(proof.projection.get().is_none(), "production must not materialize audit JSON");
        let expected = json!({"physical_definition":0,"ability":game.audit_ability(Some(source), &key),
            "restriction":game.ability_text(Some(source), key.clone()),"chosen":{}});
        game.move_object(source, Zone::Graveyard);
        game.obj_mut(source).chars.name = "Changed after production".to_owned();
        let proof_debug = format!("{proof:?}");
        let game_debug = format!("{game:?}");
        assert_eq!(proof.audit(), Some(&expected));
        assert_eq!(format!("{proof:?}"), proof_debug, "audit cache warmth cannot change receipt debug identity");
        assert_eq!(format!("{game:?}"), game_debug, "requesting evidence cannot change game debug identity");
        assert!(cold.projection.get().is_none());
        assert_eq!(proof.as_ref(), &cold, "audit cache warmth cannot change pool equality");
        let mut renamed = cold.clone();
        renamed.name = "Different name without a self-reference".to_owned();
        assert_eq!(cold, renamed, "names absent from the wire wording do not change receipt equality");
        renamed.source = ObjId(source.0 + 1);
        assert_ne!(cold, renamed, "a producing incarnation remains part of the evidence");
    }
}
