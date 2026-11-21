"""Whole-card engine support and rule-test coverage."""
from collections import defaultdict
import json
import hashlib
import math
from pathlib import Path

import deck_archive
import rule_evidence

# Relationships from the pinned Oracle text. A meld result is not another physical card;
# its text must also be handled for either component to receive whole-card credit.
DISPLAY_FORMATS = ('pauper', 'standard', 'modern', 'legacy', 'vintage', 'others')

MELD_RESULTS = {
    'Brisela, Voice of Nightmares': ['Bruna, the Fading Light', 'Gisela, the Broken Blade'],
    'Chittering Host': ['Graf Rats', 'Midnight Scavengers'],
    'Hanweir, the Writhing Township': ['Hanweir Battlements', 'Hanweir Garrison'],
    'Mishra, Lost to Phyrexia': ['Mishra, Claimed by Gix', 'Phyrexian Dragon Engine'],
    'Titania, Gaea Incarnate': ['Argoth, Sanctum of Nature', 'Titania, Voice of Gaea'],
    'Urza, Planeswalker': ['Urza, Lord Protector', 'The Mightstone and Weakstone'],
    'Ragnarok, Divine Deliverance': ["Fang, Fearless l'Cie", "Vanille, Cheerful l'Cie"],
}


def whole_cards(inventory, corpus_path):
    faces = [json.loads(line) for line in corpus_path.read_text().splitlines() if line.strip()]
    expected = {(f['oracle_id'], f.get('face', 0)) for f in faces}
    if expected != inventory.keys() or len(expected) != len(faces):
        raise ValueError('card corpus and admission inventory differ')
    groups = defaultdict(list)
    for f in faces:
        stage = inventory[f['oracle_id'], f.get('face', 0)]['stage']
        if stage not in {'parse', 'engine', 'admitted'}:
            raise ValueError(f'unknown card stage: {stage}')
        groups[f['oracle_id']].append(f)
    by_name = {parts[0]['name']: identity for identity, parts in groups.items()}
    extra = defaultdict(list)
    for result, components in MELD_RESULTS.items():
        if result not in by_name:
            continue
        for component in components:
            if component not in by_name:
                raise ValueError(f'missing physical meld component: {component}')
            extra[by_name[component]].extend(groups[by_name[result]])
    cards = {}
    for identity, parts in groups.items():
        name = parts[0]['name']
        if name in MELD_RESULTS:
            continue
        # Refuse unknown meld-result relationships rather than silently count a result.
        if any(f.get('layout') == 'meld' and not f.get('mana_cost') and 'Land' not in f['type_line'] for f in parts):
            raise ValueError(f'unmapped meld result: {name}')
        needed = parts + extra[identity]
        stages = [inventory[f['oracle_id'], f.get('face', 0)]['stage'] for f in needed]
        cards[identity] = {'name': name, 'parsed': all(s != 'parse' for s in stages),
                           'supported': all(s == 'admitted' for s in stages)}
    return cards


def rule_test_coverage(ledger):
    """Count actual passing behavioral tests.

    Keep last-run coverage visible after source changes and identify it as historical.
    Do not turn test references alone, ignored tests or failed tests into passing evidence.
    """
    run = ledger.get('execution')
    if not run:
        return {'tested': 0, 'status': 'NOT RUN', 'revision': None, 'passing_tests': 0}
    passed = {name for name, outcomes in run.get('tests', {}).items()
              if outcomes and all(outcome == 'ok' for outcome in outcomes)}
    tested = sum(any(test['test'] in passed for test in entry.get('tests', []))
                 for entry in ledger['rules'].values())
    status = 'CURRENT' if ledger.get('execution_current') else 'LAST RUN'
    if run.get('exit_code') != 0:
        status += ' (FAILURES)'
    return {'tested': tested, 'status': status,
            'revision': run.get('revision', {}).get('commit'), 'passing_tests': len(passed)}


def deck_progress(rows, cards):
    by_name = {card['name']: identity for identity, card in cards.items()}
    entries = []
    for row in rows:
        names = deck_archive.deck_cards(row)
        if not names:
            raise ValueError(f"empty selected deck: {row['id']}")
        main_names = deck_archive.deck_cards({**row, 'sideboard': {}})
        supported = sum(cards[by_name[n]]['supported'] for n in names if n in by_name)
        entries.append({'id': row['id'], 'format': row['format'], 'total_cards': len(names),
                        'percent': 100 * supported / len(names),
                        'supported_cards': supported, 'supported_percent': 100 * supported / len(names),
                        'unresolved_names': sorted(names - by_name.keys()),
                        'main_fully_read': main_names <= by_name.keys()
                            and all(cards[by_name[n]]['parsed'] for n in main_names),
                        'main_fully_admitted': main_names <= by_name.keys()
                            and all(cards[by_name[n]]['supported'] for n in main_names),
                        'fully_admitted': supported == len(names),
                        'requires_source_review': bool(row.get('source_issues') or row.get('names_absent_from_pinned_corpus')
                            or row.get('text_export_check', {}).get('status') in {'different', 'unreadable'}),
                        'game_recording': row.get('game_recording')})
    def aggregate(part):
        return {'decks': len(part), 'percent': sum(e['percent'] for e in part) / len(part) if part else None,
                'supported_percent': sum(e['supported_percent'] for e in part) / len(part) if part else None,
                'main_fully_read': sum(e['main_fully_read'] for e in part),
                'main_fully_admitted': sum(e['main_fully_admitted'] for e in part),
                'fully_admitted': sum(e['fully_admitted'] for e in part),
                'source_review_required': sum(e['requires_source_review'] for e in part)}
    return {'overall': aggregate(entries),
            'by_format': {fmt: aggregate([e for e in entries if
                              (e['format'] if e['format'] in DISPLAY_FORMATS[:-1] else 'others') == fmt])
                          for fmt in DISPLAY_FORMATS}, 'entries': entries,
            'meaning': 'Equal-weight mean of deck support fractions; distinct whole cards in main/sideboard '
                       'and designations count once per deck. Not a completed-game or deck-correctness count.'}


def legal_pool(cards, inventory, legal_path, measurement_current=True, blockers=12):
    """Whole-card admission over a pinned format-legal pool (`resources/oracle/legal-*.jsonl`).

    The pool is a subset of the corpus by Oracle identity; a listed card the corpus lacks is an
    error, never silently dropped from the denominator. Beside the counts, the first blocker of
    each refused card (the reason of its first non-admitted face: a censored diagnostic in the
    engine's own terms) is tallied so that the next concept to implement is visible.
    """
    pool = [json.loads(line) for line in Path(legal_path).read_text().splitlines() if line.strip()]
    ids = [row['oracle_id'] for row in pool]
    if len(set(ids)) != len(ids):
        raise ValueError(f'duplicate cards in the pool {legal_path}')
    missing = [row['name'] for row in pool if row['oracle_id'] not in cards]
    if missing:
        raise ValueError(f'{len(missing)} pool cards are not in the corpus: {", ".join(missing[:5])}')
    result = {'format': Path(legal_path).stem.removeprefix('legal-'), 'total': len(pool), 'source': str(legal_path),
              'supported': None, 'parsed': None, 'refused': None, 'first_blockers': [],
              'meaning': 'Cards legal in the format per the pinned Scryfall bulk, admitted whole; '
                         'first blockers are censored diagnostics, one per refused card.'}
    result['pool_sha256'] = hashlib.sha256(Path(legal_path).read_bytes()).hexdigest()
    manifest_path = Path(legal_path).parent.parent / 'manifest.json'
    result['effective_at'] = None
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text())
        info = manifest.get('files', {}).get(f'oracle/{Path(legal_path).name}', {})
        if info.get('sha256') != result['pool_sha256']:
            raise ValueError('legal pool does not match its provenance manifest')
        provenance = manifest.get('sources', {}).get('legal_' + result['format'], {})
        result['bulk_updated_at'] = provenance.get('updated_at')
        result['effective_at'] = provenance.get('scope', {}).get('effective_at')
    if not measurement_current:
        return result
    faces = defaultdict(list)
    for (oracle_id, face), row in inventory.items():
        faces[oracle_id].append((face, row))
    refused, by_reason, examples = {'parse': 0, 'engine': 0}, defaultdict(int), defaultdict(list)
    for oracle_id in ids:
        if cards[oracle_id]['supported']:
            continue
        face, row = next((f, r) for f, r in sorted(faces[oracle_id], key=lambda x: x[0]) if r['stage'] != 'admitted')
        refused[row['stage']] += 1
        reason = ('not read: ' if row['stage'] == 'parse' else '') + (row.get('reason') or 'unstated')
        by_reason[reason] += 1
        if len(examples[reason]) < 3:
            examples[reason].append(cards[oracle_id]['name'])
    result.update(supported=sum(cards[i]['supported'] for i in ids), parsed=sum(cards[i]['parsed'] for i in ids),
                  refused=refused, distinct_blockers=len(by_reason),
                  first_blockers=[{'reason': reason, 'cards': count, 'examples': ', '.join(examples[reason])}
                                  for reason, count in sorted(by_reason.items(), key=lambda kv: (-kv[1], kv[0]))[:blockers]])
    return result


def dashboard(inventory, corpus_path, selected_path, ledger, measurement_current=True, legal_path=None):
    cards = whole_cards(inventory, corpus_path)
    rows = deck_archive.load(selected_path)
    decks = deck_progress(rows, cards)
    total = len(cards)
    parsed = sum(c['parsed'] for c in cards.values())
    supported = sum(c['supported'] for c in cards.values())
    result = {'cards': {'total': total, 'parsed': parsed if measurement_current else None,
                        'supported': supported if measurement_current else None},
              'rules': {'total': ledger['metrics']['total'],
                        'assessments': dict(ledger['metrics'].get('statuses', {})),
                        'assessment_meaning': 'Recorded assessments, not inferred from admission or test passes. Unreviewed means unassessed, not missing.',
                        **rule_test_coverage(ledger),
                        'verified': ledger['metrics']['verified'] if ledger['execution_current'] else None},
              'decks': decks,
              'denominators': {'cards': str(corpus_path), 'decks': str(selected_path)},
              'units': {'cards': 'distinct physical cards, every required face',
                        'decks': 'selected archive lists, equal weight; main + sideboard + designations',
                        'rules': 'numbered rule IDs cited by passing tests, not full-rule correctness'},
              'measurement_current': measurement_current}
    if legal_path is not None:
        result['legal'] = legal_pool(cards, inventory, legal_path, measurement_current)
    if not measurement_current:
        result['rules']['verified'] = None
        if ledger.get('execution'):
            result['rules']['status'] = 'LAST RUN' + (' (FAILURES)' if ledger['execution'].get('exit_code', 0) != 0 else '')
        else:
            result['rules']['status'] = 'NOT RUN'
        for group in [decks['overall'], *decks['by_format'].values()]:
            group['percent'] = group['supported_percent'] = None
            group['main_fully_read'] = group['main_fully_admitted'] = group['fully_admitted'] = None
        for entry in decks['entries']:
            entry['percent'] = entry['supported_percent'] = entry['supported_cards'] = None
            entry['main_fully_read'] = entry['main_fully_admitted'] = entry['fully_admitted'] = None
    return result


def render(data):
    value = data['dashboard']
    cards, rule = value['cards'], value['rules']
    def ratio(count, total):
        return 'UNKNOWN' if count is None else f'{(10000 * count // total) / 100:.2f}%  ({count:,}/{total:,})' if total else 'n/a'
    lines = ['Magic engine progress', '',
             f"  Whole cards parsed                  {ratio(cards['parsed'], cards['total'])}",
             f"  Whole cards supported               {ratio(cards['supported'], cards['total'])}",
             *(rule_evidence.render(rule['attributed']) if 'attributed' in rule else []),
             f"  Legacy: rules named by passing tests {'NOT RUN' if rule['status'] == 'NOT RUN' else ratio(rule['tested'], rule['total'])}  (whole tests, secondary)",
             '', 'Selected archive decks — average card admission', '',
             f"  {'Format':14} {'Decks':>6}  {'Average':>9}  {'Main admitted':>13}  {'Whole admitted':>14}"]
    for fmt, group in [('ALL', value['decks']['overall']), *value['decks']['by_format'].items()]:
        progress = 'n/a' if not group['decks'] else 'UNKNOWN' if group['percent'] is None else f"{math.floor(group['percent'] * 100) / 100:.2f}%"
        count = lambda key: '?' if group.get(key) is None else str(group[key])
        lines.append(f"  {fmt:14} {group['decks']:6,}  {progress:>9}  "
                     f"{count('main_fully_admitted'):>13}  {count('fully_admitted'):>14}")
    if 'decks' in data:
        local = data['decks']
        admitted = lambda key: '?' if not value['measurement_current'] else str(local[key])
        lines += ['', f"Local decks tree: {local['main_decks_total']} lists; "
                  f"{admitted('main_decks_admitted')} main lists admitted, "
                  f"{admitted('whole_decks_admitted')} whole lists admitted (including sideboards)."]
    lines += ['', 'Cards count once; all parts must pass. Main includes commander/companion designations.',
              'Whole lists also include sideboards. Admission does not verify format setup or game behavior.']
    lines.append('Supported = complete definitions pass engine admission checks. Legacy rule counts are test names and // Rules: comments; '
                 'attributed counts are per expectation. Neither is complete-rule correctness.')
    revision = (rule.get('revision') or 'unrecorded')[:12]
    lines.append(f"Rule tests: {rule['status']} ({revision}); {rule['passing_tests']:,} passing test names.")
    assessments = rule.get('assessments', {})
    if assessments:
        lines.append('Rule assessments: ' + ', '.join(f'{name} {n:,}' for name, n in sorted(assessments.items())) + '.')
        lines.append('These are recorded assessments; unreviewed means unassessed, not missing behavior.')
    if rule['status'].startswith('LAST RUN'):
        lines.append('Engine/test sources changed since that run; coverage shown is last-run evidence.')
    if not value['measurement_current']:
        lines.append('Inputs changed during measurement; rerun make progress.')
    if revision := data.get('revision'):
        dirty = ' (dirty)' if revision.get('worktree') else ''
        lines.append(f"Report checkout: {revision['commit'][:12]}{dirty}.")
    lines.append('Details: make progress PROGRESS_ARGS=--details')
    return '\n'.join(lines)
