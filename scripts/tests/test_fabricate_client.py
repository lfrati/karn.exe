"""Fabricate's resolving choice and Cayth's granted instance through the client."""
from client_support import ClientTestCase


class FabricateClientTests(ClientTestCase):
    def test_weaponcraft_chooses_after_entry_and_resolves_the_whole_branch(self):
        # CR 702.123a and pinned Enthusiast ruling: enter0/1, then choose during
        # resolution; the answer immediately places counters or creates Servos.
        for counters in [False, True]:
            with self.subTest(counters=counters):
                self.new([{'hand': ['Weaponcraft Enthusiast'], 'battlefield': ['Swamp'] * 3}, {}])
                self.cast('Weaponcraft Enthusiast', mana=self.ids('Swamp'), spend={'B': 3})
                self.pass_round()
                source, = self.ids('Weaponcraft Enthusiast', zone='battlefield')
                self.assertEqual(self.object(source)['counters'], {})
                self.assertEqual(self.object(source)['power'], 0)
                self.pass_round()
                self.assertEqual(self.engine.state['asked']['kind'], 'may')
                self.engine.act({'kind': 'choose', 'answer': 'yes' if counters else 'no'})
                self.assertEqual(self.object(source)['counters'], {'+1/+1': 2} if counters else {})
                self.assertEqual(len(self.ids('Servo Token', zone='battlefield')), 0 if counters else 2)
                self.assertEqual(self.engine.state['stack'], [])

    def test_bounced_source_creates_servos_without_offering_an_impossible_counter_payment(self):
        # Pinned Enthusiast ruling; CR 608.2d, 702.123a: leaving before the
        # trigger resolves forces the Servo branch, with no counter choice.
        self.new([{'hand': ['Weaponcraft Enthusiast', 'Unsummon'],
                   'battlefield': ['Swamp'] * 3 + ['Island']}, {}])
        self.cast('Weaponcraft Enthusiast', mana=self.ids('Swamp'), spend={'B': 3})
        self.pass_round()
        source, = self.ids('Weaponcraft Enthusiast', zone='battlefield')
        self.cast('Unsummon', [[{'object': source}]], mana=self.ids('Island'), spend={'U': 1})
        self.pass_round()
        self.assertEqual(self.ids('Weaponcraft Enthusiast', zone='battlefield'), [])
        self.pass_round()
        self.assertEqual(len(self.ids('Servo Token', zone='battlefield')), 2)
        self.assertEqual(self.engine.state['stack'], [])
        self.assertEqual(self.engine.state['asked']['kind'], 'priority')

    def test_cayth_grants_the_trigger_to_bears_and_excludes_its_servo(self):
        # Oracle; CR 613.1f, 702.123a: Cayth's own trigger creates a Servo; its
        # static grant applies to the later nontoken Bears, not that Servo.
        self.new([{'hand': ['Cayth, Famed Mechanist', 'Grizzly Bears'],
                   'battlefield': ['Island', 'Mountain', 'Plains'] + ['Forest'] * 3}, {}])
        forests = self.ids('Forest')
        self.cast('Cayth, Famed Mechanist', mana=self.ids('Island') + self.ids('Mountain') + self.ids('Plains') + forests[:1],
                  spend={'U': 1, 'R': 1, 'W': 1, 'G': 1})
        self.pass_round()
        self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'], 'may')
        self.engine.act({'kind': 'choose', 'answer': 'no'})
        self.assertEqual(len(self.ids('Servo Token', zone='battlefield')), 1)
        self.assertEqual(self.engine.state['stack'], [])
        self.cast('Grizzly Bears', mana=forests[1:], spend={'G': 2})
        self.pass_round()
        self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'], 'may')
        self.engine.act({'kind': 'choose', 'answer': 'yes'})
        bear, = self.ids('Grizzly Bears', zone='battlefield')
        cayth, = self.ids('Cayth, Famed Mechanist', zone='battlefield')
        self.assertEqual(self.object(bear)['counters'], {'+1/+1': 1})
        self.assertEqual(self.object(cayth)['counters'], {})
        self.assertEqual(len(self.ids('Servo Token', zone='battlefield')), 1)
