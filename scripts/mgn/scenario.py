"""Small authoring helper for ordinary MGN position starts; no engine is invoked."""
from copy import deepcopy
import json

from .integrity import ROOT, digest
from .oracle import Oracle
from .state import zone_names
from .validate import validate
from .vocabulary import RULES
import pinned

PRIORITY_STEPS = ('upkeep', 'draw', 'precombat_main', 'begin_combat',
                  'end_combat', 'postcombat_main', 'end')


def _pins():
    manifest = json.loads((ROOT / 'resources/manifest.json').read_text())
    return {'rules': {'effective': pinned.RULES_EFFECTIVE, 'sha256': digest(RULES)},
            'oracle': {'source': 'scryfall-oracle-cards', 'sha256': digest(ROOT / 'resources/oracle/cards.jsonl'),
                       'updated_at': manifest['sources']['oracle_cards']['verification']['bulk_updated_at']}}


class Scenario:
    """Place named physical cards, then supply normal MGN actions and expectations.

    Repeated ``card`` calls append in MGN order: libraries top first,
    graveyards oldest first. Shared zones use their full names; ``hand``,
    ``library`` and ``graveyard`` are shorthand for the owner's zone.
    """
    def __init__(self, *, turn=3, step='precombat_main', active=None, priority=None,
                 seats=('A', 'B'), life=20, format='custom'):
        if step not in PRIORITY_STEPS:
            raise ValueError(f'scenario setup requires priority in one of {PRIORITY_STEPS}')
        if type(turn) is not int or turn < 1 or not seats or len(set(seats)) != len(seats):
            raise ValueError('scenario setup requires a positive turn and distinct seats')
        if len(seats) == 2 and turn == 1 and step == 'draw':
            raise ValueError('the first player skips the first-turn draw step in a two-player game (103.8a)')
        ordinary_active = seats[(turn - 1) % len(seats)]
        active = ordinary_active if active is None else active
        if active != ordinary_active:
            raise ValueError('active player must match the ordinary first-seat-first turn order')
        priority = active if priority is None else priority
        if priority not in seats:
            raise ValueError('priority player is not a seat')
        self._oracle = Oracle()
        self._game = {'format': format, 'life': life, 'hand_size': 7, 'seats': list(seats)}
        self._cards = {}
        self._state = {'turn': {'number': turn, 'step': step, 'active': active},
                       'pending': {'kind': 'priority', 'seat': priority}, 'result': {'kind': 'unfinished'},
                       'players': {seat: {'life': life} for seat in seats},
                       'zones': {zone: [] for zone in zone_names(seats)}, 'objects': {}}
        self._sick = []

    def player(self, seat, *, life=None, mana=None):
        if seat not in self._game['seats']:
            raise ValueError(f'unknown player {seat}')
        if life is not None:
            self._state['players'][seat]['life'] = life
        if mana is not None:
            self._state['players'][seat]['mana'] = deepcopy(mana)
        return self

    def card(self, label, name, *, owner, zone, controller=None, tapped=False,
             counters=None, damage=0, summoning_sick=False, expected=None):
        if label in self._cards:
            raise ValueError(f'duplicate card label {label}')
        if owner not in self._game['seats']:
            raise ValueError(f'unknown owner {owner}')
        zone = f'{owner}.{zone}' if zone in ('library', 'hand', 'graveyard') else zone
        if zone not in self._state['zones'] or zone in ('stack', 'command'):
            raise ValueError(f'unsupported setup zone {zone}')
        if '.' in zone and zone.split('.')[0] != owner:
            raise ValueError('a private zone belongs to the physical card owner (400.3)')
        if controller is not None and controller not in self._game['seats']:
            raise ValueError(f'unknown controller {controller}')
        if zone != 'battlefield' and (controller is not None or tapped or counters or damage or summoning_sick):
            raise ValueError('control, status and summoning sickness are battlefield setup facts')
        try:
            oracle_id = self._oracle.by_name(name)
        except KeyError as error:
            raise ValueError(f'card name must identify exactly one pinned Oracle identity: {name!r}') from error
        expected = deepcopy(expected or {})
        if set(expected) - {'power', 'toughness'}:
            raise ValueError('expected projection supports power and toughness only')
        if expected and zone != 'battlefield':
            raise ValueError('expected power and toughness are battlefield projections')
        facts = expected
        if controller is not None:
            facts['controller'] = controller
        if tapped:
            facts['tapped'] = tapped
        if counters:
            facts['counters'] = deepcopy(counters)
        if damage:
            facts['damage'] = damage
        self._cards[label] = {'owner': owner, 'oracle': oracle_id, 'name': self._oracle.name(oracle_id)}
        self._state['zones'][zone].append(label)
        if facts:
            self._state['objects'][label] = facts
        if summoning_sick:
            self._sick.append(label)
        return self

    def build(self, transitions=(), *, note=''):
        """Return an independent ordinary record, checked without an engine.

        Actions, payments and expected deltas are authored by the caller. This
        checks notation/reference coherence, not rules-engine admission or all
        possible rules consequences of the chosen board.
        """
        start = {'kind': 'position', 'state': deepcopy(self._state)}
        if self._sick:
            start['assumptions'] = {'summoning_sick': list(self._sick)}
        record = {'mgn': '1', 'tier': 2, 'pins': _pins(), 'game': deepcopy(self._game),
                  'author': {'kind': 'tool', 'name': 'mgn-scenario', 'version': '1'},
                  'cards': deepcopy(self._cards), 'start': start, 'transitions': deepcopy(list(transitions))}
        if note:
            record['note'] = note
        errors = [str(problem) for problem in validate(record, self._oracle) if problem.severity == 'error']
        if errors:
            raise ValueError('invalid MGN scenario: ' + '; '.join(errors[:3]))
        return record
