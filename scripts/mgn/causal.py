"""Portable causal premises and the bounded native fixed-result constructor.

Historical labels identify objects, not physical-card incarnations guessed from a
counter. A `current` alias is bound once at setup and never follows a zone change.
"""
from copy import deepcopy

from .state import Problem


def check_causal(record, state, oracle, problems):
    causal = record["start"].get("causal")
    if causal is None:
        return
    path = "/start/causal"
    objects = causal["objects"]
    known = set(state.objects) | set(state.historical) | set(state.departed) | set(record["cards"]) | set(objects)
    def error(code, at, message):
        problems.append(Problem("error", code, path + at, message))
    def reference(ref, at):
        if ref not in known:
            error("unknown_reference", at, f"unknown causal object {ref}")
    def seat(player, at):
        if player not in state.seats:
            error("unknown_reference", at, f"unknown player {player}")
    for label, obj in objects.items():
        if obj["zone"] not in state.zones:
            error("spelling", f"/objects/{label}/zone", "not a rules zone of this game")
        if label in record["cards"] or state.exists(label) or label in state.departed:
            error("duplicate_id", f"/objects/{label}", "a historical label must differ from live object and physical card ids")
        if "card" in obj and obj["card"] not in record["cards"]:
            error("unknown_reference", f"/objects/{label}/card", "unknown physical card")
        if "owner" in obj:
            seat(obj["owner"], f"/objects/{label}/owner")
        if "current" in obj:
            current = obj["current"]
            if not state.exists(current) or ("card" in obj and obj["card"] != current) or ("owner" in obj and obj["owner"] != state.owner_of(current)):
                error("coherence", f"/objects/{label}/current", "current must alias this object's live initial identity and owner")
            zone = state.zone_of(current)
            if obj["zone"] != zone:
                error("coherence", f"/objects/{label}/zone", "current alias has a different initial zone")
        if "oracle" in obj["definition"] and oracle is not None and obj["definition"]["oracle"] not in oracle:
            error("unknown_reference", f"/objects/{label}/definition", "historical Oracle definition is absent from the pinned corpus")
    producers = {}
    producer_text = {}
    for index, producer in enumerate(causal["producers"]):
        at = f"/producers/{index}"
        if producer["id"] in producers or producer["id"] in known:
            error("duplicate_id", at + "/id", "producer occurrence id is not unique")
        producers[producer["id"]] = producer
        reference(producer["source"], at + "/source")
        from .abilities import ability_object_refs
        for declaring in ability_object_refs(producer["ability"]):
            reference(declaring, at+"/ability")
        seat(producer["controller"], at + "/controller")
        if oracle is not None:
            ability = producer["ability"]
            source = objects.get(producer["source"], {})
            facts = state.reference_facts(producer["source"])
            definition = source.get("definition") or facts.get("copy_definition") or facts.get("token") or {"oracle": record["cards"].get(producer["source"], {}).get("oracle"), "face":ability.get("face",0)}
            from .copy_definition import basis, characteristics
            frozen = (characteristics(state.copy_bases[producer["source"]], oracle)
                if not source and not facts.get("copy_definition") and producer["source"] in state.copy_bases
                else characteristics(basis(definition), oracle))
            text = None
            if "paragraph" in ability:
                from .oracle import paragraphs
                lines = paragraphs((frozen or {}).get("text", ""))
                paragraph = ability["paragraph"]
                if 1 <= paragraph <= len(lines):
                    text = lines[paragraph - 1]
                else:
                    error("ability", at + "/ability", "the frozen source has no such Oracle paragraph")
            elif "rule" not in ability and "inherited_from" not in ability:
                error("ability", at + "/ability", "the producer needs a rule or frozen ability reference")
            producer_text[producer["id"]] = text
    ids = set(producers) | known
    def bindings(values, at):
        seen = set()
        for index, binding in enumerate(values):
            here = f"{at}/{index}"
            identity = (binding["kind"], binding.get("group"), binding.get("choice"))
            if identity in seen:
                error("coherence", here, "duplicate retained semantic role")
            seen.add(identity)
            object_roles = {"targets", "exiled", "created", "destroyed", "discarded", "event_subject"}
            for value in binding["values"]:
                if "object" in value:
                    reference(value["object"], here + "/values")
                if "player" in value:
                    seat(value["player"], here + "/values")
                if binding["kind"] in object_roles and not ("object" in value or binding["kind"] in ("targets", "event_subject") and "player" in value):
                    error("coherence", here, "this retained role requires object/player identities")
            if binding["kind"] in ("x", "event_amount") and (len(binding["values"]) != 1 or "number" not in binding["values"][0] or binding["values"][0]["number"] < 0):
                error("coherence", here, "a retained amount is one nonnegative integer")
            if binding["kind"] == "chosen" and not binding.get("choice"):
                error("coherence", here, "a retained choice names its semantic kind")
            if binding["kind"] == "paid" and (len(binding["values"]) != 1 or "boolean" not in binding["values"][0]):
                error("coherence", here, "a retained payment outcome is one boolean")
    for index, producer in enumerate(causal["producers"]):
        bindings(producer.get("retained", []), f"/producers/{index}/retained")
    cast_counts = {player: 0 for player in state.seats}
    cast_sources = set()
    for index, cast in enumerate(causal.get("casts_this_turn", [])):
        at=f"/casts_this_turn/{index}"
        reference(cast["source"], at+"/source")
        seat(cast["player"], at+"/player")
        if cast["source"] in cast_sources:
            error("coherence", at, "one spell incarnation can be cast only once")
        cast_sources.add(cast["source"])
        if cast["player"] in cast_counts:
            cast_counts[cast["player"]] += 1
    assumptions=record["start"].get("assumptions", {}).get("spells_cast", {})
    if any(cast_counts[player] != assumptions.get(player, 0) for player in state.seats):
        error("coherence", "/casts_this_turn", "the complete current-turn cast inventory disagrees with assumptions.spells_cast")
    for family in ("continuous_effects", "delayed_triggers"):
        for index, effect in enumerate(causal[family]):
            at = f"/{family}/{index}"
            if effect["id"] in ids:
                error("duplicate_id", at + "/id", "causal occurrence id is not unique")
            ids.add(effect["id"])
            if effect["producer"] not in producers:
                error("unknown_reference", at + "/producer", "unknown producer occurrence")
            duration = effect["duration"]
            if duration["kind"] == "end_of_turn" and duration["turn"] != state.turn["number"]:
                error("coherence", at + "/duration", "an outstanding until-end-of-turn effect belongs to the current turn")
            if "player" in duration:
                seat(duration["player"], at + "/duration/player")
            if "source" in duration:
                reference(duration["source"], at + "/duration/source")
            for subject in effect.get("subjects", []):
                if "object" in subject:
                    reference(subject["object"], at + "/subjects")
                if "player" in subject:
                    seat(subject["player"], at + "/subjects")
            if family == "delayed_triggers":
                bindings(effect["retained"], at + "/retained")
                when = effect["when"]
                if when.get("whose") not in (None, "any"):
                    seat(when["whose"], at + "/when/whose")
            # Generic rule-text clauses are quotations from the frozen producing
            # ability, with all named objects supplied by typed bindings. They
            # cannot substitute arbitrary executable text for that ability.
            quoted = [duration] if duration["kind"] == "oracle" else []
            quoted += [change for change in effect.get("changes", []) if change["kind"] == "oracle"]
            if effect.get("when", {}).get("kind") == "event":
                quoted.append(effect["when"])
            for quotation in quoted:
                text = producer_text.get(effect["producer"])
                if oracle is not None and (text is None or quotation["text"] not in text):
                    error("coherence", at, "a rule-text cause must quote its frozen producing ability exactly")
            for subject in effect.get("subjects", []):
                if "scope" in subject:
                    text=producer_text.get(effect["producer"])
                    if oracle is not None and (text is None or subject["scope"] not in text):
                        error("coherence", at+"/subjects", "rule-text applicability must quote its frozen producing ability exactly")
    for object_id, player in causal.get("base_controllers", {}).items():
        reference(object_id, "/base_controllers")
        seat(player, "/base_controllers")
    for index, stamp in enumerate(causal.get("timestamp_order", [])):
        at = f"/timestamp_order/{index}"
        if "object" in stamp:
            reference(stamp["object"], at)
        elif "on" in stamp:
            reference(stamp["on"], at)
        elif stamp["effect"] not in {effect["id"] for effect in causal["continuous_effects"]}:
            error("unknown_reference", at, "timestamp does not name a declared continuous effect")


def native_causal(adapter, start, native):
    """Translate semantic causes; refuse facts outside the native import profile."""
    from .adapters.karn import unsupported
    causal = start.get("causal")
    if causal is None:
        return
    def refuse(message, path=""):
        raise unsupported("unsupported_start", message, "start.causal" + path)
    sources = {}
    aliases = {}
    for label, obj in causal["objects"].items():
        definition = obj["definition"]
        card = obj.get("card")
        if (card not in adapter.cards or "oracle" not in definition or definition.get("face", 0) != 0
                or definition.get("fused") or definition["oracle"] != adapter.cards[card]["oracle"] or obj.get("lki")):
            refuse("native historical import requires a plain printed card definition without additional LKI", f".objects.{label}")
        if "current" in obj:
            aliases[label] = obj["current"]
        else:
            if obj["zone"] != "stack":
                refuse("native departed producer import currently supports resolved spell sources", f".objects.{label}.zone")
            sources[label] = {"physical": card, "name": adapter._face_name(card)}
    def ref(label):
        return aliases.get(label, label)
    producers = {}
    for producer in causal["producers"]:
        ability = producer["ability"]
        if set(ability) - {"face", "paragraph"} or ability.get("face", 0) != 0 or producer.get("retained"):
            refuse("native causal producer requires a plain printed paragraph without other retained choices")
        source = ref(producer["source"])
        descriptor = causal["objects"].get(producer["source"])
        card = descriptor.get("card") if descriptor else producer["source"]
        if card not in adapter.cards:
            refuse("native causal producer has no immutable printed physical definition")
        producers[producer["id"]] = {"source": source,
            "definition": {"oracle": adapter.cards[card]["oracle"]}, "paragraph": ability["paragraph"], "origin": {"kind": "printed"}}
    def change(value):
        kind = value["kind"]
        if kind == "control":
            return {"controller": adapter._player(value["player"])}
        if kind in ("add_pt", "set_pt"):
            return {"add_pt" if kind == "add_pt" else "base_pt": {"power": value["power"], "toughness": value["toughness"]}}
        if kind == "grant":
            return {"grant": value["text"].lower()}
        refuse("continuous change has no native initial constructor")
    effects = []
    bases = {}
    declared_bases = {ref(key): player for key, player in causal.get("base_controllers", {}).items()}
    for effect in causal["continuous_effects"]:
        subjects = []
        for subject in effect["subjects"]:
            if set(subject) != {"object"}:
                refuse("native continuous import currently requires fixed object subjects")
            subject = ref(subject["object"])
            if subject not in native["objects"]:
                refuse("native continuous subject must be a current initial permanent")
            subjects.append(subject)
            bases[subject] = declared_bases.get(subject, native["objects"][subject]["controller"])
            if any(c["kind"] == "control" for c in effect["changes"]) and subject not in declared_bases:
                refuse("a resolved control effect requires its explicit initial base controller")
        producer = next(p for p in causal["producers"] if p["id"] == effect["producer"])
        effects.append({"id": effect["id"], "producer": producers[effect["producer"]], "controller": producer["controller"],
            "subjects": subjects, "changes": [change(c) for c in effect["changes"]], "expires": deepcopy(effect["duration"])})
    delays = []
    for delayed in causal["delayed_triggers"]:
        retained = delayed["retained"]
        if len(retained) != 1 or retained[0]["kind"] != "exiled" or any(set(v) != {"object"} for v in retained[0]["values"]):
            refuse("native delayed import currently requires one retained exile result")
        producer = next(p for p in causal["producers"] if p["id"] == delayed["producer"])
        delays.append({"id": delayed["id"], "producer": producers[delayed["producer"]], "controller": producer["controller"],
            "clause": delayed["clause"], "retained": [{"kind": "exiled", "objects": [ref(v["object"]) for v in retained[0]["values"]]}],
            "when": deepcopy(delayed["when"]), "once": delayed["once"], "expires": deepcopy(delayed["duration"])})
    if causal.get("timestamp_order"):
        # The native constructor currently creates all permanents before resolved
        # effects. Preserve this fact, and refuse rather than reorder an implicit static.
        order = causal["timestamp_order"]
        expected = [{"object": member} for member in native["zones"]["battlefield"]] + [{"effect": e["id"]} for e in causal["continuous_effects"]]
        if order != expected:
            refuse("native initial timestamp constructor requires battlefield order followed by resolved-effect order", ".timestamp_order")
    native["causal"].update(source_objects=sources, continuous_effects=effects, base_controllers=bases, delayed_triggers=delays)
    native["history"]["pending"] = "represented_delayed" if delays else "none"
    native["history"]["effects"] = "represented_only" if effects else "intrinsic_only"
    for cast in causal.get("casts_this_turn", []):
        if cast.get("announcement"):
            refuse("native current-turn history import does not yet restore past announcement choices", ".casts_this_turn")
    casts=[{"source":ref(cast["source"]),"player":cast["player"]} for cast in causal.get("casts_this_turn", [])]
    counts={player:sum(cast["player"]==player for cast in casts) for player in adapter.seats}
    if any(counts[player]!=start.get("assumptions",{}).get("spells_cast",{}).get(player,0) for player in adapter.seats):
        refuse("current-turn cast inventory disagrees with assumptions.spells_cast", ".casts_this_turn")
    native["causal"]["casts_this_turn"]=casts
    native["history"]["turn_events"]="represented_casts_only" if casts else "empty"


def bind_and_verify(adapter, start):
    """Read actual causes back before the first action; compare more than P/T."""
    from .adapters.karn import fault
    causal = start.get("causal")
    if causal is None:
        return
    native = adapter._native_position(start, adapter.setup_request["game"])
    refs = {engine: label for label, engine in adapter.current.items()}
    handles = dict(adapter.current)
    allocated = set()
    for label, descriptor in sorted(causal["objects"].items()):
        if "current" in descriptor:
            handles[label] = adapter.current[descriptor["current"]]
            continue
        card = descriptor["card"]
        candidates = sorted(int(key) for key, item in adapter.audit["objects"].items()
            if item["zone"] == "Gone" and item.get("historical_physical") == adapter.physical[card]
            and item["definition"] == adapter._face_name(card) and int(key) not in allocated)
        if not candidates:
            raise fault("engine_fault", f"historical causal source {label} was not retained")
        engine = candidates[0]
        allocated.add(engine)
        adapter.causal_labels[engine] = label
        refs[engine] = label
        handles[label] = engine
    adapter.causal_refs = refs
    adapter.causal_handles = handles
    adapter.initial_causal = deepcopy(causal)
    observed = readback(adapter)
    # Omitted, duplicated, reordered, rebound or wrong-duration causes are
    # differences even when their immediate visible characteristics coincide.
    for family in ("continuous_effects", "delayed_triggers", "timestamp_order", "base_controllers", "casts_this_turn"):
        if family not in causal:
            continue
        if observed[family] != causal[family]:
            raise fault("engine_fault", f"initial causal readback differs in {family}: {observed[family]!r}")
    adapter.imported_continuous = {row["identity"] for row in adapter.audit.get("continuous_effects", [])}
    adapter.imported_delayed = {(row["producer"]["source"], row["producer"]["paragraph"], row["clause"]) for row in adapter.audit.get("delayed_triggers", [])}
    adapter.causal_verified = True
    return observed


def readback(adapter):
    from .adapters.karn import fault
    causal = adapter.initial_causal
    if causal is None:
        return None
    def ref(engine):
        return adapter.causal_refs[engine] if engine in adapter.causal_refs else adapter._record_id(engine)
    def retained_ref(engine, wanted):
        # Historical/current aliases are authored names for one setup identity,
        # not invitations to follow its physical card after a zone change.
        return wanted if adapter.causal_handles.get(wanted)==engine else ref(engine)
    producers = {p["id"]: p for p in causal["producers"]}
    def producer_id(row):
        source = ref(row["producer"]["source"])
        controller = adapter.seats[row["controller"]]
        matches = [p["id"] for p in producers.values() if p["source"] == source and p["controller"] == controller
            and p["ability"]["paragraph"] == row["producer"]["paragraph"]]
        if len(matches) != 1:
            raise fault("engine_fault", "causal readback has no unique authored producer occurrence")
        return matches[0]
    out = {"objects": deepcopy(causal["objects"]), "producers": deepcopy(causal["producers"]),
           "continuous_effects": [], "delayed_triggers": []}
    if "casts_this_turn" in causal:
        declared={row["source"] for row in causal["casts_this_turn"]}
        casts=[]
        for row in adapter.audit.get("casts_this_turn", []):
            source=ref(row["source"])
            if source in declared:
                casts.append({"source":source,"player":adapter.seats[row["player"]]})
            elif adapter.causal_verified:
                adapter.unsupported_paths.append("causal.new_occurrences")
            else:
                raise fault("engine_fault","undeclared initial cast occurrence")
        out["casts_this_turn"]=casts
    for family in ("continuous_effects", "delayed_triggers"):
        unused = list(causal[family])
        for row in adapter.audit.get(family, []):
            if adapter.causal_verified:
                imported = (row.get("identity") in adapter.imported_continuous if family == "continuous_effects" else
                    (row.get("producer", {}).get("source"), row.get("producer", {}).get("paragraph"), row.get("clause")) in adapter.imported_delayed)
                if not imported:
                    adapter.unsupported_paths.append("causal.new_occurrences")
                    continue
            if "unavailable" in row:
                raise fault("engine_fault", f"causal readback unavailable: {row['unavailable']}")
            pid = producer_id(row)
            matches = [effect for effect in unused if effect["producer"] == pid and (family != "delayed_triggers" or effect["clause"] == row["clause"])]
            if not matches:
                raise fault("engine_fault", "native causal inventory has an undeclared occurrence")
            declared = matches[0]
            unused.remove(declared)
            effect = {"id": declared["id"], "producer": pid, "duration": deepcopy(row["expires"])}
            if family == "continuous_effects":
                changes = []
                for change in row["changes"]:
                    if "add_pt" in change or "base_pt" in change:
                        field = "add_pt" if "add_pt" in change else "base_pt"
                        changes.append({"kind": "add_pt" if field == "add_pt" else "set_pt", **change[field]})
                    elif "controller" in change:
                        changes.append({"kind": "control", "player": adapter.seats[change["controller"]]})
                    elif "grant" in change:
                        changes.append({"kind": "grant", "text": change["grant"]})
                    else:
                        raise fault("engine_fault", "native continuous change unavailable")
                wanted=declared["subjects"]
                effect.update(subjects=[{"object": retained_ref(engine,wanted[index].get("object")) if index<len(wanted) else ref(engine)} for index,engine in enumerate(row["subjects"])], changes=changes)
            else:
                bindings=[]
                for index,binding in enumerate(row["retained"]):
                    wanted=declared["retained"][index]["values"] if index<len(declared["retained"]) else []
                    bindings.append({"kind":binding["kind"],"values":[{"object":retained_ref(engine,wanted[i].get("object")) if i<len(wanted) else ref(engine)} for i,engine in enumerate(binding["objects"])]})
                effect.update(clause=row["clause"], once=row["once"], when=deepcopy(row["when"]),retained=bindings)
            out[family].append(effect)
    for field in ("timestamp_order", "base_controllers"):
        if field in causal:
            if field == "timestamp_order":
                actual=[]
                for entry in causal[field]:
                    if "object" in entry:
                        wanted=entry["object"]
                        engine=adapter.current.get(wanted) or next((eid for eid,label in adapter.causal_refs.items() if label==wanted),None)
                        if engine is None or str(engine) not in adapter.audit["objects"]:
                            continue
                        timestamp=adapter.audit["objects"][str(engine)]["timestamp"]
                    elif "effect" in entry:
                        effect=next((row for row in out["continuous_effects"] if row["id"]==entry["effect"]),None)
                        if effect is None:
                            continue
                        index=out["continuous_effects"].index(effect)
                        timestamp=adapter.audit["continuous_effects"][index]["identity"]
                    else:
                        raise fault("engine_fault","counter timestamp readback is outside the native profile")
                    actual.append((timestamp,entry))
                out[field]=[deepcopy(entry) for _,entry in sorted(actual,key=lambda row:row[0])]
            else:
                actual={}
                for key in causal[field]:
                    engine=next((eid for eid,label in adapter.causal_refs.items() if label==key),adapter.current.get(key))
                    controller=adapter.audit["base_controllers"].get(str(engine))
                    if controller is not None:
                        actual[key]=adapter.seats[controller]
                out[field]=actual
    return out
