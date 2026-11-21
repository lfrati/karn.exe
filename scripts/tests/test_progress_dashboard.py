import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import progress_dashboard as dashboard


class DashboardTests(unittest.TestCase):
    def corpus(self, directory, rows):
        path = Path(directory) / 'cards.jsonl'
        path.write_text(''.join(json.dumps(r) + '\n' for r in rows))
        return path

    def test_whole_card_requires_every_part_and_deduplicates_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            rows = [{'oracle_id': 'dfc', 'name': 'Front // Back', 'face': i} for i in (0, 1)]
            rows += [{'oracle_id': 'single', 'name': 'Single'}]
            path = self.corpus(temp, rows)
            inventory = {('dfc', 0): {'stage': 'admitted'}, ('dfc', 1): {'stage': 'parse'},
                         ('single', 0): {'stage': 'engine'}}
            cards = dashboard.whole_cards(inventory, path)
            self.assertEqual(len(cards), 2)
            self.assertFalse(cards['dfc']['parsed'])
            self.assertTrue(cards['single']['parsed'])
            self.assertFalse(cards['single']['supported'])
            inventory['dfc', 1]['stage'] = 'engine'
            cards = dashboard.whole_cards(inventory, path)
            self.assertTrue(cards['dfc']['parsed'])
            self.assertFalse(cards['dfc']['supported'])
            inventory['dfc', 1]['stage'] = 'admitted'
            self.assertTrue(dashboard.whole_cards(inventory, path)['dfc']['supported'])
            del inventory['dfc', 1]
            with self.assertRaises(ValueError):
                dashboard.whole_cards(inventory, path)

    def test_meld_result_is_dependency_not_an_extra_physical_card(self):
        with tempfile.TemporaryDirectory() as temp:
            names = ['Graf Rats', 'Midnight Scavengers', 'Chittering Host']
            rows = [{'oracle_id': n, 'name': n, 'layout': 'meld', 'mana_cost': '{2}' if i < 2 else None,
                     'type_line': 'Creature'} for i, n in enumerate(names)]
            path = self.corpus(temp, rows)
            inventory = {(n, 0): {'stage': 'admitted' if i < 2 else 'parse'} for i, n in enumerate(names)}
            cards = dashboard.whole_cards(inventory, path)
            self.assertEqual(set(cards), {'Graf Rats', 'Midnight Scavengers'})
            self.assertFalse(any(c['parsed'] for c in cards.values()))
            inventory['Chittering Host', 0]['stage'] = 'admitted'
            self.assertTrue(all(c['supported'] for c in dashboard.whole_cards(inventory, path).values()))

    def test_combined_decks_use_equal_weight_mean_not_copies_or_pooled_cards(self):
        cards = {n: {'name': n, 'parsed': True, 'supported': n == 'A'} for n in 'ABCDEF'}
        rows = [{'id': 'one', 'format': 'modern', 'main': {'A': 4}, 'sideboard': {'A': 1, 'B': 2}},
                {'id': 'two', 'format': 'legacy', 'main': {'A': 1, 'B': 1, 'C': 1, 'D': 1},
                 'commanders': {'E': 1}, 'companion': 'F'}]
        result = dashboard.deck_progress(rows, cards)
        self.assertEqual(result['entries'][0]['total_cards'], 2)
        self.assertEqual(result['entries'][0]['percent'], 50)
        self.assertAlmostEqual(result['entries'][1]['percent'], 100/6)
        self.assertAlmostEqual(result['overall']['percent'], (50 + 100/6)/2)
        self.assertEqual(result['by_format']['modern']['percent'], 50)
        self.assertNotAlmostEqual(result['overall']['percent'], 100*2/8)
        rows[0]['sideboard']['Unknown'] = 1
        self.assertEqual(dashboard.deck_progress(rows, cards)['entries'][0]['unresolved_names'], ['Unknown'])

    def test_other_formats_are_combined_with_equal_deck_weight(self):
        cards = {'A': {'name': 'A', 'parsed': True, 'supported': True},
                 'B': {'name': 'B', 'parsed': False, 'supported': False}}
        rows = [{'id': 'commander', 'format': 'commander', 'main': {'A': 1}},
                {'id': 'pioneer', 'format': 'pioneer', 'main': {'A': 4, 'B': 4}}]
        result = dashboard.deck_progress(rows, cards)
        self.assertEqual(list(result['by_format']), ['pauper', 'standard', 'modern', 'legacy', 'vintage', 'others'])
        self.assertEqual(result['by_format']['others']['decks'], 2)
        self.assertEqual(result['by_format']['others']['percent'], 75)
        self.assertEqual([r['format'] for r in result['entries']], ['commander', 'pioneer'])


    def test_engine_support_produces_numeric_card_and_deck_coverage_without_certification(self):
        with tempfile.TemporaryDirectory() as temp:
            corpus = self.corpus(temp, [{'oracle_id': 'A', 'name': 'A'}, {'oracle_id': 'B', 'name': 'B'}])
            selected = Path(temp) / 'selected.jsonl'
            selected.write_text(json.dumps({'id': 'one', 'format': 'modern', 'main': {'A': 4}, 'sideboard': {'B': 1}}))
            ledger = {'execution_current': False, 'metrics': {'total': 2, 'verified': 0},
                      'execution': None, 'rules': {}}
            result = dashboard.dashboard({('A', 0): {'stage': 'admitted'}, ('B', 0): {'stage': 'engine'}},
                                         corpus, selected, ledger)
            self.assertEqual(result['cards']['supported'], 1)
            self.assertEqual(result['decks']['overall']['percent'], 50)
            self.assertNotIn('UNKNOWN', dashboard.render({'dashboard': result}))

    def test_rule_coverage_counts_passes_not_references_or_certificates(self):
        ledger = {'execution_current': True, 'execution': {'exit_code': 0,
                  'tests': {'good': ['ok'], 'failed': ['FAILED'], 'ignored': ['ignored'], 'shared': ['ok', 'ok']}},
                  'rules': {str(i): {'status': 'partial', 'tests': [{'test': t}]} for i, t in enumerate(
                      ['good', 'good', 'failed', 'ignored', 'never-run', 'shared'])}}
        result = dashboard.rule_test_coverage(ledger)
        self.assertEqual(result['tested'], 3)
        self.assertEqual(result['status'], 'CURRENT')
        ledger['execution_current'] = False
        historical = dashboard.rule_test_coverage(ledger)
        self.assertEqual(historical['tested'], 3)
        self.assertEqual(historical['status'], 'LAST RUN')
        ledger['execution']['exit_code'] = 1
        self.assertEqual(dashboard.rule_test_coverage(ledger)['status'], 'LAST RUN (FAILURES)')

    def test_incomplete_coverage_never_rounds_up_to_one_hundred_percent(self):
        value = {'cards': {'total': 200000, 'parsed': 199999, 'supported': 199999},
                 'rules': {'total': 200000, 'tested': 199999, 'passing_tests': 199999, 'status': 'CURRENT'},
                 'decks': {'overall': {'decks': 2000, 'percent': 99.9999}, 'by_format': {}},
                 'measurement_current': True}
        output = dashboard.render({'dashboard': value})
        self.assertNotIn('100.00%', output)
        self.assertEqual(output.count('99.99%'), 4)
        value['cards']['parsed'] = 200000
        self.assertIn('100.00%', dashboard.render({'dashboard': value}))

    def test_attributed_rule_evidence_leads_and_legacy_references_follow(self):
        value = {'cards': {'total': 10, 'parsed': 5, 'supported': 4},
                 'rules': {'total': 100, 'tested': 40, 'passing_tests': 50, 'status': 'CURRENT',
                           'attributed': {'scope': 's', 'total': 100, 'rules_with_sites': 3, 'rules_without_sites': 97,
                                          'sites': 4, 'row_supplied_sites': 0, 'problems': [], 'run_state': 'MISSING',
                                          'execution': None, 'run': None}},
                 'decks': {'overall': {'decks': 0, 'percent': None}, 'by_format': {}}, 'measurement_current': True}
        lines = dashboard.render({'dashboard': value}).splitlines()
        attributed = next(i for i, l in enumerate(lines) if 'Rules with attributed expectations  3/100' in l)
        legacy = next(i for i, l in enumerate(lines) if l.strip().startswith('Legacy: rules named by passing tests'))
        self.assertLess(attributed, legacy)
        self.assertIn('Executed: UNAVAILABLE, no attributed run', lines[attributed + 1])


    def test_changed_inputs_cannot_publish_current_percentages(self):
        with tempfile.TemporaryDirectory() as temp:
            corpus = self.corpus(temp, [{'oracle_id': 'A', 'name': 'A'}])
            selected = Path(temp) / 'selected.jsonl'
            selected.write_text(json.dumps({'id': 'one', 'format': 'modern', 'main': {'A': 1}}))
            ledger = {'execution_current': True, 'metrics': {'total': 2, 'verified': 1}, 'execution': {}, 'rules': {}}
            result = dashboard.dashboard({('A', 0): {'stage': 'admitted'}}, corpus, selected,
                                         ledger, measurement_current=False)
            self.assertIsNone(result['cards']['parsed'])
            self.assertIsNone(result['rules']['verified'])
            self.assertIsNone(result['decks']['overall']['supported_percent'])
            self.assertIsNone(result['decks']['overall']['main_fully_admitted'])
            self.assertIsNone(result['decks']['entries'][0]['fully_admitted'])
            output = dashboard.render({'dashboard': result})
            self.assertIn('Inputs changed', output)
            self.assertNotIn('100.00%', output)
            self.assertNotIn('SIDE', output)
            self.assertNotIn('faces', output)


    def test_fully_admitted_lists_include_designations_and_sideboard_separately(self):
        # B is read by the grammar and does not run
        cards = {n: {'name': n, 'parsed': True, 'supported': n == 'A'} for n in 'AB'}
        rows = [{'id': 'sideboard-gap', 'format': 'modern', 'main': {'A': 60}, 'sideboard': {'B': 1}},
                {'id': 'companion-gap', 'format': 'modern', 'main': {'A': 60}, 'companion': 'B'},
                {'id': 'commander-gap', 'format': 'commander', 'main': {'A': 99}, 'commanders': {'B': 1}},
                {'id': 'unknown-main', 'format': 'modern', 'main': {'Unknown': 1}},
                {'id': 'admitted', 'format': 'modern', 'main': {'A': 60}}]
        result = dashboard.deck_progress(rows, cards)
        self.assertEqual(result['overall']['main_fully_admitted'], 2)
        self.assertEqual(result['overall']['fully_admitted'], 1)
        self.assertEqual([e['main_fully_admitted'] for e in result['entries']], [True, False, False, False, True])
        # a companion and a commander are read with the main deck; an unknown name never is
        self.assertEqual([e['main_fully_read'] for e in result['entries']], [True, True, True, False, True])
        self.assertEqual(result['overall']['main_fully_read'], 4)

    def test_archive_local_populations_and_assessments_are_distinct(self):
        value = {'cards': {'total': 10, 'parsed': 9, 'supported': 5},
                 'rules': {'total': 10, 'tested': 2, 'status': 'LAST RUN', 'passing_tests': 3,
                           'revision': 'historical-revision', 'assessments': {'unreviewed': 8, 'partial': 2}},
                 'decks': {'overall': {'decks': 2087, 'percent': 67.86, 'main_fully_admitted': 100,
                                      'fully_admitted': 80}, 'by_format': {}}, 'measurement_current': True}
        local = {'main_decks_total': 67, 'main_decks_admitted': 13, 'whole_decks_admitted': 10}
        output = dashboard.render({'dashboard': value, 'decks': local})
        self.assertIn('Selected archive decks', output)
        self.assertIn('2,087', output)
        self.assertIn('Local decks tree: 67 lists; 13 main lists admitted, 10 whole lists admitted', output)
        self.assertIn('LAST RUN (historical-r)', output)
        self.assertIn('unreviewed 8', output)
        self.assertIn('unassessed, not missing', output)
        self.assertNotIn('faces', output)


    def test_measurement_change_keeps_failed_historical_suite_label(self):
        with tempfile.TemporaryDirectory() as temp:
            corpus = self.corpus(temp, [{'oracle_id': 'A', 'name': 'A'}])
            selected = Path(temp) / 'selected.jsonl'
            selected.write_text(json.dumps({'id': 'one', 'format': 'modern', 'main': {'A': 1}}))
            ledger = {'execution_current': False, 'metrics': {'total': 1, 'verified': 0}, 'rules': {},
                      'execution': {'exit_code': 101, 'tests': {}, 'revision': {'commit': 'failed-revision'}}}
            result = dashboard.dashboard({('A', 0): {'stage': 'admitted'}}, corpus, selected, ledger, False)
            self.assertEqual(result['rules']['status'], 'LAST RUN (FAILURES)')
            self.assertEqual(result['rules']['revision'], 'failed-revision')

    def test_legal_pool_counts_whole_cards_and_tallies_first_blockers(self):
        with tempfile.TemporaryDirectory() as temp:
            corpus = self.corpus(temp, [{'oracle_id': 'A', 'name': 'A'}, {'oracle_id': 'B', 'name': 'B'},
                                        {'oracle_id': 'C', 'name': 'C'}, {'oracle_id': 'C', 'name': 'C', 'face': 1},
                                        {'oracle_id': 'D', 'name': 'D'}])
            inventory = {('A', 0): {'stage': 'admitted'}, ('B', 0): {'stage': 'parse', 'reason': 'text the grammar does not read'},
                         ('C', 0): {'stage': 'admitted'}, ('C', 1): {'stage': 'engine', 'reason': 'keyword: Ward'},
                         ('D', 0): {'stage': 'engine', 'reason': 'keyword: Ward'}}
            legal = Path(temp) / 'legal-standard.jsonl'
            legal.write_text(''.join(json.dumps({'oracle_id': i, 'name': i}) + '\n' for i in 'ABC'))
            cards = dashboard.whole_cards(inventory, corpus)
            pool = dashboard.legal_pool(cards, inventory, legal)
            self.assertEqual((pool['format'], pool['total'], pool['supported'], pool['parsed']), ('standard', 3, 1, 2))
            self.assertEqual(pool['refused'], {'parse': 1, 'engine': 1}, 'C is refused by its second face; D is not in the pool')
            self.assertEqual(pool['first_blockers'], [{'reason': 'keyword: Ward', 'cards': 1, 'examples': 'C'},
                                                      {'reason': 'not read: text the grammar does not read', 'cards': 1, 'examples': 'B'}])
            self.assertIsNone(dashboard.legal_pool(cards, inventory, legal, measurement_current=False)['supported'])
            legal.write_text(json.dumps({'oracle_id': 'Z', 'name': 'Zombie'}) + '\n')
            with self.assertRaisesRegex(ValueError, 'not in the corpus: Zombie'):
                dashboard.legal_pool(cards, inventory, legal)
            selected = Path(temp) / 'selected.jsonl'
            selected.write_text(json.dumps({'id': 'one', 'format': 'standard', 'main': {'A': 4}}))
            ledger = {'execution_current': False, 'metrics': {'total': 2, 'verified': 0}, 'execution': None, 'rules': {}}
            self.assertNotIn('legal', dashboard.dashboard(inventory, corpus, selected, ledger))
            legal.write_text(json.dumps({'oracle_id': 'A', 'name': 'A'}) + '\n')
            self.assertEqual(dashboard.dashboard(inventory, corpus, selected, ledger, legal_path=legal)['legal']['supported'], 1)

    def test_legal_pool_report_binds_effective_date_to_exact_pool_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            legal = root / 'oracle/legal-standard.jsonl'
            legal.parent.mkdir()
            legal.write_text('{"oracle_id":"A","name":"A"}\n')
            manifest = {'files': {'oracle/legal-standard.jsonl': {'sha256': dashboard.hashlib.sha256(legal.read_bytes()).hexdigest()}},
                        'sources': {'legal_standard': {'updated_at': '2026-10-11T09:00:00Z',
                                                      'scope': {'effective_at': '2026-10-11T09:00:00Z'}}}}
            (root / 'manifest.json').write_text(json.dumps(manifest))
            pool = dashboard.legal_pool({'A': {'supported': True, 'parsed': True}}, {}, legal)
            self.assertEqual(pool['effective_at'], '2026-10-11T09:00:00Z')
            self.assertEqual(pool['pool_sha256'], manifest['files']['oracle/legal-standard.jsonl']['sha256'])
            manifest['files']['oracle/legal-standard.jsonl']['sha256'] = 'different'
            (root / 'manifest.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, 'provenance manifest'):
                dashboard.legal_pool({'A': {'supported': True, 'parsed': True}}, {}, legal)


if __name__ == '__main__':
    unittest.main()
