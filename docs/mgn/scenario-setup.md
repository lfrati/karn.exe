# Authoring a scenario setup

`mgn.scenario.Scenario` builds an ordinary tier-2 MGN position from named cards.
It resolves exact names against the pinned Oracle data and emits the card table,
input pins and start state. It does not invoke the engine or choose actions,
payments or expected results.

Run Python with `PYTHONPATH=scripts` (for example, `PYTHONPATH=scripts uv run python
my_scenario.py`):

```python
import json
from mgn.scenario import Scenario

board = (Scenario(turn=3, step='precombat_main')
    .card('bear', 'Grizzly Bears', owner='A', zone='battlefield')
    .card('forest', 'Forest', owner='A', zone='battlefield')
    .card('growth', 'Giant Growth', owner='A', zone='hand')
    .card('top', 'Forest', owner='A', zone='library')
    .card('bottom', 'Island', owner='A', zone='library')
    .player('A', life=18, mana={'C': 1}))

record = board.build([{
    'seat': 'A',
    'do': {
        'kind': 'cast', 'card': 'growth', 'targets': [['bear']],
        'pay': {'abilities': [{
            'source': 'forest', 'ability': {'rule': '305.6'}, 'adding': ['G']
        }]}
    },
    'delta': {
        'pending': {'kind': 'priority', 'seat': 'B'},
        'objects': {'forest': {'tapped': True},
                    'growth': {'zone': 'stack', 'targets': [['bear']]}}
    }
}], note='A casts Giant Growth and passes priority to B.')
print(json.dumps(record, indent=2))
```

The transition uses the existing MGN action and explicit payment notation.
Casting puts Growth on the stack; its effect requires resolution through the
normal priority sequence. The complete reviewed
[Growth/Bolt example](examples/scenario-growth-response.json) adds B's response:
Bolt kills the Bears before Growth resolves, so Growth has no legal target and
does not resolve (CR 608.2b). Its setup includes every supported card zone.

Each `card` call adds one physical card with a unique label. `hand`, `library`
and `graveyard` abbreviate the owner's zone; `battlefield` and `exile` are shared
zones. Calls append in MGN order: libraries are top first and graveyards are
oldest first. Names must resolve to exactly one pinned Oracle identity.

Battlefield cards may specify `controller`, `tapped`, `counters`, `damage` and
`summoning_sick`. Ownership stays in the card table; control is a separate
battlefield fact. For example:

```python
board.card('borrowed', 'Hill Giant', owner='B', zone='battlefield',
           controller='A', tapped=True, counters={'+1/+1': 1},
           damage=1, summoning_sick=True, expected={'power': 4, 'toughness': 4})
```

Omitted control means the owner controls the permanent. Omitted summoning
sickness means its controller has controlled it since the beginning of their
most recent turn. Life and mana can be set per player with `player`.

The helper does not calculate effective characteristics. Author any changed
power and toughness explicitly through `expected`: Hill Giant is printed 3/3,
so its +1/+1 counter requires the 4/4 projection above (CR 122.1a). These are
expected public values, checked with the rest of the record; they do not create
an effect or replace the actual counter. Omitted values use MGN's printed
characteristic defaults.

The setup is an ordinary turn in first-seat-first order. Active player defaults
to that turn's player and priority defaults to active player. Choose `upkeep`,
`draw`, `precombat_main`, `begin_combat`, `end_combat`, `postcombat_main` or `end`.
The position is at priority after the step's beginning procedures; importing a
draw-step position does not draw another card. The first player's first draw
step in a two-player game is skipped (CR 103.8a), so that setup is refused.

The stack and combat start empty. The ordinary position defaults apply: no land
plays or spells cast this turn, no consecutive passes, and nothing else pending
or in force beyond the stated facts. This is a convenient starting premise,
not a reconstruction of an earlier game. To establish spell effects, triggers,
combat or other consequences, start earlier and supply ordinary transitions.

`build` returns a defensive copy and checks MGN structure and references. It
does not prove that a board is reachable or that its cards are supported by a
particular engine. Derive expected deltas from the pinned rules, full Oracle
text and rulings, then review and replay the record ([testing](../testing.md)).
