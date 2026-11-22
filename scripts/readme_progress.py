"""Refresh the README's counts. Runs no tests: the counts say what compiles and what exists."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from pathlib import Path
import subprocess
import re
import sys

from mgn.runner import inventory_records
from progress import route_inventory
from progress_dashboard import deck_progress, whole_cards
import rules
import deck_archive
import rust_tools

ROOT = Path(__file__).resolve().parents[1]
START = '<!-- implementation-progress:start -->'
END = '<!-- implementation-progress:end -->'
TEST_START = '<!-- testing-inventory:start -->'
TEST_END = '<!-- testing-inventory:end -->'


def source_fingerprint():
    """Identify measurement inputs, including dirty source, independently of Git HEAD."""
    digest = hashlib.sha256(rules.suite_hash().encode())
    paths = [ROOT / 'resources/manifest.json', ROOT / 'resources/decks/manifest.json',
             ROOT / 'resources/decks/selected-decks.jsonl', ROOT / 'resources/decks/commander.jsonl',
             ROOT / 'resources/decks/commander-legality.json']
    paths += sorted((ROOT / 'tests/fixtures/mgn').rglob('*.json'))
    paths += sorted((ROOT / 'tests/fixtures/mgn').rglob('*.jsonl'))
    paths += sorted((ROOT / 'scripts').glob('*.py'))
    paths += sorted((ROOT / 'scripts/mgn').rglob('*.py'))
    for path in paths:
        digest.update(str(path.relative_to(ROOT)).encode() + b'\0' + path.read_bytes() + b'\0')
    return digest.hexdigest()


def record_counts(cases):
    counts = {'scenarios_or_prefixes': 0, 'complete_games': 0}
    for path in cases:
        record = json.loads(path.read_text())
        result = record['start'].get('state', {}).get('result', {'kind': 'unfinished'})
        for transition in record['transitions']:
            result = (transition.get('delta') or {}).get('result', result)
        if result.get('kind') not in ('unfinished', 'win', 'draw'):
            raise ValueError(f'invalid recorded result: {path}')
        # MGN has decks and arbitrary position starts (spec 2.4). A terminal
        # position scenario never becomes a complete-game inventory claim.
        complete = record['start']['kind'] == 'decks' and result['kind'] != 'unfinished'
        counts['complete_games' if complete else 'scenarios_or_prefixes'] += 1
    return counts


def measure(snapshot):
    """The two goals' counts: whole cards of the pinned corpus the grammar reads and the engine
    runs, and the published decks of the roster whose every card does."""
    inventory = route_inventory(snapshot)
    cards = whole_cards(inventory, ROOT / 'resources/oracle/cards.jsonl')
    roster = ROOT / 'resources/decks/selected-decks.jsonl'
    decks = deck_progress([json.loads(line) for line in roster.read_text().splitlines() if line.strip()], cards)['overall']
    commander = deck_progress(deck_archive.load(ROOT / 'resources/decks/commander.jsonl'), cards)['overall']
    corpus = json.loads((ROOT / 'resources/manifest.json').read_text())['sources']['oracle_cards']
    archive = json.loads((ROOT / 'resources/decks/manifest.json').read_text())
    test_paths = [*sorted((ROOT / 'crates/game/tests/scenarios').rglob('*.rs')),
                  *sorted((ROOT / 'crates/game/tests/engine').rglob('*.rs'))]
    return {
        'cards': {'total': len(cards), 'read': sum(card['parsed'] for card in cards.values()),
                  'run': sum(card['supported'] for card in cards.values()),
                  'dated': corpus['verification']['bulk_updated_at'][:10]},
        'decks': {'total': decks['decks'], 'main_read': decks['main_fully_read'],
                  'main_run': decks['main_fully_admitted'], 'run': decks['fully_admitted'],
                  'dated': archive['snapshot']},
        'commander': {'total': commander['decks'], 'main_read': commander['main_fully_read'],
                      'main_run': commander['main_fully_admitted'], 'dated': archive['commander']['snapshot']},
        'rule_cited_tests': sum(len(rules.test_references(path)) for path in test_paths),
        'mgn_records': record_counts(path for _, path in inventory_records(active=True)),
    }


def render(report):
    cards, decks, commander = report['cards'], report['decks'], report['commander']
    return f'''{START}
<!-- Measurement inputs: {report['fingerprint']} -->
Snapshot: {report['date']}. Card corpus dated {cards['dated']}; deck roster dated {decks['dated']}.

| Goal | Measure | Count |
| --- | --- | ---: |
| Read every card | Cards in the pinned corpus | {cards['total']:,} |
| | Cards the grammar reads, every face | {cards['read']:,} |
| Run most of Magic | Cards the engine runs, every face | {cards['run']:,} |
| | Published decks in the roster | {decks['total']:,} |
| | Decks whose main deck is read in full | {decks['main_read']:,} |
| | Decks whose main deck runs in full | {decks['main_run']:,} |
| | Decks that run in full with their sideboard | {decks['run']:,} |

Commander corpus (dated {commander['dated']}): **{commander['total']:,} authored lists**,
**{commander['main_read']:,} read in full**, **{commander['main_run']:,} compiled in full**, including
commanders. Separate from the tournament roster above.

A card that runs is one the engine compiles; that does not certify correct play.
{END}'''


def render_testing(report):
    records = report['mgn_records']
    return f'''{TEST_START}
Repository inventory: **{report['rule_cited_tests']:,} rule-cited tests**,
**{records['scenarios_or_prefixes']:,} active MGN scenarios or game prefixes**, and
**{records['complete_games']:,} complete MGN game recordings**. These are counts of tests
and recordings, not current test results or a measure of coverage.
{TEST_END}'''


def replace_block(readme, block, start=START, end=END):
    if readme.count(start) != 1 or readme.count(end) != 1:
        raise ValueError('README needs exactly one progress marker pair')
    before, rest = readme.split(start)
    _, after = rest.split(end)
    return before + block + after


def check_freshness(readme):
    """Authenticate measurement inputs only; no build, admission scan or writes."""
    replace_block(readme, '')
    replace_block(readme, '', TEST_START, TEST_END)
    block = readme.split(START)[1].split(END)[0]
    fingerprints = re.findall(r'<!-- Measurement inputs: ([0-9a-f]{64}) -->', block)
    if len(fingerprints) != 1:
        print('README snapshot needs exactly one measurement fingerprint; run make readme-progress.')
        return 1
    current = source_fingerprint()
    if fingerprints[0] != current:
        print(f'README snapshot is stale: recorded {fingerprints[0]}, current {current}.')
        print('Run make readme-progress. This check did not rebuild or rescan admission.')
        return 1
    print('README measurement inputs match. No tests or admission scan run.')
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='check measurement-input freshness without building or writing')
    args = parser.parse_args(argv)
    readme_path = ROOT / 'README.md'
    original = readme_path.read_text()
    if args.check:
        return check_freshness(original)
    replace_block(original, '')  # Refuse malformed markers before building.
    replace_block(original, '', TEST_START, TEST_END)
    cache = ROOT / '.cache/readme-progress'
    cache.mkdir(parents=True, exist_ok=True)
    before = source_fingerprint()
    engine = rust_tools.build('game-gate')
    snapshot = cache / 'admission.jsonl'
    subprocess.run([str(engine), '--limit', '0', '--admission-snapshot', str(snapshot)],
                   cwd=ROOT, stdout=subprocess.DEVNULL, check=True)
    report = measure(snapshot)
    if before != source_fingerprint():
        raise RuntimeError('measurement inputs changed during the scan; rerun')
    report.update(date=dt.datetime.now(dt.timezone.utc).date().isoformat(), fingerprint=before)
    if readme_path.read_text() != original:
        raise RuntimeError('README changed during the scan; rerun')
    (cache / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    updated = replace_block(original, render(report))
    readme_path.write_text(replace_block(updated, render_testing(report), TEST_START, TEST_END))
    print('Updated the README counts. No tests were run.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
