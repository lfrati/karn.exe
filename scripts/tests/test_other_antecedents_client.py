"""Full-card object/group contrasts through the audited native client."""
import unittest
import play
import test_grant_client as helpers


class OtherAntecedentsClientTests(unittest.TestCase):
    setUpClass = classmethod(helpers.GrantClientTests.setUpClass.__func__)
    setUp = helpers.GrantClientTests.setUp
    close_engine = helpers.GrantClientTests.close_engine
    new = helpers.GrantClientTests.new
    ids = helpers.GrantClientTests.ids
    cast = helpers.GrantClientTests.cast
    pass_round = helpers.GrantClientTests.pass_round
    view = helpers.GrantClientTests.view
    object = helpers.GrantClientTests.object

    def test_martial_coup_spares_its_created_tokens_for_every_seat(self):
        self.new([{'hand': ['Martial Coup'], 'battlefield': ['Plains'] * 7},
                  {'battlefield': ['Grizzly Bears']}, {}])
        self.cast('Martial Coup', mana=self.ids('Plains', zone='battlefield'), spend={'W': 7}, x=5)
        self.pass_round()
        for seat in range(3):
            view = self.view(seat)
            tokens = [view['objects'][str(o)] for o in view['battlefield'] if view['objects'][str(o)]['token']]
            self.assertEqual(len(tokens), 5)
            self.assertTrue(all((o['power'], o['toughness'], o['controller']) == (1, 1, 0) for o in tokens))
            self.assertFalse(any(view['objects'][str(o)]['name'] == 'Grizzly Bears' for o in view['battlefield']))

    def test_oketras_zombies_get_only_their_own_bonus(self):
        self.new([{'hand': ["In Oketra's Name"], 'battlefield': ['Plains'] * 2 + ['Walking Corpse', 'Grizzly Bears']},
                  {'battlefield': ['Walking Corpse']}])
        self.cast("In Oketra's Name", mana=self.ids('Plains'), spend={'W': 2})
        self.pass_round()
        own_zombie, = self.ids('Walking Corpse', zone='battlefield', controller=0)
        other_zombie, = self.ids('Walking Corpse', zone='battlefield', controller=1)
        bear, = self.ids('Grizzly Bears', zone='battlefield')
        for seat in range(2):
            objects = self.view(seat)['objects']
            self.assertEqual((objects[str(own_zombie)]['power'], objects[str(own_zombie)]['toughness']), (4, 3))
            self.assertEqual((objects[str(bear)]['power'], objects[str(bear)]['toughness']), (3, 3))
            self.assertEqual((objects[str(other_zombie)]['power'], objects[str(other_zombie)]['toughness']), (2, 2))

    def test_masquerade_rejects_its_dying_creature_before_any_resolution(self):
        self.new([{'hand': ['Illicit Masquerade', 'Doom Blade'], 'graveyard': ['Elvish Visionary'],
                   'battlefield': ['Grizzly Bears'] + ['Swamp'] * 6}, {}])
        swamps = self.ids('Swamp', zone='battlefield')
        self.cast('Illicit Masquerade', mana=swamps[:4], spend={'B': 4})
        self.pass_round()  # The enchantment enters; its counter-granting ability triggers.
        self.pass_round()
        bear, = self.ids('Grizzly Bears', zone='battlefield')
        self.assertEqual(self.object(bear)['counters'].get('impostor'), 1)
        self.cast('Doom Blade', [[{'object': bear}]], mana=swamps[4:], spend={'B': 2})
        self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'], 'targets')
        graveyard = self.engine.state['players'][0]['graveyard']
        dead, = [o for o in graveyard if self.object(o)['name'] == 'Grizzly Bears']
        other, = [o for o in graveyard if self.object(o)['name'] == 'Elvish Visionary']
        before = self.engine.state['request_id']
        with self.assertRaises(play.Refused) as rejected:
            self.engine.act({'kind': 'targets', 'targets': [[{'object': dead}]]})
        self.assertEqual(rejected.exception.refusal_class, 'illegal')
        self.assertIn('object the target must be other than', rejected.exception.answer['error'])
        self.assertEqual(self.view(0)['request_id'], before)
        self.engine.act({'kind': 'targets', 'targets': [[{'object': other}]]})
        self.pass_round()
        self.assertTrue(self.ids('Elvish Visionary', zone='battlefield'))
        self.assertTrue(self.ids('Grizzly Bears', zone='exile'))


if __name__ == '__main__':
    unittest.main()
