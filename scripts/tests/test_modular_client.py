"""Modular targets and optional last-known counter placement through the player client."""
from client_support import ClientTestCase


class ModularClientTests(ClientTestCase):
    def test_self_sacrificed_ravager_offers_the_opponents_artifact_creature_and_optional_transfer(self):
        # CR 702.43a, 113.7a, 608.2h: the activated source has left before either
        # ability resolves. Modular retains its one counter and explicitly chooses.
        for accept in [False, True]:
            with self.subTest(accept=accept):
                self.new([{'hand': ['Arcbound Ravager'], 'battlefield': ['Wastes'] * 2},
                          {'battlefield': ['Ornithopter']}])
                self.cast('Arcbound Ravager', mana=self.ids('Wastes'), spend={'C': 2})
                self.pass_round()
                ravager, = self.ids('Arcbound Ravager', zone='battlefield')
                artifact, = self.ids('Ornithopter', zone='battlefield')
                self.assertEqual(self.object(ravager)['counters'], {'+1/+1': 1})
                self.engine.act({'kind': 'activate', 'source': ravager, 'ability': 0,
                                 'targets': [], 'paying': [[ravager]]})
                self.assertEqual(self.engine.state['asked']['kind'], 'targets')
                self.engine.act({'kind': 'targets', 'targets': [[{'object': artifact}]]})
                self.pass_round()
                self.assertEqual(self.engine.state['asked']['kind'], 'may')
                self.engine.act({'kind': 'choose', 'answer': 'yes' if accept else 'no'})
                self.assertEqual(self.object(artifact)['counters'], {'+1/+1': 1} if accept else {})
                self.assertEqual(self.ids('Arcbound Ravager', zone='battlefield'), [])
