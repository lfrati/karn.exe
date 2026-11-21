"""Exact MGN/native payment boundary, using captured locked costs, never Oracle guesses.

CR 601.2f locks the cost before the mana window; 601.2g–h and 605.3
separate production from spending. CR 106.6 keeps restricted units distinct.
The native quote executes a disposable clone and exposes that actual boundary.
"""
from __future__ import annotations

from collections import Counter
from copy import deepcopy
import json

TYPES = "WUBRGC"


class PaymentEvidenceError(ValueError):
    """The available evidence does not determine a faithful payment."""


class InvalidPayment(PaymentEvidenceError):
    """A fully supported locked cost is provably unpaid by the selected resources."""


def payment_suspension(quote):
    """Authenticate the narrow paid-mana, unfinished object-order boundary."""
    boundary = quote.get("suspended")
    if boundary is None:
        return None
    if quote.get("complete") is not False or quote.get("error") is not None:
        raise PaymentEvidenceError("suspended payment claims completion or refusal")
    receipts = quote.get("payments", [])
    if not receipts or "productions" not in quote:
        raise PaymentEvidenceError("suspended payment lacks scoped native receipts")
    if boundary.get("stage") == "mana_choice":
        move, asked = quote.get("move", {}), boundary.get("asked", {})
        nodes = dict(payment_nodes(move))
        trigger, scope = boundary.get("trigger", {}), boundary.get("scope")
        announced = boundary.get("announced", {})
        zones = [e for e in quote.get("events", []) if e.get("event") == "zone"
                 and e.get("was") == move.get("card") and e.get("to") == "Stack" and e.get("now") == boundary.get("source")]
        if (move.get("kind") != "cast" or boundary.get("kind") != "spell" or boundary.get("path") != []
                or type(boundary.get("source")) is not int or type(boundary.get("player")) is not int
                or not isinstance(scope, list) or not scope or tuple(scope) not in nodes
                or asked.get("kind") != "mana" or asked.get("player") != boundary["player"]
                or not isinstance(asked.get("options"), list) or not asked["options"]
                or type(trigger.get("source")) is not int or trigger.get("source") != asked.get("source")
                or type(trigger.get("declaration_index")) is not int
                or len(zones) != 1 or announced.get("cast_by") != boundary["player"]
                or (announced.get("cast") or {}).get("completed") is not False
                or announced.get("cast_from") != zones[0].get("from", "").lower()
                or any(e.get("event") == "cast" for e in quote.get("events", []))
                or any(r.get("path") == [] for r in receipts)):
            raise PaymentEvidenceError("mana-choice suspension lacks its exact unpaid spell root and producing trigger")
        pools=quote.get("pools", [])
        if boundary["player"] >= len(pools) or _resources(boundary.get("pool", {})) != _resources(pools[boundary["player"]]):
            raise PaymentEvidenceError("mana-choice suspension disagrees with its actual unspent pool")
        return boundary
    root = receipts[-1]
    if (not isinstance(boundary, dict) or boundary.get("path") != [] or root.get("path") != []
            or any(boundary.get(k) != root.get(k) for k in ("source", "player", "kind"))
            or type(boundary.get("source")) is not int or type(boundary.get("player")) is not int
            or root.get("left") is None):
        raise PaymentEvidenceError("suspended payment does not name its paid root scope")
    asked = boundary.get("asked") or {}
    cards = asked.get("cards")
    if (asked.get("kind") != "order_cards" or type(asked.get("player")) is not int
            or not isinstance(cards, list) or len(cards) < 2
            or any(type(card) is not int for card in cards) or len(set(cards)) != len(cards)
            or (asked.get("to") or {}).get("zone") not in ("graveyard", "library")):
        raise PaymentEvidenceError("suspended payment lacks its actual owner-order decision")
    announced = boundary.get("announced")
    if not isinstance(announced, dict):
        raise PaymentEvidenceError("suspended payment lacks retained announcement facts")
    move = quote.get("move") or {}
    if move.get("kind") == "cast":
        cast = announced.get("cast") or {}
        zones = [event for event in quote.get("events", []) if event.get("event") == "zone"
                 and event.get("was") == move.get("card") and event.get("to") == "Stack"
                 and event.get("now") == boundary["source"]]
        if (boundary["kind"] != "spell" or len(zones) != 1 or cast.get("completed") is not False
                or cast.get("payment") is not None or announced.get("cast_by") != boundary["player"]
                or not isinstance(zones[0].get("from"), str)
                or announced.get("cast_from") != zones[0]["from"].lower()
                or not isinstance(boundary.get("substitutions"), dict)):
            raise PaymentEvidenceError("suspended payment lacks its exact unfinished spell announcement")
    else:
        raise PaymentEvidenceError("unsupported suspended payment announcement")
    return boundary


def payment_ready(quote):
    if quote.get("complete") is True:
        if "suspended" in quote:
            raise PaymentEvidenceError("completed payment also claims suspension")
        return True
    return payment_suspension(quote) is not None


def _counts(value):
    if not isinstance(value, dict) or set(value) - set(TYPES) - {"restricted"}:
        raise PaymentEvidenceError("malformed mana pool")
    if any(type(value.get(c, 0)) is not int or value.get(c, 0) < 0 for c in TYPES):
        raise PaymentEvidenceError("malformed mana counts")
    return Counter({c: value.get(c, 0) for c in TYPES})


def prior_spending(before, remaining, produced, spent=None):
    """Invert legacy plain pools only where prior/new attribution is unique.

    Same-type old/new mana with a leftover has no observable age tag (106.4);
    a legacy capture cannot be upgraded into an explicit provenance claim.
    """
    if before.get("restricted") or remaining.get("restricted") or (spent or {}).get("restricted"):
        raise PaymentEvidenceError("restricted mana needs an explicit allocation")
    prior, left, generated = _counts(before), _counts(remaining), _counts(produced)
    actual = _counts(spent) if spent is not None else None
    out = {}
    for c in TYPES:
        used = prior[c] + generated[c] - left[c]
        if left[c] < 0 or used < 0 or (actual is not None and actual[c] != used):
            raise PaymentEvidenceError("native total mana spending disagrees with captured pools and production")
        low, high = max(0, used - generated[c]), min(prior[c], used)
        if low != high:
            raise PaymentEvidenceError("pool and newly added mana overlap with leftover mana; spending provenance is ambiguous")
        if low:
            out[c] = low
    return out


def _restricted(pool):
    entries = pool.get("restricted", [])
    if not isinstance(entries, list) or any(not isinstance(r, dict) or r.get("mana") not in TYPES for r in entries):
        raise PaymentEvidenceError("restricted mana lacks typed captured identities")
    return entries


def _prior_units(prior, requested):
    """MGN pool selects unrestricted prior mana exclusively (MGN 5.8)."""
    plain, wanted = _counts(prior), _counts(requested)
    if any(wanted[c] > plain[c] for c in TYPES):
        raise PaymentEvidenceError("MGN unrestricted prior-pool selection exceeds the captured plain pool")
    return wanted, []


def _lot_fact(unit, restricted_lot):
    if restricted_lot is None:
        raise PaymentEvidenceError("restricted mana needs authenticated Oracle, producer and frozen-choice provenance")
    fact = restricted_lot(deepcopy(unit))
    if (not isinstance(fact, dict) or set(fact) - {"type", "restriction", "producer", "chosen"}
            or fact.get("type") != unit.get("mana") or not isinstance(fact.get("restriction"), str)
            or not fact["restriction"]):
        raise PaymentEvidenceError("restricted lot projection has no matching mana type and complete semantic identity")
    return fact


def _select_lots(pool, selectors, restricted_lot):
    """Resolve portable lot descriptions to native units only at the boundary."""
    units = _restricted(pool)
    selected = []
    for selector in selectors:
        if (not isinstance(selector, dict) or set(selector) - {"type", "count", "restriction", "producer", "chosen"}
                or selector.get("type") not in TYPES or type(selector.get("count")) is not int
                or selector["count"] <= 0 or not isinstance(selector.get("restriction"), str)
                or not selector["restriction"]):
            raise PaymentEvidenceError("malformed semantic restricted mana lot selector")
        wanted = {k: v for k, v in selector.items() if k != "count"}
        matches = []
        for index, unit in enumerate(units):
            if index in selected or unit["mana"] != selector["type"]:
                continue
            fact = _lot_fact(unit, restricted_lot)
            if all(fact.get(k) == v for k, v in wanted.items()):
                matches.append(index)
        if len(matches) < selector["count"]:
            raise PaymentEvidenceError("restricted lot selector exceeds its authenticated matching mana")
        identities = {_unit_identity(units[index]) for index in matches}
        if len(identities) > 1 and selector["count"] < len(matches):
            raise PaymentEvidenceError("restricted lot selector needs producer or frozen chosen values to disambiguate units")
        selected.extend(matches[:selector["count"]])
    return selected


def _unit_identity(unit):
    return json.dumps(unit, sort_keys=True, separators=(",", ":"))


def payment_nodes(move, path=()):
    """Authored execution scopes; indices identify invocations, including repeated sources."""
    payment = move.get("answer", {}).get("pay", {}) if move.get("kind") == "choose" else move
    for index, child in enumerate(payment.get("plan", [])):
        yield from payment_nodes(child, (*path, index))
    yield path, payment


def scoped_quote(quote, path):
    """Project one explicitly named execution scope, retaining its descendants."""
    path = tuple(path)
    nodes = dict(payment_nodes(quote.get("move", {})))
    if path not in nodes:
        raise PaymentEvidenceError("payment receipt names no authored execution scope")
    receipts = []
    for receipt in quote.get("payments", []):
        actual = tuple(receipt.get("path", ()))
        if actual[:len(path)] == path:
            receipt = deepcopy(receipt)
            receipt["path"] = list(actual[len(path):])
            receipts.append(receipt)
    own = next((r for r in receipts if r["path"] == []), None)
    if own is None:
        raise PaymentEvidenceError("quote did not reach this scoped locked payment boundary")
    node = deepcopy(nodes[path])
    if path:
        node["kind"] = "activate"
    rows = []
    for row in quote.get("productions", []):
        actual = tuple(row.get("path") or ())
        if actual[:len(path)] == path:
            row = deepcopy(row); row["path"] = list(actual[len(path):]); rows.append(row)
    direct = [r for r in rows if r["path"] == []]
    player = own["player"]
    before = own.get("prior_pool", own["pool"])
    after = direct[-1]["after"] if direct else own["left"]
    return {**quote, "move": node, "payments": receipts, "productions": rows,
            "symbol_indices": own.get("symbol_indices") if path else quote.get("symbol_indices"),
            "prior_pools": [before if i == player else p for i, p in enumerate(quote.get("prior_pools", []))],
            "pools": [after if i == player else p for i, p in enumerate(quote.get("pools", []))]}


def _nested_plan_inputs(pay, move, record_id):
    native = move.get("answer", {}).get("pay", {}) if move.get("kind") == "choose" else move
    authored_steps, native_steps = pay.get("abilities", []), native.get("plan", [])
    if len(authored_steps) != len(native_steps) and any(set(step) & {"x", "additional", "pay"} for step in authored_steps):
        raise PaymentEvidenceError("nested payment has no matching captured native activation plan")
    for authored, step in zip(authored_steps, native_steps):
        if authored.get("x") != step.get("x"):
            raise PaymentEvidenceError("nested activation lacks its own retained X announcement or it disagrees with its native activation")
        choices = authored.get("additional", {}).get("choices", [])
        if choices != [choice + 1 for choice in step.get("chose", [])]:
            raise PaymentEvidenceError("nested mana-ability cost choices disagree with native positional evidence")
        nested = authored.get("pay", {})
        if nested.get("life", []) != step.get("life", []):
            raise PaymentEvidenceError("nested life payment disagrees with its native activation")
        if nested.get("objects") or step.get("paying"):
            if record_id is None:
                raise PaymentEvidenceError("nested object payment needs authenticated native-to-record object references")
            actual = [[record_id(obj) for obj in group] for group in step.get("paying", [])]
            if nested.get("objects", []) != actual:
                raise PaymentEvidenceError("nested object payment disagrees with captured native objects")
        _nested_plan_inputs(nested, step, record_id)


def _spent_resources(receipt):
    if receipt.get("left") is None:
        raise PaymentEvidenceError("nested payment has no completed native spend")
    before, left = _resources(receipt["pool"]), _resources(receipt["left"])
    if any(left[key] > before[key] for key in left):
        raise PaymentEvidenceError("native payment receipt violates spending conservation")
    spent = before - left
    if receipt.get("selected") is not None and _resources(receipt["selected"]) != spent:
        raise PaymentEvidenceError("payment receipt selected resources disagree with spending conservation")
    return spent


def _child_quote(receipt):
    # Native flat plan receipts retain the locked symbols but not their printed
    # indices. Do not manufacture per-symbol allocation provenance for a child.
    return {"payments": [receipt], "move": {"kind": "activate", "source": receipt.get("source")}}


def _validate_child_payments(pay, quote, restricted_lot, record_id):
    abilities = pay.get("abilities", [])
    children = len(abilities) if "productions" in quote else len(quote["payments"]) - 1
    for index in range(children):
        if index >= len(abilities):
            raise PaymentEvidenceError("nested payment has no authored mana ability")
        ability = abilities[index]
        if "productions" in quote:
            child = scoped_quote(quote, (index,))
            receipt = child["payments"][-1]
            prior = receipt["prior_pool"]
        else:
            receipt = quote["payments"][index]
            child, prior = _child_quote(receipt), receipt["pool"]
        spent = _spent_resources(receipt)
        nested = ability.get("pay", {})
        if not spent and not any(nested.get(k) for k in ("pool", "pool_restricted", "allocation", "abilities")):
            continue
        selected = select_payment(nested, prior, child, restricted_lot=restricted_lot, record_id=record_id)
        if _selected_resources(selected, prior, receipt["pool"]) != spent:
            raise PaymentEvidenceError("nested MGN payment differs from the quoted native child spending")


def _prior_survivors(prior, quote):
    """Possible surviving outer prior plain units, and proven restricted origins.

    CR 106.4 plain units of one type are fungible. A child spend can consume
    either old or earlier-produced units; retain the feasible interval rather
    than assigning age to an individual unit. Restricted origins stay exact.
    """
    low, high = _counts(prior), _counts(prior)
    restricted = Counter(_unit_identity(unit) for unit in _restricted(prior))
    for receipt in quote["payments"][:-1]:
        spent = _spent_resources(receipt)
        available, left = _counts(receipt["pool"]), _counts(receipt["left"])
        for color in TYPES:
            used = available[color] - left[color]
            low[color] = max(0, low[color] - used)
            high[color] = min(high[color], left[color])
        for (kind, identity), used in spent.items():
            if kind != "restricted":
                continue
            available_count = _resources(receipt["pool"])[(kind, identity)]
            prior_count = restricted[identity]
            minimum = max(0, used - (available_count - prior_count))
            maximum = min(used, prior_count)
            if minimum != maximum:
                raise PaymentEvidenceError("nested restricted spending has ambiguous prior/generated origin")
            restricted[identity] -= minimum
    return low, high, restricted


def _distributions(total, caps, prefix=()):
    if len(prefix) == len(TYPES) - 1:
        if total <= caps[len(prefix)]:
            yield prefix + (total,)
        return
    i = len(prefix)
    low = max(0, total - sum(caps[i + 1:]))
    for n in range(low, min(total, caps[i]) + 1):
        yield from _distributions(total - n, caps, prefix + (n,))


def _spendings(requirement, lower, upper, allocation, *, witnesses=None):
    """Enumerate distinct exact type totals consistent with explicit symbol choices."""
    symbols = requirement.get("symbols")
    if not isinstance(symbols, list) or any(not isinstance(s, str) for s in symbols):
        raise PaymentEvidenceError("quote lacks the locked mana requirement")
    # Validate the entire locked vocabulary before searching. An unpaid early
    # symbol must not conceal an unsupported later symbol and become an illegal
    # classification without sufficient evidence.
    for symbol in symbols:
        alternatives = symbol.split("/")
        if symbol.isdigit():
            continue
        if len(alternatives) == 2 and alternatives[0] == "2" and alternatives[1] in set(TYPES):
            continue
        if len(alternatives) not in (1, 2) or any(part not in set(TYPES) for part in alternatives):
            raise PaymentEvidenceError("locked symbol needs additional snow, variable or Phyrexian provenance")
    allocation_by_symbol = {}
    for item in allocation:
        if (not isinstance(item, dict) or set(item) != {"symbol", "mana"}
                or type(item["symbol"]) is not int or not 0 <= item["symbol"] < len(symbols)
                or item["mana"] not in TYPES):
            raise PaymentEvidenceError("allocation names no authenticated printed mana symbol")
        allocation_by_symbol.setdefault(item["symbol"], []).append(item["mana"])
    reduction = requirement.get("reduction", 0)
    minimum = requirement.get("minimum", 0)
    substitution = requirement.get("substitution", 0)
    typed_reduction = list(requirement.get("typed_reduction", [0] * 6))
    convoke = list(requirement.get("convoke", [0] * 5))
    if len(typed_reduction) != 6 or len(convoke) != 5 or any(type(n) is not int or n < 0 for n in [reduction, minimum, substitution, *typed_reduction, *convoke]):
        raise PaymentEvidenceError("malformed locked mana reductions")

    def search(at, used, generic, reductions, covered, generic_indices, chosen):
        if at == len(symbols):
            if any(covered):
                return  # colored convoke cannot pay generic or colorless requirements
            # CR 702.51b: determine the total (including a floor) before substituting.
            # Colored requirements paid by creatures still count toward that total.
            typed = sum(used.values()) + sum(convoke)
            generic = max(max(0, generic - reduction - sum(reductions)), minimum - typed) - substitution
            if generic < 0:
                return
            fixed = Counter()
            named = [i for i in generic_indices if i in allocation_by_symbol]
            if named:
                if len(generic_indices) != 1:
                    raise PaymentEvidenceError("allocation across multiple generic symbols lacks reduction provenance")
                fixed.update(allocation_by_symbol[named[0]])
                if sum(fixed.values()) != generic:
                    return
                choices = [tuple(fixed[c] for c in TYPES)]
            else:
                choices = _distributions(generic, [upper[c] - used[c] for c in TYPES])
            for amounts in choices:
                total = used + Counter(dict(zip(TYPES, amounts)))
                if all(lower[c] <= total[c] <= upper[c] for c in TYPES):
                    if witnesses is not None:
                        witness = list(chosen)
                        if len(generic_indices) == 1:
                            witness.extend({"symbol": generic_indices[0], "mana": c} for c in TYPES for _ in range(amounts[TYPES.index(c)]))
                        elif generic and generic_indices:
                            witness = None  # No retained reduction assignment across generic symbols.
                        witnesses.append(witness)
                    yield total
            return
        symbol = symbols[at]
        if symbol.isdigit():
            yield from search(at + 1, used, generic + int(symbol), reductions, covered, generic_indices + [at], chosen)
            return
        alternatives = symbol.split("/")
        if len(alternatives) == 2 and alternatives[0] == "2" and alternatives[1] in TYPES:
            alternatives = [alternatives[1], "2"]
        if any(a not in TYPES and a != "2" for a in alternatives):
            raise PaymentEvidenceError("locked symbol needs additional snow, variable or Phyrexian provenance")
        for choice in alternatives:
            if choice == "2":
                yield from search(at + 1, used, generic + 2, reductions, covered, generic_indices + [at], chosen)
                continue
            i = TYPES.index(choice)
            if reductions[i]:
                if at in allocation_by_symbol:
                    continue
                fewer = reductions.copy()
                fewer[i] -= 1
                yield from search(at + 1, used, generic, fewer, covered, generic_indices, chosen)
            else:
                if i < 5 and covered[i] and at not in allocation_by_symbol:
                    fewer = covered.copy()
                    fewer[i] -= 1
                    yield from search(at + 1, used, generic, reductions, fewer, generic_indices, chosen)
                # CR 702.51a does not require convoking the first matching symbol.
                # A selected mana payment can leave that creature for a later one.
                if used[choice] < upper[choice] and allocation_by_symbol.get(at, [choice]) == [choice]:
                    yield from search(at + 1, used + Counter({choice: 1}), generic, reductions, covered, generic_indices, chosen + [{"symbol": at, "mana": choice}])
    yield from search(0, Counter(), 0, typed_reduction, convoke, [], [])


def select_payment(pay, prior_pool, quote, *, restricted_lot=None, record_id=None):
    """Return explicit native TOTAL mana spending for a quoted MGN payment.

    The quote retains the exact announcement at its unpaid locked boundary;
    earlier child payments already have explicit selections. Re-quote the
    returned explicit move and send that quote as act's
    payment_expect; a changed or incomplete quote is rejected atomically.
    """
    receipts = quote.get("payments", [])
    if not receipts or not isinstance(receipts[-1].get("requirement"), dict):
        raise PaymentEvidenceError("payment quote did not reach a locked mana requirement")
    _nested_plan_inputs(pay, quote.get("move", {}), record_id)
    _validate_child_payments(pay, quote, restricted_lot, record_id)
    final = receipts[-1]
    boundary = final["pool"]
    prior_plain, _ = _prior_units(prior_pool, pay.get("pool", {}))
    available = _counts(boundary)
    prior_low, prior_high, surviving_restricted = _prior_survivors(prior_pool, quote)
    if any(prior_plain[c] > prior_high[c] for c in TYPES):
        raise InvalidPayment("MGN prior pool selection was consumed by a child payment")
    prior_restricted, now_restricted = _restricted(prior_pool), _restricted(boundary)
    prior_indices = _select_lots(prior_pool, pay.get("pool_restricted", []), restricted_lot)
    selected_restricted = Counter(_unit_identity(prior_restricted[i]) for i in prior_indices)
    if any(count > surviving_restricted[identity] for identity, count in selected_restricted.items()):
        raise InvalidPayment("MGN restricted prior pool selection was consumed by a child payment")
    generated_restricted = Counter(_unit_identity(unit) for unit in now_restricted) - surviving_restricted
    allocated_restricted = []
    plain_allocation = Counter()
    normalized_allocation = []
    for item in pay.get("allocation", []):
        if not isinstance(item, dict) or set(item) - {"symbol", "mana", "restricted"}:
            raise PaymentEvidenceError("malformed printed-symbol allocation")
        if "restricted" in item:
            selector = item["restricted"]
            if not isinstance(selector, dict) or selector.get("type") != item.get("mana") or "count" in selector:
                raise PaymentEvidenceError("allocation restricted lot type must match its mana type and select one unit")
            allocated_restricted.append({**selector, "count": 1})
        else:
            plain_allocation[item.get("mana")] += 1
        normalized_allocation.append({k: v for k, v in item.items() if k != "restricted"})
    allocated_indices = _select_lots(boundary, allocated_restricted, restricted_lot)
    required_restricted = Counter(_unit_identity(now_restricted[i]) for i in allocated_indices)
    for identity, count in required_restricted.items():
        extra = max(0, count - selected_restricted[identity])
        if extra > generated_restricted[identity]:
            raise PaymentEvidenceError("allocation selects restricted prior mana absent from pool_restricted")
        selected_restricted[identity] += extra
    restricted_counts = Counter()
    for identity, count in selected_restricted.items():
        restricted_counts[json.loads(identity)["mana"]] += count
    lower = prior_plain + restricted_counts
    upper = restricted_counts + Counter({c: min(available[c], prior_plain[c] + available[c] - prior_low[c]) for c in TYPES})
    allocation = normalized_allocation
    if allocation:
        indices = quote.get("symbol_indices")
        if not isinstance(indices, list) or len(indices) != len(final["requirement"]["symbols"]):
            raise PaymentEvidenceError("printed-symbol allocation needs retained symbol provenance")
        mapped = []
        for item in allocation:
            if item.get("symbol") not in indices:
                raise PaymentEvidenceError("allocation names a symbol paid with life or absent from the locked cost")
            mapped.append({**item, "symbol": indices.index(item["symbol"])})
        allocation = mapped
    solutions = {}
    for total in _spendings(final["requirement"], lower, upper, allocation):
        plain = total - restricted_counts
        if any(plain[c] < plain_allocation[c] for c in TYPES):
            continue
        key = tuple(total[c] for c in TYPES)
        solutions[key] = total
        if len(solutions) > 1:
            raise PaymentEvidenceError("MGN payment leaves generated mana spending ambiguous; provide per-symbol allocation")
    if not solutions:
        raise InvalidPayment("MGN prior-pool and per-symbol selections do not pay the locked cost")
    total = next(iter(solutions.values())) - restricted_counts
    native = {c: total[c] for c in TYPES if total[c]}
    needs = selected_restricted.copy()
    indices = []
    for index, unit in enumerate(now_restricted):
        identity = _unit_identity(unit)
        if needs[identity]:
            indices.append(index)
            needs[identity] -= 1
    if any(needs.values()):
        raise PaymentEvidenceError("selected semantic restricted units are absent from the payment boundary")
    if indices:
        native["restricted_after" if now_restricted[:len(prior_restricted)] != prior_restricted
               or any(i >= len(prior_restricted) for i in indices) else "restricted"] = indices
    return native


def validate_production(pay, quote):
    if "productions" not in quote:
        return _legacy_production(pay, quote)
    nodes = dict(payment_nodes(quote.get("move", {})))
    authored = {}
    def walk(component, path):
        authored[path] = component
        for i, step in enumerate(component.get("abilities", [])):
            walk(step.get("pay", {}), (*path, i))
    walk(pay, ())
    if set(authored) != set(nodes):
        raise PaymentEvidenceError("native payment scopes disagree with the MGN activation tree")
    groups = {path: [] for path in nodes}
    captured = []
    sequences = []
    for row in quote["productions"]:
        path = row.get("path")
        if not isinstance(path, list) or any(type(i) is not int or i < 0 for i in path) or tuple(path) not in nodes or type(row.get("triggered")) is not bool:
            raise PaymentEvidenceError("mana production lacks its explicit execution scope or trigger classification")
        path = tuple(path); node = nodes[path]
        if (not isinstance(row.get("invocation"), dict)
                or row["invocation"].get("source") != row.get("source")
                or type(row["invocation"].get("declaration_index")) is not int
                or type(row.get("sequence")) is not int or type(row.get("player")) is not int
                or not isinstance(row.get("mana"), list) or any(m not in TYPES for m in row["mana"])):
            raise PaymentEvidenceError("mana production receipt lacks its actual producing invocation")
        if not row["triggered"]:
            if row.get("source") != node.get("source") or row.get("activation") != node.get("ability"):
                raise PaymentEvidenceError("mana production receipt names another activation invocation")
            groups[path].extend(row["mana"])
        # Triggered production is an automatic consequence, owned by its own
        # invocation. It participates in exact event/pool conservation below,
        # never in the enabling activation's authored `adding` output.
        sequences.append(row["sequence"])
        captured.append({"event": "mana_added", "source": row["source"], "player": row["player"], "mana": row["mana"]})
    if sequences != sorted(set(sequences)):
        raise PaymentEvidenceError("mana production invocation sequence is duplicated or reordered")
    actual = [e for e in quote.get("events", []) if e.get("event") == "mana_added"]
    if captured != actual:
        raise PaymentEvidenceError("production receipts disagree with the captured mana events")
    partial = payment_suspension(quote)
    executed = {tuple(r["path"]) for r in quote.get("payments", [])}
    for path in nodes:
        if not path or (partial is not None and partial.get("stage") == "mana_choice" and path not in executed):
            continue
        parent, index = path[:-1], path[-1]
        expected = authored[parent]["abilities"][index]["adding"]
        if Counter(groups[path]) != Counter(expected):
            raise PaymentEvidenceError(f"quoted mana production disagrees with MGN activation at {list(path)}")
    return {"plan": [groups[(i,)] for i in range(len(pay.get("abilities", [])))],
            "main": groups[()], "scopes": groups}


def _legacy_production(pay, quote):
    """Attribute every production to a declared plan activation or the main one.

    Triggered/unattributed production needs its own representation. It must not
    silently enlarge the resources the MGN payment is allowed to consume.
    Returns separate per-plan and post-payment main-ability production.
    """
    expected = [step["adding"] for step in pay.get("abilities", [])]
    events = quote.get("events", [])
    move = quote.get("move")
    if not isinstance(move, dict):
        actual = [event["mana"] for event in events if event.get("event") == "mana_added"]
        if expected != actual:
            raise PaymentEvidenceError("quoted mana production disagrees with the MGN payment abilities or includes unattributed production")
        return {"plan": expected, "main": []}
    payment = move.get("answer", {}).get("pay", {}) if move.get("kind") == "choose" else move
    steps = payment.get("plan", [])
    if len(steps) != len(expected):
        raise PaymentEvidenceError("native payment plan disagrees with the MGN payment abilities")
    sources = [step["source"] for step in steps]
    main = move.get("source") if move.get("kind") == "activate" else None
    groups = [[] for _ in sources]
    productions = [0 for _ in sources]
    main_added = []
    main_productions = 0
    main_is_mana = False
    at = -1
    actor = (quote.get("payments") or [{}])[-1].get("player")
    for event in events:
        if event.get("event") == "activated":
            at += 1
            wanted = sources[at] if at < len(sources) else main
            if wanted is None or event.get("source") != wanted or at > len(sources):
                raise PaymentEvidenceError("activation events do not identify the ordered payment plan and main activation")
            if at < len(sources) and event.get("ability") is not None:
                raise PaymentEvidenceError("a payment-plan activation is not a mana ability")
            if at == len(sources):
                main_is_mana = event.get("ability") is None
        elif event.get("event") == "mana_added":
            wanted = sources[at] if 0 <= at < len(sources) else main if at == len(sources) else None
            if (wanted is None or event.get("source") != wanted
                    or (actor is not None and event.get("player") != actor)):
                raise PaymentEvidenceError("additional mana production has no declared activation provenance")
            if at == len(sources):
                if not main_is_mana:
                    raise PaymentEvidenceError("main activation production is not authenticated as a mana ability")
                main_productions += 1
                if main_productions > 1:
                    raise PaymentEvidenceError("multiple main production instructions lack direct-versus-trigger invocation provenance")
                main_added.extend(event["mana"])
            else:
                productions[at] += 1
                if productions[at] > 1:
                    raise PaymentEvidenceError("multiple production instructions lack direct-versus-trigger invocation provenance")
                groups[at].extend(event["mana"])
    if [Counter(group) for group in groups] != [Counter(group) for group in expected] or at + 1 < len(sources):
        raise PaymentEvidenceError("quoted mana production disagrees with the MGN payment abilities")
    return {"plan": groups, "main": main_added}


def _resources(pool):
    out = Counter({("plain", c): n for c, n in _counts(pool).items() if n})
    for r in _restricted(pool):
        out[("restricted", json.dumps(r, sort_keys=True, separators=(",", ":")))] += 1
    return out


def _resource_types(resources):
    out = Counter()
    for (kind, identity), count in resources.items():
        out[identity if kind == "plain" else json.loads(identity)["mana"]] += count
    return out


def _added_between(before, after, production, source):
    old, new = _resources(before), _resources(after)
    if any(new[key] < count for key, count in old.items()):
        raise PaymentEvidenceError("payment receipt boundary consumed mana outside its declared payment")
    added = new - old
    if _resource_types(added) != Counter(production):
        raise PaymentEvidenceError("payment receipt boundary disagrees with captured production conservation")
    if any(json.loads(identity).get("source") != source for (kind, identity) in added if kind == "restricted"):
        raise PaymentEvidenceError("generated restricted mana has no matching declared producer")


def _validate_receipt_chain(quote, payment, player, production):
    if "productions" not in quote:
        return _legacy_receipt_chain(quote, payment, player, production)
    nodes = dict(payment_nodes(payment))
    receipts = {}
    ordered = []
    for receipt in quote.get("payments", []):
        path = receipt.get("path")
        if not isinstance(path, list) or any(type(i) is not int or i < 0 for i in path) or tuple(path) not in nodes or tuple(path) in receipts:
            raise PaymentEvidenceError("missing, duplicate or unauthored scoped payment receipt")
        path = tuple(path); receipts[path] = receipt; ordered.append(path)
    boundary = payment_suspension(quote)
    partial = boundary is not None and boundary.get("stage") == "mana_choice"
    expected_nodes = list(nodes)
    if partial:
        scope = tuple(boundary["scope"])
        if scope not in nodes or len(scope) != 1:
            raise PaymentEvidenceError("unfinished nested trigger needs its retained ancestor cost boundaries")
        expected_nodes = expected_nodes[:expected_nodes.index(scope) + 1]
    if ordered != expected_nodes:
        raise PaymentEvidenceError("payment receipts do not cover the authored execution tree exactly once")
    def visit(path, cursor):
        node, receipt = nodes[path], receipts[path]
        if receipt.get("player") != player or _resources(receipt["prior_pool"]) != _resources(cursor):
            raise PaymentEvidenceError("payment scope does not begin at its authenticated parent boundary")
        if path and (receipt.get("kind") != "ability" or receipt.get("source") != node.get("source")):
            raise PaymentEvidenceError("scoped child receipt names another activation")
        for i in range(len(node.get("plan", []))):
            cursor = visit((*path, i), cursor)
        if _resources(receipt["pool"]) != _resources(cursor):
            raise PaymentEvidenceError("locked scope pool disagrees with its completed child executions")
        spent = _spent_resources(receipt)
        selected = node.get("mana")
        if selected is None or (not path and type(selected) is bool): selected = {}
        if not isinstance(selected, dict) or set(selected) - set(TYPES) - {"restricted", "restricted_after"}:
            raise PaymentEvidenceError("malformed explicit native mana selector")
        if _selected_resources(selected, receipt["prior_pool"], receipt["pool"]) != spent:
            raise PaymentEvidenceError("scoped payment receipt differs from its exact native selector")
        cursor = receipt["left"]
        for row in quote["productions"]:
            if tuple(row.get("path") or ()) != path:
                continue
            if row.get("player") != player or _resources(row["before"]) != _resources(cursor):
                raise PaymentEvidenceError("production invocation does not start from its paid scope")
            _added_between(cursor, row["after"], row["mana"], row["source"])
            cursor = row["after"]
        return cursor
    if partial:
        result = quote["prior_pools"][player]
        for index in range(boundary["scope"][0] + 1):
            result = visit((index,), result)
    else:
        result = visit((), quote["prior_pools"][player])
    if _resources(result) != _resources(quote["pools"][player]):
        raise PaymentEvidenceError("scoped payment execution disagrees with the captured final pool")
    final = receipts.get(())
    kind = quote["move"].get("kind")
    if kind == "cast":
        casts = [e for e in quote.get("events", []) if e.get("event") == "cast"]
        suspended = payment_suspension(quote)
        if suspended is not None:
            if casts:
                raise PaymentEvidenceError("unfinished announcement already claims a cast event")
        elif len(casts) != 1 or final.get("kind") != "spell" or final.get("source") != casts[0].get("spell"):
            raise PaymentEvidenceError("main payment receipt does not name the captured cast")
    elif kind == "activate" and (final.get("kind") != "ability" or final.get("source") != payment.get("source")):
        raise PaymentEvidenceError("main payment receipt does not name the captured activation")
    elif kind == "choose" and final.get("kind") != "effect":
        raise PaymentEvidenceError("main payment receipt does not name a resolving effect payment")
    activations = [e for e in quote.get("events", []) if e.get("event") == "activated"]
    expected = [receipts[p]["source"] for p in receipts if p or kind == "activate"]
    if [e.get("source") for e in activations] != expected:
        raise PaymentEvidenceError("activation events disagree with scoped payment execution")
    if any(e.get("ability") is not None for e in activations[:len(nodes) - 1]):
        raise PaymentEvidenceError("a scoped payment activation is not a mana ability")


def _legacy_receipt_chain(quote, payment, player, production):
    receipts = quote.get("payments", [])
    steps = payment.get("plan", [])
    if len(receipts) != len(steps) + 1:
        raise PaymentEvidenceError("payment receipt attempts do not identify the complete declared plan")
    priors, following = quote["prior_pools"], quote["pools"]
    cursor = priors[player]
    for index, step in enumerate(steps):
        receipt = receipts[index]
        if receipt.get("kind") != "ability" or receipt.get("source") != step["source"]:
            raise PaymentEvidenceError("payment-plan receipt names another activation")
        if _resources(receipt["pool"]) != _resources(cursor):
            raise PaymentEvidenceError("payment-plan receipt does not start from the preceding boundary")
        _spent_resources(receipt)
        next_pool = receipts[index + 1]["pool"]
        _added_between(receipt["left"], next_pool, production["plan"][index], step["source"])
        cursor = next_pool
    final = receipts[-1]
    if _resources(final["pool"]) != _resources(cursor):
        raise PaymentEvidenceError("main payment receipt does not start from the captured prior pool")
    outer_move = quote["move"]
    kind = outer_move.get("kind")
    if kind == "cast":
        casts = [event for event in quote.get("events", []) if event.get("event") == "cast"]
        if len(casts) != 1 or final.get("kind") != "spell" or final.get("source") != casts[0].get("spell"):
            raise PaymentEvidenceError("main payment receipt does not name the captured cast")
    elif kind == "activate":
        if final.get("kind") != "ability" or final.get("source") != outer_move.get("source"):
            raise PaymentEvidenceError("main payment receipt does not name the captured activation")
    elif kind == "choose" and final.get("kind") != "effect":
        raise PaymentEvidenceError("main payment receipt does not name a resolving effect payment")
    main_source = outer_move.get("source") if kind == "activate" else None
    if final.get("left") is None:
        raise PaymentEvidenceError("payment_selection has no completed native spend")
    _added_between(final["left"], following[player], production["main"], main_source)



def validate_bound_payment(quote, move, player, production):
    """Check the entire paid receipt chain before submitting the live action."""
    if not payment_ready(quote) or quote.get("move") != move:
        raise PaymentEvidenceError("final payment quote does not authenticate the submitted action")
    payment = move.get("answer", {}).get("pay", {}) if move.get("kind") == "choose" else move
    _validate_receipt_chain(quote, payment, player, production)
    receipts = quote["payments"]
    nodes = dict(payment_nodes(payment))
    pairs = [(nodes[tuple(r["path"])], r) for r in receipts] if "productions" in quote else zip([*payment.get("plan", []), payment], receipts)
    for step, receipt in pairs:
        if receipt.get("player") != player:
            raise PaymentEvidenceError("payment receipt names another payer")
        selected = step.get("mana")
        if not isinstance(selected, dict):
            raise PaymentEvidenceError("payment quote lacks an explicit child or outer pool selection")
        prior = receipt.get("prior_pool", receipt["pool"]) if step is not payment else quote["prior_pools"][player]
        if _selected_resources(selected, prior, receipt["pool"]) != _spent_resources(receipt):
            raise PaymentEvidenceError("payment receipt differs from the submitted exact pool selection")


def _selected_resources(native, prior, boundary):
    out = _resources({c: native.get(c, 0) for c in TYPES})
    if "restricted" in native and "restricted_after" in native:
        raise PaymentEvidenceError("native payment names two restricted selectors")
    for field, pool in (("restricted", prior), ("restricted_after", boundary)):
        indices = native.get(field, [])
        resources = _restricted(pool)
        if not isinstance(indices, list) or any(type(i) is not int or not 0 <= i < len(resources) for i in indices):
            raise PaymentEvidenceError("native restricted selector has no captured resource identity")
        for i in indices:
            out.update(_resources({"restricted": [resources[i]]}))
    return out


def _pool_view(pool):
    """Compare the facts both privileged snapshots and richer quotes expose."""
    counts = _counts(pool)
    riders = []
    for r in _restricted(pool):
        kept = r.get("kept")
        kept = {"None": None, "Some(Turn)": "turn", "Some(Combat)": "combat"}.get(kept, kept)
        riders.append((r["mana"], r.get("source"), bool(r.get("uncounterable")), kept,
                       json.dumps(r.get("provenance"), sort_keys=True)))
    return tuple(counts[c] for c in TYPES), riders


def _validate_quote(quote, move, before_pool, after_pool, actual_events, player):
    """Bind either quote source to the exact captured action and payment frames."""
    if not isinstance(quote, dict) or not payment_ready(quote):
        raise PaymentEvidenceError("payment quote is not complete")
    return _validate_quote_frames(quote, move, before_pool, after_pool, actual_events, player)


def _validate_quote_frames(quote, move, before_pool, after_pool, actual_events, player):
    """Authenticate raw execution frames; continuation readiness is checked on joined slices."""
    quoted_move = quote.get("move")
    # Effect-payment conversion passes the inner pay object to this helper.
    if isinstance(quoted_move, dict) and quoted_move.get("kind") == "choose" and "kind" not in move:
        quoted_move = quoted_move.get("answer", {}).get("pay")
    if quoted_move != move:
        raise PaymentEvidenceError("payment quote does not bind this exact native move")
    priors, following = quote.get("prior_pools", []), quote.get("pools", [])
    if player >= len(priors) or player >= len(following):
        raise PaymentEvidenceError("payment quote lacks the payer's boundary pools")
    if _pool_view(priors[player]) != _pool_view(before_pool) or _pool_view(following[player]) != _pool_view(after_pool):
        raise PaymentEvidenceError("payment quote disagrees with captured before/after pools")
    relevant = {"mana_added", "activated", "cast"}
    claimed = [e for e in quote.get("events", []) if e.get("event") in relevant]
    actual = [e for e in actual_events if e.get("event") in relevant]
    if claimed != actual:
        raise PaymentEvidenceError("payment quote disagrees with captured payment events")
    boundary = payment_suspension(quote) if move.get("kind") == "cast" else None
    if boundary is not None:
        quoted = [event for event in quote.get("events", []) if event.get("event") == "zone"
                  and event.get("was") == move.get("card") and event.get("to") == "Stack"
                  and event.get("now") == boundary["source"]]
        captured = [event for event in actual_events if event.get("event") == "zone"
                    and (event.get("was") == move.get("card") or event.get("now") == boundary["source"])]
        if captured != quoted:
            raise PaymentEvidenceError("suspended payment disagrees with captured input-to-stack transition")
    if any(receipt.get("player") != player for receipt in quote.get("payments", [])):
        raise PaymentEvidenceError("payment receipt names another payer")


def restore_payment(selection, move, before_pool, after_pool, actual_events, reconstructed, player, *, restricted_lot=None, record_id=None):
    """Validate a fresh inverse frame's authored selection against its native proof.

    `payment_selection` is {pay: original MGN payment, quote: final quote_payment
    response}. The quote must name the exact native move, have completed spending,
    and agree with captured prior/following pools and payment events. A bare prior
    pool override is never sufficient. The recorder must retain the engine/version
    provenance of the quote alongside its original input/frames.
    """
    if not isinstance(selection, dict) or set(selection) != {"pay", "quote"}:
        raise PaymentEvidenceError("payment_selection needs original MGN pay and the authenticated native quote")
    pay, quote = selection["pay"], selection["quote"]
    if (not isinstance(pay, dict) or set(pay) - {"pool", "pool_restricted", "abilities", "objects", "life", "allocation"}
            or not isinstance(quote, dict) or not payment_ready(quote)):
        raise PaymentEvidenceError("payment_selection lacks a complete authenticated payment quote")
    _validate_quote(quote, move, before_pool, after_pool, actual_events, player)
    priors = quote["prior_pools"]
    production = validate_production(pay, quote)
    _validate_receipt_chain(quote, move, player, production)
    def activation_inputs(abilities):
        inputs = deepcopy(abilities)
        for ability in inputs:
            if "pay" in ability:
                if "abilities" in ability["pay"]:
                    ability["pay"]["abilities"] = activation_inputs(ability["pay"]["abilities"])
                for field in ("pool", "pool_restricted", "allocation"):
                    ability["pay"].pop(field, None)
                if not ability["pay"]:
                    ability.pop("pay")
        return inputs

    for key in ("abilities", "objects", "life"):
        authored, captured = pay.get(key, []), reconstructed.get(key, [])
        if key == "abilities":
            authored, captured = activation_inputs(authored), activation_inputs(captured)
        if authored != captured:
            raise PaymentEvidenceError(f"payment_selection {key} disagrees with the captured native payment")
    if (payment_suspension(quote) or {}).get("stage") == "mana_choice":
        if move.get("mana") is not None:
            raise PaymentEvidenceError("unpaid mana-choice root cannot claim an already completed selector")
        _prior_units(priors[player], pay.get("pool", {}))
        _select_lots(priors[player], pay.get("pool_restricted", []), restricted_lot)
        return deepcopy(pay)
    total = select_payment(pay, priors[player], quote, restricted_lot=restricted_lot, record_id=record_id)
    native = move.get("mana")
    if not isinstance(native, dict):
        raise PaymentEvidenceError("payment_selection requires explicit native total spending")
    _counts({k: v for k, v in native.items() if k != "restricted_after"})
    normalized = {c: native[c] for c in TYPES if native.get(c)}
    for key in ("restricted", "restricted_after"):
        if native.get(key):
            normalized[key] = native[key]
    if set(native) - set(TYPES) - {"restricted", "restricted_after"} or normalized != total:
        raise PaymentEvidenceError("payment_selection disagrees with explicit native total spending")
    receipts = quote.get("payments", [])
    if not receipts or receipts[-1].get("left") is None:
        raise PaymentEvidenceError("payment_selection has no completed native spend")
    final = receipts[-1]
    if final.get("selected") is None:
        raise PaymentEvidenceError("payment_selection quote did not execute explicit spending")
    if _resources(final["selected"]) != _selected_resources(native, priors[player], final["pool"]):
        raise PaymentEvidenceError("payment receipt selected resources disagree with the explicit native selector")
    selected = _counts(final["selected"])
    start, left = _counts(final["pool"]), _counts(final["left"])
    if any(start[c] - left[c] != selected[c] for c in TYPES):
        raise PaymentEvidenceError("payment_selection violates native spending conservation")
    identity = lambda r: repr(sorted(r.items()))
    restricted_start = Counter(identity(r) for r in _restricted(final["pool"]))
    restricted_left = Counter(identity(r) for r in _restricted(final["left"]))
    restricted_selected = Counter(identity(r) for r in _restricted(final["selected"]))
    if restricted_start != restricted_left + restricted_selected:
        raise PaymentEvidenceError("payment_selection violates restricted-mana conservation")
    # Preserve the authored prior/new provenance only after all checks above.
    return deepcopy(pay)


def invert_payment_quote(quote, move, before_pool, after_pool, actual_events, reconstructed, player, *, restricted_lot=None, record_id=None):
    """Project a captured native payment into MGN without inventing lot origins.

    CR 106.4 unrestricted units of one type are fungible. Native captures do not
    claim an authored old/new choice: use prior units first, then prove the forward
    payment has the same exact spend and remaining pool. CR 106.6 restricted units
    retain production-time semantic lot provenance, including consumed generated units.
    """
    _validate_quote(quote, move, before_pool, after_pool, actual_events, player)
    if (payment_suspension(quote) or {}).get("stage") == "mana_choice":
        raise PaymentEvidenceError("unfinished root needs retained authored payment")
    pay = deepcopy(reconstructed)
    production = validate_production(pay, quote)
    _validate_receipt_chain(quote, move, player, production)
    receipts = quote["payments"]
    for pool in [quote["prior_pools"][player], quote["pools"][player],
                 *(r[field] for r in receipts for field in ("pool", "left"))]:
        for unit in _restricted(pool):
            _lot_fact(unit, restricted_lot)
    if "productions" in quote:
        def invert_children(component, scope):
            for i, ability in enumerate(component.get("abilities", [])):
                child = scoped_quote(scope, (i,))
                nested = deepcopy(ability.get("pay", {}))
                invert_children(nested, child)
                final = child["payments"][-1]
                nested = _invert_locked_payment(nested, final["prior_pool"], child, restricted_lot=restricted_lot, record_id=record_id)
                if nested: ability["pay"] = nested
        invert_children(pay, quote)
    else:
        for index, receipt in enumerate(receipts[:-1]):
            if _spent_resources(receipt):
                nested = pay["abilities"][index].get("pay", {})
                pay["abilities"][index]["pay"] = _invert_locked_payment(nested, receipt["pool"], _child_quote(receipt), restricted_lot=restricted_lot, record_id=record_id)
    return _invert_locked_payment(pay, quote["prior_pools"][player], quote,
        restricted_lot=restricted_lot, record_id=record_id)


def _invert_locked_payment(pay, prior_pool, quote, *, restricted_lot=None, record_id=None):
    pay = deepcopy(pay)
    receipts = quote["payments"]
    final = receipts[-1]
    boundary, left = _counts(final["pool"]), _counts(final["left"])
    if any(left[c] > boundary[c] for c in TYPES):
        raise PaymentEvidenceError("native payment quote violates spending conservation")
    spent_plain = boundary - left
    resources = _resources(final["pool"])
    remaining = _resources(final["left"])
    if any(remaining[key] > resources[key] for key in remaining):
        raise PaymentEvidenceError("native payment quote violates restricted spending conservation")
    spent_resources = resources - remaining
    spent = _resource_types(spent_resources)
    restricted_spent = []
    for (kind, identity), count in spent_resources.items():
        if kind == "restricted":
            restricted_spent.extend([json.loads(identity)] * count)
    if final.get("selected") is not None and _resources(final["selected"]) != spent_resources:
        raise PaymentEvidenceError("payment receipt selected resources disagree with spending conservation")
    native = quote.get("move", {}).get("mana")
    # Legal::Activate's Boolean mana field classifies the ability. Native wire
    # parsing consumes an explicit selector only when this field is an object.
    if type(native) is bool:
        native = None
    if native is not None and (not isinstance(native, dict) or set(native) - set(TYPES) - {"restricted", "restricted_after"}):
        raise PaymentEvidenceError("malformed explicit native mana selector")
    if native is not None and _selected_resources(native, prior_pool, final["pool"]) != spent_resources:
        raise PaymentEvidenceError("payment receipt disagrees with explicit native selector")
    _, before, surviving_restricted = _prior_survivors(prior_pool, quote)
    prior = {c: min(before[c], spent_plain[c]) for c in TYPES if min(before[c], spent_plain[c])}
    if prior:
        pay["pool"] = prior
    witnesses = []
    if next(_spendings(final.get("requirement", {}), spent, spent, [], witnesses=witnesses), None) is None:
        raise PaymentEvidenceError("native payment quote spending does not pay the locked cost")
    prior_units = surviving_restricted.copy()
    lots = {}
    for unit in restricted_spent:
        fact = _lot_fact(unit, restricted_lot)
        identity = _unit_identity(unit)
        if prior_units[identity]:
            prior_units[identity] -= 1
            key = json.dumps(fact, sort_keys=True)
            if key not in lots:
                lots[key] = {**fact, "count": 0}
            lots[key]["count"] += 1
    if lots:
        pay["pool_restricted"] = list(lots.values())

    def allocate():
        indices = quote.get("symbol_indices")
        witness = next((w for w in witnesses if w is not None), None)
        if not isinstance(indices, list) or len(indices) != len(final["requirement"]["symbols"]) or witness is None:
            raise PaymentEvidenceError("native spending needs retained printed-symbol allocation provenance")
        pending = deepcopy(restricted_spent)
        allocation = []
        for item in witness:
            entry = {**item, "symbol": indices[item["symbol"]]}
            unit = next((r for r in pending if r["mana"] == item["mana"]), None)
            if unit is not None:
                entry["restricted"] = _lot_fact(unit, restricted_lot)
                pending.remove(unit)
            allocation.append(entry)
        if pending:
            raise PaymentEvidenceError("restricted spending lacks a symbol allocation witness")
        pay["allocation"] = allocation

    if restricted_spent:
        allocate()
    try:
        total = select_payment(pay, prior_pool, quote, restricted_lot=restricted_lot, record_id=record_id)
    except PaymentEvidenceError as error:
        if "ambiguous" not in str(error):
            raise
        allocate()
        total = select_payment(pay, prior_pool, quote, restricted_lot=restricted_lot, record_id=record_id)
    if _selected_resources(total, prior_pool, final["pool"]) != spent_resources:
        raise PaymentEvidenceError("inverse payment does not reproduce the captured native spending")
    return pay


def continuation_payment(prefix, answer_quote):
    """Join two preserved execution slices for validation, not a replacement wire quote.

    The first slice ends at the actual triggered-mana choice. The second starts
    there and reaches the retained root's locked spend. Raw inputs/quotes remain
    separate in the capture; sequence rebasing here only makes their chronology
    explicit for the existing resource validator.
    """
    boundary = payment_suspension(prefix)
    if boundary is None or boundary.get("stage") != "mana_choice":
        raise PaymentEvidenceError("continuation lacks its authenticated initial mana-choice boundary")
    keys = ("stage", "path", "kind", "source", "player", "scope", "trigger", "pool")
    if answer_quote.get("continuation") != {k: boundary[k] for k in keys}:
        raise PaymentEvidenceError("continuation names another pending root, trigger, scope or pool")
    if answer_quote.get("error") is not None:
        raise PaymentEvidenceError("continuation quote refused its actual mana answer")
    if [_pool_view(p) for p in answer_quote.get("prior_pools", [])] != [_pool_view(p) for p in prefix["pools"]]:
        raise PaymentEvidenceError("continuation does not start at the captured unpaid pool")
    answer = (answer_quote.get("move") or {}).get("answer", {})
    rows = answer_quote.get("productions", [])
    if (not isinstance(answer.get("mana"), list) or not rows
            or rows[0].get("source") != boundary["trigger"]["source"]
            or rows[0].get("invocation") != boundary["trigger"] or rows[0].get("mana") != answer["mana"]
            ):
        raise PaymentEvidenceError("continuation output does not belong to the answered trigger invocation")
    receipts = answer_quote.get("payments", [])
    next_boundary = answer_quote.get("suspended")
    if next_boundary is None:
        if (not receipts or receipts[-1].get("path") != [] or receipts[-1].get("kind") != "spell"
                or receipts[-1].get("source") != boundary["source"] or receipts[-1].get("player") != boundary["player"]):
            raise PaymentEvidenceError("continuation did not reach its retained root cost")
    elif (next_boundary.get("stage") != "mana_choice" or any(r.get("path") == [] for r in receipts)
            or any(next_boundary.get(k) != boundary.get(k) for k in ("source", "path", "kind", "player", "announced"))):
        raise PaymentEvidenceError("intermediate answer changed its unfinished root announcement")
    joined = deepcopy(prefix)
    joined.pop("suspended", None)
    if next_boundary is not None:
        joined["suspended"] = deepcopy(next_boundary)
    joined.update(complete=answer_quote.get("complete"), error=answer_quote.get("error"),
                  symbol_indices=answer_quote.get("symbol_indices"), pools=deepcopy(answer_quote["pools"]))
    joined["events"].extend(deepcopy(answer_quote["events"]))
    joined["payments"].extend(deepcopy(receipts))
    offset = max((r["sequence"] for r in joined["productions"]), default=-1) + 1
    for row in rows:
        captured = deepcopy(row)
        captured["sequence"] += offset
        joined["productions"].append(captured)
    if next_boundary is not None:
        payment_suspension(joined)
    return joined


def continuation_selector(selected, prior, boundary):
    """Resolve selected restricted identities at the actual post-answer pool."""
    resources = _selected_resources(selected, prior, boundary)
    out = {c: selected[c] for c in TYPES if selected.get(c)}
    needed = Counter({identity: n for (kind, identity), n in resources.items() if kind == "restricted"})
    indices = []
    for i, unit in enumerate(_restricted(boundary)):
        identity = _unit_identity(unit)
        if needed[identity]:
            indices.append(i)
            needed[identity] -= 1
    if any(needed.values()):
        raise PaymentEvidenceError("pending selector lacks its exact post-production restricted units")
    if indices:
        out["restricted_after"] = indices
    return out
