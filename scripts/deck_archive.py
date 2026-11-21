#!/usr/bin/env python3
"""Deck lists in the roster's format: reading, name normalization, export and quantity-aware
overlap reports. The default input is the pinned roster."""
import argparse
from collections import Counter, defaultdict
from html import unescape
import json
import heapq
import hashlib
from pathlib import Path
import re
import unicodedata

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'resources/decks/selected-decks.jsonl'


def spelling(name):
    text = unicodedata.normalize('NFKD', name).casefold().replace('æ', 'ae').replace('œ', 'oe')
    return ''.join(c for c in text if c.isalnum())


class Names:
    def __init__(self, path=ROOT / 'resources/oracle/cards.jsonl'):
        self.names = defaultdict(set)
        self.basic = set()
        self.exact = set()
        for line in path.read_text().splitlines():
            row = json.loads(line)
            if row.get('face', 0) != 0:
                continue
            name = row['name']
            self.exact.add(name)
            for alias in (name, row.get('face_name', name)):
                self.names[spelling(alias)].add(name)
            if 'Basic' in row.get('type_line', ''):
                self.basic.add(name)

    def normalize(self, parts):
        result, changes, unknown = {}, [], set()
        for part, cards in parts.items():
            out = Counter()
            for name, quantity in cards.items():
                if not isinstance(quantity, int) or isinstance(quantity, bool) or quantity <= 0:
                    raise ValueError(f'invalid quantity: {name}: {quantity}')
                alias = unescape(re.sub(r'</?i>', '', name))
                annotation = re.fullmatch(r'(.+) \[[^\[\]]+\]', name)
                if annotation and annotation[1] in self.exact:
                    alias = annotation[1]
                choices = self.names.get(spelling(alias), set())
                if len(choices) != 1:
                    # Some publishers repeat a full name beside its front-name alias.
                    # Collapse only when both sides identify the same physical card.
                    for split in re.finditer(r' // ', alias):
                        shared = self.names.get(spelling(alias[:split.start()]), set()) & self.names.get(
                            spelling(alias[split.end():]), set())
                        if len(shared) == 1:
                            choices = shared
                            break
                canonical = name if name in self.exact or len(choices) != 1 else next(iter(choices))
                if canonical not in self.exact:
                    unknown.add(name)
                if canonical != name:
                    changes.append({'source': name, 'canonical': canonical})
                out[canonical] += quantity
            result[part] = dict(sorted(out.items()))
        return result, changes, sorted(unknown)


def load(path):
    body = path.read_text()
    rows = json.loads(body)['decks'] if path.suffix == '.json' else [
        json.loads(line) for line in body.splitlines() if line.strip()]
    if len({r['id'] for r in rows}) != len(rows):
        raise ValueError('duplicate archive IDs')
    return rows


def merge(paths):
    """Deduplicate repeated discovery of the same publication, never equal deck contents."""
    result = {}
    for path in paths:
        for row in load(path):
            previous = result.get(row['id'])
            if previous:
                for key in ('main', 'sideboard', 'commanders', 'companion', 'format'):
                    if previous.get(key) != row.get(key):
                        raise ValueError(f"conflicting publication {row['id']}: {key}")
                if previous.get('source_document_utf8_sha256') != row.get('source_document_utf8_sha256'):
                    versions = previous.setdefault('source_document_versions', [{
                        'source': previous['source'], 'sha256': previous['source_document_utf8_sha256']}])
                    version = {'source': row['source'], 'sha256': row['source_document_utf8_sha256']}
                    if version not in versions:
                        versions.append(version)
            else:
                result[row['id']] = row
    return [result[key] for key in sorted(result)]


def export(row, path):
    if row.get('source_issues'):
        raise ValueError('source needs review: ' + '; '.join(row['source_issues']))
    if row.get('names_absent_from_pinned_corpus'):
        raise ValueError('unresolved card names: ' + ', '.join(row['names_absent_from_pinned_corpus']))
    lines = [f"# {row['id']}", f"# Source: {row['source']}"]
    # Commander designations are setup data, not cards to shuffle into the library.
    lines.extend(f'{q} {name}' for name, q in row['main'].items())
    lines.extend(f'SB: {q} {name}' for name, q in row.get('sideboard', {}).items())
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text('\n'.join(lines) + '\n')
    setup = {'commanders': list(row.get('commanders', {})), 'companion': row.get('companion'),
             'implementation_status': 'unverified', 'game_recording': None}
    path.with_suffix('.setup.json').write_text(json.dumps(setup, indent=2) + '\n')


def overlap(a, b):
    """Multiset Jaccard: shared copies / copies in the union."""
    union = sum((a | b).values())
    return sum((a & b).values()) / union if union else 0.0


def deck_cards(row):
    cards = {name for part in ('main', 'sideboard', 'commanders') for name in row.get(part, {})}
    if row.get('companion'):
        cards.add(row['companion'])
    return cards


def era(row):
    date = row.get('event_date_iso') or row.get('deck_date')
    return str(int(date[:4]) // 5 * 5) if date else row.get('era', 'undated')


def coverage_features(row, scope):
    key = () if scope == 'card' else (row['format'],)
    if scope == 'format-era':
        key += (era(row),)
    return {('card', *key, name) for name in deck_cards(row)} | {('era', row['format'], era(row))}


def representative_groups(rows, threshold, basic=(), coverage_scope='card'):
    """Direct similarity groups, then restore witnesses for lost coverage."""
    if not 0 < threshold <= 1:
        raise ValueError('threshold must be in (0, 1]')
    if coverage_scope not in {'card', 'format', 'format-era'}:
        raise ValueError('unknown coverage scope')
    groups, indexes, sizes, dropped = [], defaultdict(lambda: defaultdict(list)), [], []
    ignored = set(basic)
    for row in rows:
        key = (row['format'], tuple(sorted(row.get('commanders', {}).items())), row.get('companion'))
        cards = Counter({n: q for n, q in row['main'].items() if n not in ignored})
        size = sum(cards.values())
        shared = defaultdict(int)
        for name, quantity in cards.items():
            for i, other_quantity in indexes[key][name]:
                shared[i] += min(quantity, other_quantity)
        matches = [(intersection / (size + sizes[i] - intersection), i)
                   for i, intersection in shared.items()
                   if intersection / (size + sizes[i] - intersection) >= threshold]
        if matches:
            score, parent = max(matches, key=lambda item: (item[0], -item[1]))
            groups[parent]['covered'].append({'id': row['id'], 'main_overlap': round(score, 6),
                                              'sideboard_overlap': round(overlap(Counter(row.get('sideboard', {})),
                                                  Counter(groups[parent]['deck'].get('sideboard', {}))), 6)})
            dropped.append((row, parent, coverage_features(row, coverage_scope)))
        else:
            i = len(groups)
            groups.append({'deck': row, 'reason': 'distinct_main_or_setup', 'covered': []})
            sizes.append(size)
            for name, quantity in cards.items():
                indexes[key][name].append((i, quantity))
    before = len(groups)
    required = set().union(*(coverage_features(row, coverage_scope) for row in rows))
    retained = set().union(*(coverage_features(group['deck'], coverage_scope) for group in groups))
    missing = required - retained
    initially_missing = set(missing)
    # Lazy greedy set cover: only promote variants which restore missing evidence.
    heap = [(-len(features & missing), i) for i, (_, _, features) in enumerate(dropped)]
    heapq.heapify(heap)
    promoted = set()
    while missing:
        if not heap:
            raise ValueError('coverage restoration failed')
        old_gain, i = heapq.heappop(heap)
        row, _, features = dropped[i]
        gain = len(features & missing)
        if not gain:
            continue
        if gain != -old_gain:
            heapq.heappush(heap, (-gain, i))
            continue
        restored = features & missing
        groups.append({'deck': row, 'reason': 'preserve_coverage', 'covered': [],
                       'restored_features': [list(feature) for feature in sorted(restored)]})
        promoted.add(row['id'])
        missing -= features
    for group in groups:
        group['covered'] = [member for member in group['covered'] if member['id'] not in promoted]
    selected = [group['deck'] for group in groups]
    all_cards = set().union(*(deck_cards(row) for row in rows))
    retained_cards = set().union(*(deck_cards(row) for row in selected))
    by_format = {}
    for fmt in sorted({row['format'] for row in rows}):
        source_cards = set().union(*(deck_cards(row) for row in rows if row['format'] == fmt))
        chosen_cards = set().union(*(deck_cards(row) for row in selected if row['format'] == fmt))
        by_format[fmt] = {'input_cards': len(source_cards), 'retained_cards': len(chosen_cards),
                          'lost_cards': sorted(source_cards - chosen_cards),
                          'representatives': sum(row['format'] == fmt for row in selected)}
    by_era = []
    for fmt, period in sorted({(row['format'], era(row)) for row in rows}):
        source_cards = set().union(*(deck_cards(row) for row in rows if (row['format'], era(row)) == (fmt, period)))
        chosen_cards = set().union(*(deck_cards(row) for row in selected if (row['format'], era(row)) == (fmt, period)))
        by_era.append({'format': fmt, 'era': period, 'input_cards': len(source_cards),
                       'retained_cards': len(chosen_cards), 'lost_cards': sorted(source_cards - chosen_cards)})
    summary = {'input_lists': len(rows), 'representatives_before_coverage_restoration': before,
               'representatives': len(groups), 'coverage_promotions': len(promoted),
               'threshold': threshold, 'coverage_scope': coverage_scope,
               'cards': {'input': len(all_cards), 'retained': len(retained_cards),
                         'lost_without_restoration': len({feature[-1] for feature in initially_missing if feature[0] == 'card'}),
                         'lost': sorted(all_cards - retained_cards)},
               'by_format': by_format, 'by_format_era': by_era,
               'meaning': 'Source-list representatives, not distinct archetypes or implemented decks. Similarity is quantity-aware main-deck Jaccard excluding basic lands, within format and setup identity. Card-name coverage includes main, sideboard and designated cards; unresolved source names remain review evidence.'}
    if required - set().union(*(coverage_features(row, coverage_scope) for row in selected)):
        raise ValueError('selected representatives lost required coverage')
    return groups, summary


def pairs(rows, threshold, basic=()):
    """Use an inverted card index; never merge via transitive similarity."""
    groups = defaultdict(list)
    for row in rows:
        key = (row['format'], tuple(sorted(row.get('commanders', {}).items())), row.get('companion'))
        groups[key].append(row)
    ignored = set(basic)
    for group in groups.values():
        index = defaultdict(list)
        sizes = []
        for i, row in enumerate(group):
            main = Counter({n: q for n, q in row['main'].items() if n not in ignored})
            size = sum(main.values())
            shared = defaultdict(int)
            for name, quantity in main.items():
                for j, other_quantity in index[name]:
                    shared[j] += min(quantity, other_quantity)
            for j, intersection in sorted(shared.items()):
                score = intersection / (size + sizes[j] - intersection)
                if score >= threshold:
                    yield {'left': group[j]['id'], 'right': row['id'], 'main_overlap': round(score, 6),
                           'sideboard_overlap': round(overlap(Counter(group[j].get('sideboard', {})),
                                                             Counter(row.get('sideboard', {}))), 6),
                           'main_identical': row['main'] == group[j]['main']}
            sizes.append(size)
            for name, quantity in main.items():
                index[name].append((i, quantity))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, default=DEFAULT)
    commands = parser.add_subparsers(dest='command', required=True)
    mg = commands.add_parser('merge')
    mg.add_argument('inputs', type=Path, nargs='+')
    mg.add_argument('--output', type=Path, required=True)
    ex = commands.add_parser('export')
    ex.add_argument('id')
    ex.add_argument('--output', type=Path, required=True)
    ov = commands.add_parser('overlap')
    ov.add_argument('--threshold', type=float, default=.85)
    ov.add_argument('--include-basic-lands', action='store_true')
    ov.add_argument('--format')
    ov.add_argument('--output', type=Path, default=ROOT / '.cache/deck-overlap.jsonl')
    rp = commands.add_parser('representatives', help='diagnostic similarity groups')
    rp.add_argument('--anchors', type=Path, required=True, help='lists that are always kept')
    rp.add_argument('--threshold', type=float, default=.85)
    rp.add_argument('--coverage-scope', choices=['card', 'format', 'format-era'], default='card')
    rp.add_argument('--output', type=Path, default=ROOT / '.cache/deck-representatives.json')
    args = parser.parse_args()
    if args.command == 'merge':
        rows = merge(args.inputs)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(''.join(json.dumps(row, ensure_ascii=False, sort_keys=True) + '\n' for row in rows))
        print(f'{len(rows)} publications: {args.output}')
        return
    rows = load(args.archive)
    if args.command == 'representatives':
        names = Names()
        anchors = load(args.anchors)
        clean = [row for row in rows if not (row.get('source_issues') or row.get('names_absent_from_pinned_corpus')
                  or row.get('text_export_check', {}).get('status') in {'different', 'unreadable'})]
        eligible_ids = {row['id'] for row in clean}
        review = [row for row in rows if row['id'] not in eligible_ids]
        review_ids = {row['id'] for row in review}
        def label(row):
            return row.get('archetype') or row.get('source_title', '').rsplit(' - ', 1)[0]
        frequencies = Counter((row['format'], era(row), label(row).casefold()) for row in clean)
        clean.sort(key=lambda row: (-frequencies[row['format'], era(row), label(row).casefold()],
                                   row['format'], era(row), row['id']))
        groups, summary = representative_groups(anchors + clean + review, args.threshold, names.basic, args.coverage_scope)
        source_known = set().union(*(deck_cards(row) for row in rows)) & names.exact
        eligible_known = set().union(*(deck_cards(row) for row in clean)) & names.exact
        all_names = set().union(*(deck_cards(row) for row in anchors + rows))
        selected_names = set().union(*(deck_cards(group['deck']) for group in groups))
        for group in groups:
            if group['deck']['id'] in review_ids:
                group['requires_source_review'] = True
        summary.update(archive_lists=len(rows), eligible_archive_lists=len(clean), anchor_lists=len(anchors),
                       source_review={'lists': len(review), 'ids': [row['id'] for row in review],
                                      'selected_representatives': sum(group.get('requires_source_review', False) for group in groups),
                                      'known_cards_only_in_review_lists': sorted(source_known - eligible_known),
                                      'unresolved_names': sorted(all_names - names.exact)},
                       known_physical_cards={'input': len(all_names & names.exact),
                                             'retained': len(selected_names & names.exact),
                                             'lost': sorted((all_names - selected_names) & names.exact)},
                       provenance={'archive_sha256': hashlib.sha256(args.archive.read_bytes()).hexdigest(),
                                   'anchors_sha256': hashlib.sha256(args.anchors.read_bytes()).hexdigest(),
                                   'corpus_sha256': hashlib.sha256((ROOT / 'resources/oracle/cards.jsonl').read_bytes()).hexdigest()})
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps({'summary': summary, 'representatives': groups}, ensure_ascii=False) + '\n')
        print(f"{summary['representatives']} representatives from {summary['input_lists']} source/anchor lists; "
              f"{summary['known_physical_cards']['retained']}/{summary['known_physical_cards']['input']} known physical cards retained; "
              f"{summary['coverage_promotions']} coverage promotions: {args.output}")
        return
    if args.command == 'export':
        row = next((r for r in rows if r['id'] == args.id), None)
        if row is None:
            parser.error('unknown deck ID')
        export(row, args.output)
        print(args.output)
    else:
        if not 0 < args.threshold <= 1:
            parser.error('threshold must be in (0, 1]')
        if args.format:
            rows = [r for r in rows if r['format'] == args.format]
        basic = () if args.include_basic_lands else Names().basic
        args.output.parent.mkdir(parents=True, exist_ok=True)
        count = 0
        with args.output.open('w') as out:
            for pair in pairs(rows, args.threshold, basic):
                out.write(json.dumps(pair, sort_keys=True) + '\n')
                count += 1
        print(f'{len(rows)} lists; {count} pairs at overlap >= {args.threshold}: {args.output}')


if __name__ == '__main__':
    main()
