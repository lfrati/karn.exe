"""Translate captured native moves, without running or mutating an adapter.

MGN 3.3 and 5 define the output, including explicit payment (5.8). The caller
binds the adapter to BEFORE and supplies AFTER; implied/null answers are returned
explicitly for the recorder to fold. This is evidence translation, not an autopayer.
"""
from __future__ import annotations

from copy import deepcopy, copy
import re

from .adapters.karn import KarnAdapter, unsupported
from .keywords import keyword_name
from .payments import PaymentEvidenceError, InvalidPayment, prior_spending, restore_payment, invert_payment_quote, payment_suspension, continuation_payment, continuation_selector, select_payment, validate_production, validate_bound_payment, _validate_quote_frames
from .vocabulary import canonical_subtype

MANA = frozenset("WUBRGC")
PAYMENT_FIELDS = {"plan", "mana", "paying", "life"}
ANNOUNCEMENT_FIELDS = PAYMENT_FIELDS | {"targets", "x", "modes", "division", "chose", "cost_branches"}
CAST_FIELDS = ANNOUNCEMENT_FIELDS | {"card", "cast_as", "cost", "route", "convoke", "delve", "improvise", "kicks", "replicate", "entwine", "buyback", "bargain", "additional", "offspring", "casualty", "teamwork", "gift", "slots", "pay_life", "face_down", "alternatives"}
FIELDS = {
    "pass": set(), "concede": set(), "keep": set(), "mulligan": set(),
    "play": {"card", "face"}, "cast": CAST_FIELDS,
    "activate": ANNOUNCEMENT_FIELDS | {"source", "ability", "adding", "slots", "asks", "alternatives"},
    "discard": {"cards"}, "bottom": {"cards"}, "attack": {"attacks"}, "block": {"blocks"},
    "divide": {"division"}, "modes": {"modes"}, "targets": {"targets", "modes", "division"}, "order": {"order"}, "choose": {"answer", "pending_payment"},
    "plot": PAYMENT_FIELDS | {"card"}, "foretell": PAYMENT_FIELDS | {"card"},
    "suspend": PAYMENT_FIELDS | {"card"},
    "unlock": PAYMENT_FIELDS | {"room", "half"}, "turn_face_up": PAYMENT_FIELDS | {"permanent", "way"},
    "companion": PAYMENT_FIELDS,
}


def _refuse(message):
    raise unsupported("unsupported_fact", message, "do")


def _quoted_order_matches(boundary, state):
    # The host assigns a transport request ID after executing the quoted action.
    # Every rule-owned field, including owner, cards and placement, remains exact.
    asked = {key: value for key, value in (state.get("asked") or {}).items() if key != "request_id"}
    return asked == boundary["asked"]


def _target(adapter, value):
    """Old harness captures permit player labels and bare object IDs as targets."""
    if type(value) is int:
        value = {"object": value}
    elif isinstance(value, str) and re.fullmatch(r"p[0-9]+", value):
        value = {"player": int(value[1:])}
    elif isinstance(value, dict) and isinstance(value.get("player"), str):
        player = value["player"]
        if not re.fullmatch(r"p[0-9]+", player):
            _refuse("native target has an unknown player label")
        value = {"player": int(player[1:])}
    if not isinstance(value, dict) or set(value) not in ({"object"}, {"player"}):
        _refuse("native target has no supported object or player form")
    return adapter._target_record(value)


def _row(adapter, source, index):
    rows = adapter.audit.get("objects", {}).get(str(source), {}).get("abilities", [])
    matches = [r for r in rows if r.get("index") == index]
    if len(matches) != 1:
        _refuse(f"native ability {source}.{index} has no unique captured row")
    return matches[0]


def _mana(adapter, action, after, path=()):
    """Mana produced by this exact activation; fixed text is safe only in full."""
    evidence = after.get("payment_selection", {}).get("quote", after.get("payment_quote", {}))
    if "productions" in evidence:
        groups = [r for r in evidence["productions"] if r.get("path") == list(path) and r.get("triggered") is False]
        if any(r.get("source") != action["source"] or r.get("triggered") is not False for r in groups):
            _refuse("production receipt names another activation invocation")
        if not groups and (payment_suspension(evidence) or {}).get("stage") == "mana_choice" and not any(r.get("path") == list(path) for r in evidence.get("payments", [])):
            row = _row(adapter, action["source"], action["ability"])
            fixed = re.fullmatch(r".*?: Add ((?:\{[WUBRGC]\})+)\.", row.get("text") or "")
            if fixed:
                return re.findall(r"\{([WUBRGC])\}", fixed[1])
            _refuse("future variable production needs its explicit retained declaration")
        return [mana for r in groups for mana in r["mana"]]
    source = action["source"]
    row = _row(adapter, source, action["ability"])
    events = [e["mana"] for e in after.get("state", {}).get("events", [])
              if e.get("event") == "mana_added" and e.get("source") == source]
    # Repeated uses of one source cannot be paired by source alone. Fixed text
    # remains exact; otherwise require an unambiguous single activation event.
    text = row.get("text") or ""
    fixed = re.fullmatch(r".*?: Add ((?:\{[WUBRGC]\})+)\.", text)
    if len(events) == 1:
        return deepcopy(events[0])
    if not events and fixed:
        return re.findall(r"\{([WUBRGC])\}", fixed[1])
    adding = action.get("adding") or []
    # The native field lists choices of instructions, not necessarily the total
    # production. A captured event is required for variable/doubled production.
    if adding:
        _refuse("mana production lacks an unambiguous captured mana_added event")
    return None


def _main_activation(action, after):
    """Payment activations precede the announced activation (601.2g, 602.2b)."""
    events = [e for e in after.get("state", {}).get("events", []) if e.get("event") == "activated"]
    if not events:
        return None
    evidence = after.get("payment_selection", {}).get("quote", after.get("payment_quote", {}))
    if "productions" in evidence:
        receipt = next((r for r in evidence.get("payments", []) if r.get("path") == []), None)
        if receipt is None or receipt.get("source") != action["source"]:
            _refuse("main activation lacks its scoped payment receipt")
        return events[-1]
    expected = [step["source"] for step in action.get("plan", [])] + [action["source"]]
    if [e.get("source") for e in events] != expected:
        _refuse("activation events cannot be paired uniquely with the native payment plan and main action")
    return events[-1]


def _ability(adapter, action, after, require_mana=False, path=()):
    source = action["source"]
    row = _row(adapter, source, action["ability"])
    out = {"source": adapter._record_id(source), "ability": adapter._ability_ref(row, source)}
    # Native activated events explicitly distinguish stack abilities from mana
    # abilities; a fixed Add instruction alone need not be a mana ability (605.1a).
    activated = None if require_mana else _main_activation(action, after)
    mana_ability = require_mana or (activated is not None and activated.get("ability") is None)
    if activated is None and not require_mana and re.search(r"\bAdd\b", row.get("text") or ""):
        _refuse("activation lacks an event distinguishing a mana ability from a stack ability")
    if mana_ability:
        adding = _mana(adapter, action, after, path)
        if adding is None:
            _refuse("mana ability's production is absent from the capture")
        out["adding"] = adding
    return out


def _payment(adapter, move, after, infer_pool=True, object_groups=None):
    pay = {}
    def children(native, path=()):
        abilities = []
        for index, step in enumerate(native.get("plan", [])):
            child_path = (*path, index)
            evidence = after.get("payment_selection", {}).get("quote", after.get("payment_quote", {}))
            if any(step.get(k) is not None for k in ("x", "plan", "life")) and "productions" not in evidence:
                _refuse("native mana-plan step lacks retained nested X, life or mana-window evidence")
            if set(step) - {"source", "ability", "paying", "adding", "chose", "cost_branches", "plan", "x", "life", "mana"}:
                _refuse("native mana-plan step has an unsupported activation field")
            if step.get("cost_branches") and not step.get("chose"):
                _refuse("nested typed cost branches lack retained cost-text positional choices")
            ability = _ability(adapter, step, after, require_mana=True, path=child_path)
            nested = {}
            descendants = children(step, child_path)
            if descendants: nested["abilities"] = descendants
            if step.get("paying"):
                nested["objects"] = [[adapter._record_id(o) for o in group] for group in step["paying"]]
            if step.get("life"): nested["life"] = list(step["life"])
            if nested: ability["pay"] = nested
            if step.get("x") is not None: ability["x"] = step["x"]
            if step.get("chose"): ability["additional"] = {"choices": [n + 1 for n in step["chose"]]}
            abilities.append(ability)
        return abilities
    abilities = children(move)
    if abilities:
        pay["abilities"] = abilities
    groups = move.get("paying", []) if object_groups is None else object_groups
    if groups:
        pay["objects"] = [[adapter._record_id(o) for o in group] for group in groups]
    if move.get("life"):
        pay["life"] = list(move["life"])
    if after.get("payment_selection") is not None or after.get("payment_quote") is not None:
        player = (adapter.state.get("asked") or {}).get("acting", (adapter.state.get("asked") or {}).get("player"))
        following = after.get("state", {}).get("players", [])
        if type(player) is not int or not 0 <= player < len(following):
            _refuse("payment_selection has no captured acting player and after pool")
        try:
            helper = restore_payment if after.get("payment_selection") is not None else invert_payment_quote
            evidence = after["payment_selection"] if after.get("payment_selection") is not None else after["payment_quote"]
            quote = evidence.get("quote", evidence)
            boundary = payment_suspension(quote)
            if boundary is not None:
                captured = after.get("audit", {}).get("objects", {}).get(str(boundary["source"]), {})
                if (not _quoted_order_matches(boundary, after.get("state", {}))
                        or captured.get("announced") != boundary["announced"]):
                    raise PaymentEvidenceError("suspended payment disagrees with captured pending order or announcement")
            return helper(evidence, move,
                adapter.state["players"][player].get("pool", {}), following[player].get("pool", {}),
                after.get("state", {}).get("events", []), pay, player,
                restricted_lot=getattr(adapter, "_restricted_lot", None), record_id=adapter._record_id)
        except PaymentEvidenceError as error:
            _refuse(str(error))
    mana = move.get("mana")
    if mana is not None and (not isinstance(mana, dict) or set(mana) - MANA - {"restricted", "restricted_after"}
                             or mana.get("restricted") or mana.get("restricted_after")
                             or any(type(v) is not int or v < 0 for k, v in mana.items() if k not in {"restricted", "restricted_after"})):
        _refuse("restricted or malformed native mana payment has no captured allocation")
    pool = {}
    if infer_pool or mana is not None:
        player = (adapter.state.get("asked") or {}).get("acting", (adapter.state.get("asked") or {}).get("player"))
        if player is None:
            _refuse("payment has no captured acting player")
        before = adapter.state["players"][player].get("pool", {})
        following = after.get("state", {}).get("players", [])
        if player >= len(following):
            _refuse("payment has no after pool")
        remaining = dict(following[player].get("pool", {}))
        # The main mana ability resolves only after its cost is paid (605.3).
        if move.get("kind") == "activate":
            activated = _main_activation(move, after)
            if activated is not None and activated.get("ability") is None:
                for color in _mana(adapter, move, after) or []:
                    remaining[color] = remaining.get(color, 0) - 1
        if before.get("restricted") or remaining.get("restricted"):
            _refuse("restricted mana needs an explicit allocation")
        known_sources = {step["source"] for step in move.get("plan", [])}
        if move.get("kind") == "activate":
            known_sources.add(move["source"])
        if any(e.get("event") == "mana_added" and e.get("source") not in known_sources
               for e in after.get("state", {}).get("events", [])):
            _refuse("additional mana production has no unambiguous payment provenance")
        generated = {color: sum(a["adding"].count(color) for a in abilities) for color in MANA}
        try:
            pool = prior_spending(before, remaining, generated, mana)
        except PaymentEvidenceError as error:
            _refuse(str(error))
    if pool:
        pay["pool"] = pool
    return pay


def _announcement(adapter, move, after, object_groups=None):
    out = {}
    for key in ("x", "division"):
        if key in move and move[key] is not None:
            out[key] = deepcopy(move[key])
    if move.get("modes"):
        out["modes"] = [n + 1 for n in move["modes"]]
    if move.get("targets"):
        out["targets"] = [[_target(adapter, t) for t in group] for group in move["targets"]]
    additional = {}
    for native, mgn in (("kicks", "kicker"), ("buyback", "buyback"), ("entwine", "entwine"), ("replicate", "replicate")):
        if move.get(native):
            additional[mgn] = move[native]
    paid = [k for k in ("bargain", "offspring", "casualty", "teamwork") if move.get(k)]
    if move.get("additional"):
        item = adapter.audit["objects"].get(str(move.get("card")), {})
        options = item.get("casting_costs", {}).get("optional", [])
        if len(options) != 1 or not keyword_name(options[0].get("text", "")):
            _refuse("unnamed optional additional cost has no unique captured keyword")
        paid.append(keyword_name(options[0]["text"]))
    if paid:
        additional["paid"] = paid
    if move.get("gift") is not None:
        player = move["gift"]
        if isinstance(player, str) and player.startswith("p"):
            player = int(player[1:])
        additional["gift"] = adapter.seats[player]
    if move.get("chose"):
        additional["choices"] = [n + 1 for n in move["chose"]]
    if move.get("cost_branches") and move.get("kind") == "activate":
        if not move.get("chose"):
            _refuse("typed activation cost branches lack retained cost-text positional choices")
    elif move.get("cost_branches"):
        branches = move["cost_branches"]
        if any(b.get("branch", {}).get("kind") != "printed_or" or b.get("branch", {}).get("forage") is not None for b in branches):
            _refuse("typed native cost branch is not an alternative choice")
        # Typed branch inputs can name sites in any order. MGN choices follow
        # cost-text order, which the retained announcement supplies.
        objects = after.get("audit", {}).get("objects", {})
        cast = [objects.get(str(e["spell"]), {}) for e in after.get("state", {}).get("events", []) if e.get("event") == "cast"]
        if len(cast) != 1:
            _refuse("typed cost choices lack a unique retained cast announcement")
        retained = cast[0].get("announced", {}).get("cost_branches")
        if not isinstance(retained, list) or len(retained) != len(branches) or any(b.get("branch", {}).get("kind") != "printed_or" or b.get("branch", {}).get("forage") is not None for b in retained):
            _refuse("typed cost choices lack complete retained cost-text order")
        choices = [b["branch"]["arm"] for b in retained]
        if sorted(choices) != sorted(b["branch"]["arm"] for b in branches):
            _refuse("native and retained typed cost choices disagree")
        if additional.get("choices", choices) != choices:
            _refuse("native typed and positional cost choices disagree")
        additional["choices"] = choices
    if additional:
        out["additional"] = additional
    pay = _payment(adapter, move, after, object_groups=object_groups)
    if pay:
        out["pay"] = pay
    return out


def _cost(adapter, move, kind):
    item = adapter.audit.get("objects", {}).get(str(move["card"]), {})
    if kind == "cast_during_resolution":
        offers = item.get("offered_casts")
        player = adapter.state["asked"].get("acting", adapter.state["asked"]["player"])
        selected = [o for o in offers or [] if o["player"] == player and o["cast_as"] == move.get("cast_as", "whole")]
        explicit_cost = move.get("cost") is not None
        choice = move.get("cost")
        rows = item.get("definition_rows", [])
        index = choice.get("alternative") if isinstance(choice, dict) else None
        paid_permission = (move["card"] in adapter.state["asked"].get("cards", [])
                           and move.get("cast_as", "whole") in ("whole", {"face": 0})
                           and type(index) is int and 0 <= index < len(rows)
                           and keyword_name(rows[index]) == "madness")
        if not selected and not paid_permission:
            if offers is None:
                _refuse("offered_casts missing: casting permission and cost cannot be authenticated")
            _refuse("no captured casting permission matches this player, face and explicit cost")
        if any(o["without_paying"] for o in selected) and not explicit_cost:
            return {"kind": "alternative", "effect": "without_paying"}
    if move.get("cost") is not None:
        choice = move["cost"]
        if choice == "mana":
            return None
        if not isinstance(choice, dict) or set(choice) != {"alternative"}:
            _refuse("native casting cost has no MGN representation")
        rows = item.get("casting_costs", {}).get("alternatives", [])
        matches = [r for r in rows if r.get("paragraph") == choice["alternative"] + 1]
        keyword = keyword_name(matches[0].get("text", "")) if len(matches) == 1 else None
        if len(matches) == 1 and keyword is None:
            return {"kind": "alternative", "source": adapter._record_id(move["card"]),
                    "ability": adapter._ability_ref(matches[0], move["card"])}
        if not matches:
            # Keyword costs such as madness are retained as definition rows,
            # even when no ordinary casting-cost alternative is currently live.
            definitions = item.get("definition_rows", [])
            paragraph = choice["alternative"]
            if 0 <= paragraph < len(definitions):
                keyword = keyword_name(definitions[paragraph])
        if keyword is None:
            _refuse("alternative casting cost lacks a unique keyword provenance")
        return {"kind": "alternative", "keyword": keyword}
    return None


def _cast_evidence(adapter, move, after):
    """Authenticate route and payment choices against the completed cast, not input wrappers.

    Native choice groups append delve then convoke after printed/additional groups.
    Harmonize uses an announced fixed creature and consumes no choosing group.
    """
    objects = after.get("audit", {}).get("objects", {})
    transitions = after.get("state", {}).get("events", [])
    events = [event for event in transitions if event.get("event") == "cast"]
    sources = [event["spell"] for event in events]
    before = adapter.audit.get("objects", {}).get(str(move["card"]), {})
    quote = after.get("payment_selection", {}).get("quote", after.get("payment_quote", {}))
    try:
        boundary = payment_suspension(quote)
    except PaymentEvidenceError as error:
        _refuse(str(error))
    if boundary is not None:
        # An announced spell is on the stack but has no Cast event until its
        # retained object costs complete. Use the actual authenticated successor.
        if events:
            _refuse("suspended announcement already has a cast event")
        item = objects.get(str(boundary["source"]), {})
        if (item.get("announced") != boundary["announced"] or item.get("zone") != "Stack"
                or item.get("physical") != before.get("physical")
                or (before.get("zone") is not None
                    and str(before["zone"]).lower() != boundary["announced"].get("cast_from"))
                or not _quoted_order_matches(boundary, after.get("state", {}))):
            _refuse("suspended casting source or pending owner-order is not captured exactly")
        sources = [boundary["source"]]
    physical = before.get("physical")
    if physical is not None:
        sources = [source for source in sources if objects.get(str(source), {}).get("physical") == physical]
    elif "physical" in before:
        # A prepared card copy is cast from exile (722), unlike a stack copy
        # put directly on the stack (707.10). Bind the actual input incarnation
        # and its preserved prepared donor; matching characteristics are no proof.
        donor = before.get("prepare_source")
        if type(donor) is not int or before.get("zone") != "Exile" or before.get("stack_copy_of") is not None:
            _refuse("prepared card copy lacks its exact exiled source and prepared donor")
        departures = [event for event in transitions if event.get("event") == "zone"
                      and event.get("was") == move["card"] and event.get("from") == "Exile" and event.get("to") == "Stack"]
        sources = [source for source in sources
                  if len([transition for transition in departures if transition.get("now") == source]) == 1
                  and (recipient := objects.get(str(source), {})).get("prepare_source") == donor
                  and recipient.get("physical") is None and recipient.get("zone") == "Stack"
                  and recipient.get("stack_copy_of") is None]
        if len(sources) != 1:
            _refuse("prepared card copy lacks an exact input-to-cast transition and matching prepared donor")
    enriched = [objects.get(str(source), {}) for source in sources]
    required = ("physical" in before and physical is None) or bool(move.get("convoke") or move.get("delve") or move.get("improvise") or move.get("route") not in (None, "normal"))
    if len(enriched) != 1 or (enriched[0].get("announced") or {}).get("cast") is None:
        if required or any((item.get("announced") or {}).get("cast") is not None for item in enriched):
            _refuse("native casting route and chosen objects need one completed retained cast announcement")
        return {}, None  # old simple hand-cast evidence has no specialized choice to infer
    item = enriched[0]
    announced = item.get("announced") or {}
    retained = announced["cast"]
    if not isinstance(retained, dict) or "physical" not in before:
        _refuse("native accepted cast lacks its exact source identity or announcement")
    actor = (adapter.state.get("asked") or {}).get("acting", (adapter.state.get("asked") or {}).get("player"))
    if (retained.get("completed") is not True and boundary is None) or item.get("stack_copy_of") is not None or announced.get("cast_by") != actor:
        _refuse("native casting evidence is incomplete, copied, or belongs to another caster")
    facts = KarnAdapter._cast_route_from_announced(adapter, item, adapter._record_id(move["card"]))
    route = retained["route"]
    explicit = move.get("route")
    if explicit is not None:
        if isinstance(explicit, str):
            expected = {"kind": explicit}
            if explicit == "harmonize":
                expected["tapped"] = None
        elif isinstance(explicit, dict) and set(explicit) == {"harmonize"}:
            expected = {"kind": "harmonize", "tapped": explicit["harmonize"]}
        elif isinstance(explicit, dict) and set(explicit) == {"source", "ability"}:
            expected = {"kind": "permission", **explicit}
        else:
            _refuse("native casting route is malformed")
        if route != expected:
            _refuse("native casting route disagrees with the accepted announcement")
    if boundary is not None and boundary.get("stage") == "mana_choice":
        if move.get("convoke") or move.get("delve") or move.get("improvise"):
            _refuse("unfinished mana choice does not authenticate substitution spending")
        return facts, move.get("paying", [])
    payment = boundary["substitutions"] if boundary is not None else retained["payment"]
    convoke, delve, improvise = payment["convoke"], payment["delve"], payment.get("improvise", [])
    if len(improvise) != move.get("improvise", 0) or len(delve) != move.get("delve", 0) or [part["color"] for part in convoke] != [None if color == "generic" else color for color in move.get("convoke", [])]:
        _refuse("native substitution count or allocation disagrees with the completed payment")
    groups = move.get("paying", [])
    suffix = ([delve] if delve else []) + [[part["object"]] for part in convoke] + ([improvise] if improvise else [])
    if suffix and (len(groups) < len(suffix) or groups[-len(suffix):] != suffix):
        _refuse("native chosen objects disagree with the completed substitution payment")
    return facts, groups[:-len(suffix)] if suffix else groups


def _effect(adapter, offered):
    if offered.get("rule"):
        return {"rule": offered["rule"]}
    producer = offered.get("producer") or offered
    source = producer.get("source", offered.get("source"))
    if source is None:
        _refuse("selected effect lacks a source")
    if producer.get("ability") is not None:
        row = _row(adapter, source, producer["ability"])
    elif producer.get("paragraph") is not None:
        rows = adapter.audit.get("objects", {}).get(str(source), {}).get("abilities", [])
        matches = [r for r in rows if r.get("paragraph") == producer["paragraph"]]
        if len(matches) != 1:
            _refuse("effect producer has no unique ability row")
        row = matches[0]
    else:
        _refuse("effect producer lacks an ability")
    return {"source": adapter._record_id(source), "ability": adapter._ability_ref(row, source)}


def _continued_slice(adapter, move, after, pay, prefix, answer):
    boundary = payment_suspension(prefix)
    if (boundary is None or boundary.get("stage") != "mana_choice"
            or not _quoted_order_matches(boundary, adapter.state)
            or adapter.audit.get("objects", {}).get(str(boundary["source"]), {}).get("announced") != boundary["announced"]):
        raise PaymentEvidenceError("answer does not start at its exact retained pending trigger and announcement")
    player = boundary["player"]
    joined = continuation_payment(prefix, answer)
    _validate_quote_frames(answer, move, adapter.state["players"][player].get("pool", {}),
        after["state"]["players"][player].get("pool", {}), after["state"].get("events", []), player)
    next_boundary = payment_suspension(joined)
    if next_boundary is not None:
        if ("pending_payment" in move or not _quoted_order_matches(next_boundary, after["state"])
                or after.get("audit", {}).get("objects", {}).get(str(boundary["source"]), {}).get("announced") != next_boundary["announced"]):
            raise PaymentEvidenceError("intermediate answer changed its actual next trigger or preselected root spending")
    else:
        selected = select_payment(pay, prefix["prior_pools"][player], joined,
            restricted_lot=adapter._restricted_lot, record_id=adapter._record_id)
        expected = {"source":boundary["source"], "path":[], "mana":continuation_selector(
            selected, prefix["prior_pools"][player], answer["payments"][-1]["pool"])}
        if move.get("pending_payment") != expected:
            raise PaymentEvidenceError("pending answer changed its exact retained root spending")
        joined["move"]["mana"] = selected
        announced = after.get("audit", {}).get("objects", {}).get(str(boundary["source"]), {}).get("announced", {})
        if announced.get("cast", {}).get("completed") is not True:
            raise PaymentEvidenceError("pending answer did not complete its actual captured spell announcement")
    validate_bound_payment(joined, joined["move"], player, validate_production(pay, joined))
    return joined


def _continued_mana(adapter, move, after, question):
    capture = after.get("payment_continuation")
    if question != "choose_mana" or not isinstance(capture, dict) or set(capture) != {"pay", "prefix", "answer", "before", "history"}:
        _refuse("pending payment answer needs its exact retained before, prefix and answer capture")
    try:
        history = capture["history"]
        if not isinstance(history, list) or not history:
            raise PaymentEvidenceError("pending answer lacks its ordered original execution slices")
        origin = copy(adapter)
        origin.state, origin.audit = deepcopy(capture["before"]["state"]), deepcopy(capture["before"]["audit"])
        first = history[0]
        if first["quote"].get("move", {}).get("kind") != "cast" or first["quote"].get("continuation") is not None:
            raise PaymentEvidenceError("ordered payment slices must begin with the original cast")
        initial = native_action(origin, first["quote"]["move"], {**first["after"],
            "payment_selection":{"pay":capture["pay"], "quote":first["quote"]}})
        if initial.get("pay", {}) != capture["pay"]:
            raise PaymentEvidenceError("pending answer changed its original authored payment")
        joined = first["quote"]
        origin.state, origin.audit = first["after"]["state"], first["after"]["audit"]
        for step in history[1:]:
            joined = _continued_slice(origin, step["quote"]["move"], step["after"], capture["pay"], joined, step["quote"])
            origin.state, origin.audit = step["after"]["state"], step["after"]["audit"]
        if joined != capture["prefix"] or origin.state != adapter.state or origin.audit != adapter.audit:
            raise PaymentEvidenceError("ordered payment slices do not reach the exact captured before frame")
        _continued_slice(adapter, move, after, capture["pay"], joined, capture["answer"])
        return {"kind":"choose_mana", "mana":deepcopy(move["answer"]["mana"])}
    except (PaymentEvidenceError, InvalidPayment, KeyError, TypeError, IndexError) as error:
        _refuse(str(error))


def native_action(adapter, move: dict, after: dict, pending: dict | None = None) -> dict:
    """Return one explicit MGN answer from a captured move and exact frames.

    `pending` is required for trigger ordering; it avoids engine prompt calls and
    retains the projection's disambiguating `for` subjects. No adapter state changes.
    """
    asked = adapter.state.get("asked") or {}
    question = pending["kind"] if pending else adapter._asked_kind(asked) if asked.get("kind") else None
    kind = move.get("kind")
    unknown = set(move) - FIELDS.get(kind, set()) - {"kind"}
    if unknown:
        _refuse(f"native move fields have no MGN mapping: {sorted(unknown)}")
    if "pending_payment" in move or after.get("payment_continuation") is not None:
        return _continued_mana(adapter, move, after, question)
    if kind in ("pass", "concede"):
        return {"kind": kind}
    if kind in ("keep", "mulligan"):
        return {"kind": "mulligan", "keep": kind == "keep"}
    if kind in ("discard", "bottom"):
        return {"kind": kind, "cards": [adapter._record_id(c) for c in move["cards"]]}
    if kind == "play":
        out = {"kind": "play_land", "card": adapter._record_id(move["card"])}
        if move.get("face"):
            out["face"] = move["face"]
        return out
    if kind in ("cast", "activate"):
        unhandled = {k for k in ("slots", "asks", "face_down", "alternatives") if move.get(k)}
        if unhandled:
            _refuse(f"native announcement fields lack an MGN mapping: {sorted(unhandled)}")
        # `pay_life` is the native listing's informational total, explicitly
        # unread by read_move_offered (game-serve.rs). The actual announcement is
        # `life`, a Phyrexian-symbol choice vector (107.4f, 601.2b); fixed life
        # costs follow the selected cost and are paid under 601.2h, 118.9.
        if "pay_life" in move and (type(move["pay_life"]) is not int or move["pay_life"] < 0):
            _refuse("native informational pay_life is not a nonnegative integer")
        out = {"kind": "cast_during_resolution" if kind == "cast" and question == "cast_during_resolution" else kind}
        if kind == "cast":
            out["card"] = adapter._record_id(move["card"])
            cast_facts, object_groups = _cast_evidence(adapter, move, after)
            cost = _cost(adapter, move, out["kind"])
            if cost:
                out["cost"] = cost
            if "cost" in cast_facts and cost is not None and cast_facts["cost"] != cost:
                _refuse("native alternative cost disagrees with the retained casting route")
            out.update(cast_facts)
            cast_as = move.get("cast_as", "whole")
            if cast_as != "whole":
                if cast_as == "fused":
                    if out["card"] not in adapter.cards or not getattr(adapter.oracle, "has_fuse", lambda _: False)(adapter.cards[out["card"]]["oracle"]):
                        _refuse("native fused casting lacks a pinned fuse split card")
                    out["fused"] = True
                elif isinstance(cast_as, dict) and set(cast_as) == {"face"}:
                    out["face"] = cast_as["face"]
                else:
                    faces = adapter.oracle.faces[adapter.cards[out["card"]]["oracle"]]
                    matches = [i for i in range(len(faces)) if adapter._cast_as(out["card"], i) == cast_as]
                    if len(matches) != 1:
                        _refuse("native alternate face has no unique Oracle face")
                    if matches[0]:
                        out["face"] = matches[0]

        else:
            out.update(_ability(adapter, move, after))
        announced = _announcement(adapter, move, after, object_groups=object_groups if kind == "cast" else None)
        if kind == "activate" and set(announced.get("additional", {})) - {"choices"}:
            _refuse("activation carries a spell-only additional cost")
        out.update(announced)
        if kind == "cast" and "pay" not in out and out["card"] in adapter.cards:
            face = adapter.oracle.faces[adapter.cards[out["card"]]["oracle"]][out.get("face", 0)]
            if face.raw.get("mana_cost") == "{0}":
                out["pay"] = {}  # an explicit zero cost is paid (118.5), not absent
        return out
    if kind in ("plot", "foretell", "suspend", "unlock", "turn_face_up", "companion"):
        if kind == "companion":
            _refuse("native companion action does not name its card")
        key = {"unlock": "room", "turn_face_up": "permanent"}.get(kind, "card")
        out = {"kind": "special_action", "action": kind, "object": adapter._record_id(move[key])}
        if kind == "unlock":
            out["half"] = move.get("half", 0)
        if kind == "turn_face_up" and move.get("way", "morph") != "morph":
            _refuse("MGN turn_face_up does not encode a native way choice")
        pay = _payment(adapter, move, after)
        if pay:
            out["pay"] = pay
        return out
    if kind == "attack":
        return {"kind": "declare_attackers", "attacks": [[adapter._record_id(a), _target(adapter, d)] for a, d in move.get("attacks", [])]}
    if kind == "block":
        return {"kind": "declare_blockers", "blocks": [[adapter._record_id(b), adapter._record_id(a)] for b, a in move.get("blocks", [])]}
    if kind == "divide":
        key = "assignments" if question == "assign_combat_damage" else "amounts"
        return {"kind": question, key: [[_target(adapter, t), n] for t, n in move["division"]]}
    if kind == "modes" and question != "choose_modes":
        _refuse("native modes answer does not match a choose_modes boundary")
    if kind in ("targets", "modes"):
        if question == "choose_modes":
            if move.get("targets") or move.get("division"):
                _refuse("combined modal targeting needs explicit boundary decomposition")
            return {"kind": question, "modes": [m + 1 for m in move.get("modes", [])]}
        out = {"kind": question, "targets": [[_target(adapter, t) for t in g] for g in move.get("targets", [])]}
        if move.get("division"):
            out["division"] = deepcopy(move["division"])
        return out
    if kind == "order":
        if pending is None:
            _refuse("trigger ordering needs the projected ability instances")
        instances = pending.get("about", {}).get("abilities", [])
        if sorted(move["order"]) != list(range(len(instances))):
            _refuse("native trigger order is not a permutation of captured instances")
        return {"kind": "order_triggers", "order": [deepcopy(instances[i]) for i in move["order"]]}
    if kind != "choose":
        _refuse(f"native move {kind} has no MGN translation")
    answer = move.get("answer")
    out = {"kind": question}
    if question in ("optional", "command_zone"):
        if answer not in ("yes", "no"):
            _refuse("native boolean choice is not yes or no")
        return {**out, "yes": answer == "yes"}
    if answer == "no":
        if question == "change_targets":
            target = asked.get("object", asked.get("copy"))
            targets = adapter.state.get("objects", {}).get(str(target), {}).get("targets")
            if targets is None and asked.get("kind") == "retarget":
                # A prepared stack copy is not on the stack until its targets
                # are chosen (707.10c). Its exact lineage and the native prompt's
                # frozen original targets authenticate what declining preserves.
                copy = adapter.audit.get("objects", {}).get(str(target), {})
                original = copy.get("stack_copy_of")
                frozen = asked.get("original")
                if (type(original) is int and str(original) in adapter.audit.get("objects", {})
                        and isinstance(frozen, list) and all(isinstance(g, list) for g in frozen)):
                    retained = adapter.state.get("objects", {}).get(str(original), {}).get("targets")
                    if retained is not None and retained != frozen:
                        _refuse("copy's frozen original targets disagree with before state")
                    targets = frozen
            if targets is None:
                _refuse("unchanged targets absent from authenticated before evidence")
            return {**out, "targets": [[_target(adapter, t) for t in g] for g in targets]}
        if question in ("pay", "cast_during_resolution", "choose_alternative"):
            return {**out, "decline": True}
    if answer == "yes" and question == "pay":
        # Native Yes follows exactly the plan retained by Asked::Pay; it does
        # not search anew (program.rs, combat.rs; CR 118.3, 601.2g-h).
        if not isinstance(asked.get("plan"), list):
            _refuse("native pay yes lacks its captured payment plan")
        if asked.get("paying") or asked.get("tapped"):
            _refuse("native pay yes has waterbend objects without a cost-position mapping")
        return {**out, "pay": _payment(adapter, {"plan": asked["plan"]}, after)}
    if not isinstance(answer, dict):
        _refuse(f"native {question} answer is not a supported typed choice")
    ids = lambda values: [adapter._record_id(o) for o in values]
    if question == "arrange" and asked.get("kind") == "order_cards":
        marker = asked.get("arrange")
        offered, ordered = asked.get("cards"), answer.get("objects")
        if (not isinstance(marker, dict) or set(marker) != {"object"}
                or type(marker["object"]) is not int or marker["object"] not in adapter.state.get("stack", [])
                or (asked.get("to") or {}) != {"zone":"library", "position":"top"}
                or set(answer) != {"objects"} or not isinstance(offered, list) or not isinstance(ordered, list)
                or len(ordered) != len(offered) or len(set(ordered)) != len(ordered) or set(ordered) != set(offered)):
            _refuse("look-and-return arrangement lacks its exact native instruction or permutation")
        return {"kind":"arrange", "top":ids(ordered), "away":[]}
    if question == "order_cards" or asked.get("kind") == "order_cards":
        if question != "order_cards" or asked.get("kind") != "order_cards":
            _refuse("owner order needs the authenticated native order_cards question")
        offered, ordered = asked.get("cards"), answer.get("objects")
        if (set(answer) != {"objects"} or not isinstance(offered, list) or not isinstance(ordered, list)
                or any(type(card) is not int for card in ordered)
                or len(ordered) != len(offered) or len(set(ordered)) != len(ordered)
                or set(ordered) != set(offered)):
            _refuse("native owner order is not a permutation of the offered cards")
        zone = (asked.get("to") or {}).get("zone")
        owner = adapter.seats[asked["player"]]
        if zone not in ("library", "graveyard") or any(adapter.cards.get(card, {}).get("owner") != owner for card in ids(offered)):
            _refuse("MGN order_cards requires the ordering owner of a library or graveyard")
        # Both forms are top first, including a graveyard's incoming block. The
        # following frame may already have consumed cards; it cannot supply this input.
        return {"kind": "order_cards", "order": ids(ordered)}
    if question in ("choose_objects", "search", "opening_hand_action", "separate"):
        objects = answer.get("objects", []) if answer.get("accept", True) else []
        if question == "opening_hand_action":
            if len(objects) > 1:
                _refuse("opening hand action chooses more than one card")
            return {**out, **({"card": adapter._record_id(objects[0])} if objects else {"done": True})}
        if question == "separate":
            return {**out, "piles": [ids(objects), ids([o for o in asked["among"] if o not in objects])]}
        return {**out, "found" if question == "search" else "objects": ids(objects)}
    if question == "arrange":
        return {**out, "top": ids(answer["top"]), "away": ids(answer["away"])}
    if question == "library_destination":
        at = answer["library_position"]
        return {**out, "at": at["position"] if "position" in at else {"from_top": at["from_top"]}}
    if question in ("choose_targets", "change_targets"):
        return {**out, "targets": [[_target(adapter, t) for t in g] for g in answer["targets"]]}
    if question == "choose_player":
        player = answer["player"]
        return {**out, "player": adapter.seats[int(player[1:]) if isinstance(player, str) else player]}
    if question == "choose_number":
        return {**out, "number": answer["value"]["number"]}
    if question == "coin_call":
        if answer.get("call") not in ("heads", "tails"):
            _refuse("a native coin call is heads or tails")
        return {**out, "call": answer["call"]}
    if question == "choose_value":
        if len(answer["value"]) != 1:
            _refuse("native value choice must name one value")
        native_of, value = next(iter(answer["value"].items()))
        domain = {"creature_type": "creature", "basic_land_type": "basic_land", "land_type": "land"}.get(native_of)
        if domain:
            try:
                value = canonical_subtype(value, kind=domain)
            except ValueError as error:
                _refuse(str(error))
        return {**out, "value": value}
    if question == "choose_mana":
        return {**out, "mana": deepcopy(answer["mana"])}
    if question == "holders":
        return {**out, "holders": [_target(adapter, t) for t in answer["holders"]]}
    if question in ("attach", "choose_defender"):
        return {**out, "to" if question == "attach" else "defender": _target(adapter, answer["attach" if question == "attach" else "defender"])}
    if question in ("pile", "choose_alternative"):
        key = "pile" if question == "pile" else "alternative"
        return {**out, key: answer[key] + 1}
    if question == "pay":
        return {**out, "pay": _payment(adapter, answer["pay"], after)}
    if question in ("choose_replacement", "spell_destination", "skip"):
        field = {"choose_replacement": "replacement", "spell_destination": "spell_destination", "skip": "skip"}[question]
        selected = answer[field]
        if question == "spell_destination":
            matches = [e for e in asked.get("among", []) if e.get("effect") == selected["effect"]]
        elif question == "skip":
            matches = [e for e in asked.get("among", []) if e.get("id") == selected]
        else:
            matches = [e for e in asked.get("among", []) if all((e.get("producer") or e).get(k, e.get(k, 0 if k == "part" else None)) == v for k, v in selected.items())]
        if len(matches) != 1:
            _refuse("selected effect has no unique captured offered producer")
        return {**out, "effect": _effect(adapter, matches[0])}
    if question == "legend_rule" and "keep" in answer:
        return {**out, "keep": adapter._record_id(answer["keep"])}
    _refuse(f"native {question} choice has no MGN translation")
