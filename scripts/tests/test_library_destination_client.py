"""Native managed destination choice, refusal rollback, public announcement and private surveil.

Rules: 400.2, 400.7, 401.3, 608.2c-d, 701.25a. Pinned Vanish from Sight
ruling 6c6fcb5cb1e8dc9e: an illegal sole target prevents surveil.
"""
import os
import unittest
import play
import test_player_prompt_client as fixtures


class LibraryDestinationClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('LIBRARY_DESTINATION_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('LIBRARY_DESTINATION_ENGINE'))
        self.addCleanup(self.close_engine)

    def test_owner_choice_is_retained_and_public_then_surveil_is_private(self):
        me, owner, observer = self.new(
            {'hand': ['Vanish from Sight'], 'battlefield': ['Island'] * 4},
            {'battlefield': ['Grizzly Bears']}, {})
        p = me.prompt()
        bear, = self.named(p, 'Grizzly Bears')
        old_bears = {}
        for seat in (owner, observer):
            old_bears[seat.label], = self.named(seat.prompt(), 'Grizzly Bears', zone='battlefield')
        spell, = self.named(p, 'Vanish from Sight', zone='hand')
        islands = self.named(p, 'Island', controller='p0', zone='battlefield')
        me.answer({'kind': 'cast', 'card': spell, 'targets': [[bear]], 'mana': {'U': 4},
                   'plan': [{'ability': f'{o}.a0'} for o in islands]})
        for _ in range(3):
            self.waiting().answer({'kind': 'pass'})
        self.assertIs(self.waiting(), owner)
        d = owner.last['decision']
        self.assertEqual((d['kind'], d['chooser'], d['cards']), ('library_destination', 'p1', [old_bears['p1']]))
        self.assertEqual(d['positions'], [{'zone': 'library', 'position': 'top'}, {'zone': 'library', 'position': 'bottom'}])
        for other in (me, observer):
            self.assertIsNone(other.prompt()['decision'])
            self.assertEqual(other.last['waiting_for'], 'p1')
        before = owner.prompt()
        self.assertIn('request belongs', self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'choose', 'answer': {'library_position': {'position': 'top'}}}}))
        self.assertIn('offered library positions', self.refused(owner, {'cmd': 'answer', 'answer': {'kind': 'choose', 'answer': {'library_position': {'from_top': 2}}}}))
        after = owner.prompt()
        for key in ('objects', 'players', 'decision', 'stack', 'choices_made'):
            self.assertEqual(after[key], before[key])
        owner.answer({'kind': 'choose', 'answer': {'library_position': {'position': 'bottom'}}})
        self.assertIs(self.waiting(), me)
        self.assertEqual(me.last['decision']['kind'], 'arrange')
        top, = me.last['decision']['cards']
        for seat in (owner, observer):
            p = seat.prompt()
            self.assertIsNone(p['decision'])
            self.assertFalse(self.named(p, 'Island', zone='library'))
            self.assertEqual(p['choices_made'], [{'player': 'p1', 'cards': [old_bears[seat.label]],
                                                  'library_position': {'zone': 'library', 'position': 'bottom'}}])
            self.assertFalse(self.named(p, 'Grizzly Bears', zone='library'))
        me.answer({'kind': 'choose', 'answer': {'top': [], 'away': [top]}})
        p = me.prompt()
        self.assertEqual(p['decision']['kind'], 'priority')
        self.assertEqual(p['stack'], [])
        self.assertEqual(len(self.named(p, 'Island', zone='graveyard')), 1)
        self.assertEqual(owner.prompt()['players'][1]['library'], before['players'][1]['library'] + 1)


if __name__ == '__main__':
    unittest.main()
