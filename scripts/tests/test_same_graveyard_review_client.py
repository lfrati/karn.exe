"""CR601.2c/404.1 group restrictions through labelled player choices.

The declared client position isolates the mechanism; it is not reached by ordinary game
startup. Two card targets in one actual graveyard are legal, a cross-yard
pair is not; public graveyard partitions carry no hidden library or hand identities.
"""
import unittest
import test_player_prompt_client as fixtures
import play

class SameGraveyardPlayerTests(unittest.TestCase):
    setUpClass = classmethod(fixtures.PlayerPromptClientTests.setUpClass.__func__)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    refused = fixtures.PlayerPromptClientTests.refused
    def setUp(self):
        self.engine=play.Engine()
        self.addCleanup(self.close_engine)

    def test_target_question_exposes_actual_graveyard_partitions_and_enforces_them(self):
        me,them=self.new({'hand':['Arashin Sunshield'],'battlefield':['Plains']*4,
                          'graveyard':['Forest','Mountain']},
                         {'graveyard':['Forest','Mountain']})
        p=me.prompt();[sun]=self.named(p,'Arashin Sunshield');lands=self.named(p,'Plains')
        me.answer({'kind':'cast','card':sun,'mana':{'W':4},'plan':[{'ability':f'{o}.a0'}for o in lands]})
        me.answer({'kind':'pass'});them.prompt();them.answer({'kind':'pass'})
        p=me.prompt();self.assertEqual(p['decision']['kind'],'targets')
        slot=p['decision']['slots'][0]
        owned = lambda owner: [o for o in slot['among'] if p['objects'][o]['owner']==owner]
        own,other=owned('p0'),owned('p1')
        self.assertEqual((len(own),len(other)),(2,2))
        self.assertEqual(slot['same_graveyard'],[own,other])
        self.assertIn('single graveyard',self.refused(me,{'cmd':'answer','answer':{'kind':'targets','targets':[[own[0],other[0]]]}}))
        me.answer({'kind':'targets','targets':[own]})
        me.answer({'kind':'pass'});them.prompt();them.answer({'kind':'pass'})
        final=me.prompt()
        self.assertEqual(sorted(final['objects'][o]['name']for o in final['exile']),['Forest','Mountain'])
        self.assertEqual(len(final['players'][1]['graveyard']),2)
        self.assertNotIn('hand',final['players'][1])
        self.assertNotIn('top_card',final['players'][1])
