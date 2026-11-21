"""CR 601.2g/h and 605.4a: a real triggered choice interrupts the same cast.

Fertile Ground supplies one chosen color in addition to Forest's own G.
Expectations are authored from those Oracle instructions, not observed output.
"""
from copy import deepcopy, copy
import unittest
from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.native_actions import native_action
from test_mgn_adapter import ENGINE, ORACLE, MAIN_A, PRIORITY_A, position_record, setup_of, no_random


@unittest.skipUnless(ENGINE.is_file(), "requires the stamped candidate engine")
class TriggeredContinuationTests(unittest.TestCase):
    def setUp(self):
        self.a=KarnAdapter(ENGINE, ORACLE)
        self.addCleanup(self.a.end)

    def start(self, spell="Grizzly Bears", second=False):
        record=position_record({"forest":("Forest","A"), "f2":("Forest","A"), "f3":("Forest","A"),
            "fertile2":("Fertile Ground","A"), "f4":("Forest","A"), "fertile":("Fertile Ground","A"), "bears":(spell,"A")},
            {"A.hand":["fertile","fertile2","bears"], "battlefield":["forest","f2","f3","f4"]}, MAIN_A, PRIORITY_A)
        if second:
            record["start"]["state"]["players"]["A"]["mana"]={"G":2}
        self.a.setup(setup_of(record))
        # Establish the attachment by the actual cast; no raw attachment premise.
        self.a.act(0,"A",{"kind":"cast","card":"fertile","targets":[["forest"]],"pay":{"abilities":[
            {"source":f,"ability":{"rule":"305.6"},"adding":["G"]} for f in ("f2","f3")]}},no_random)
        self.a.default(None,no_random)
        self.a.default(None,no_random)
        self.assertEqual(self.a._reply_state()["state"]["objects"]["fertile"]["attached_to"],"forest")
        if second:
            self.a.act(1,"A",{"kind":"cast","card":"fertile2","targets":[["f4"]],"pay":{"pool":{"G":2}}},no_random)
            self.a.default(None,no_random);self.a.default(None,no_random)
        return {"kind":"cast", "card":"bears", "pay":{"abilities":[
            {"source":"forest", "ability":{"rule":"305.6"}, "adding":["G"]}],
            "allocation":[{"symbol":0,"mana":"U"},{"symbol":1,"mana":"G"}]}}

    def test_fertile_ground_waits_for_its_own_choice_before_the_exact_root_spend(self):
        action=self.start()
        before=self.a._reply_state()
        move, prefix=self.a._explicit_payment(0,self.a._move(action),action)
        self.assertEqual(self.a._reply_state(),before)
        self.assertFalse(prefix["complete"])
        self.assertEqual(prefix["suspended"]["stage"],"mana_choice")
        self.assertNotIn("mana",move,"future trigger output is not selected early")
        self.assertFalse(any(e["event"]=="cast" for e in prefix["events"]))
        reply=self.a.act(1,"A",action,no_random)
        self.assertEqual(reply["state"]["pending"],{"seat":"A","kind":"choose_mana","about":{"object":"fertile"}})
        self.assertEqual(reply["state"]["players"]["A"]["mana"],{"G":1})
        self.assertEqual(reply["state"]["zones"]["stack"],["bears"])
        frozen=self.a._reply_state()
        for seat,answer,reason in [("B",["U"],"another player.s request"),("A",["C"],"choice is which of the mana offered to add")]:
            with self.subTest(seat=seat,answer=answer),self.assertRaisesRegex(Refusal,reason):
                self.a.act(2,seat,{"kind":"choose_mana","mana":answer},no_random)
            self.assertEqual(self.a._reply_state(),frozen)
        choice={"kind":"choose_mana","mana":["U"]}
        native,proof,evidence=self.a._continue_payment(0,self.a._move(choice))
        self.assertEqual(native["pending_payment"]["mana"],{"U":1,"G":1})
        self.assertEqual(self.a._reply_state(),frozen)
        # a move whose quote is not the one expected uses an outdated quote (stale); a pending
        # payment that does not say what it takes is malformed (game-serve's refusal classes)
        changed="payment quote changed or is incomplete; action was not applied"
        for change,class_,reason in ((lambda m:m["pending_payment"].update(source=999),"stale",changed),
                       (lambda m:m["pending_payment"].update(path=[0]),"malformed","pending_payment names its exact source, root path [], and mana selector"),
                       (lambda m:m.pop("pending_payment"),"stale",changed),
                       (lambda m:m["answer"].update(mana=["R"]),"stale",changed)):
            forged=deepcopy(native);change(forged)
            denied=self.a.engine.send({"cmd":"act","session_id":self.a.session_id,
                "request_id":self.a.state["request_id"],"player":0,"move":forged,"payment_expect":proof})
            self.assertEqual(denied,{"ok":False,"class":class_,"error":reason})
            self.assertEqual(self.a._reply_state(),frozen)
        inverse=copy(self.a)
        inverse.state,inverse.audit=deepcopy(self.a.state),deepcopy(self.a.audit)
        reply=self.a.act(3,"A",choice,no_random)
        self.assertEqual(reply["state"]["players"]["A"]["mana"],{})
        self.assertEqual(reply["state"]["zones"]["stack"],["bears"])
        source=self.a._engine_id("bears")
        self.assertTrue(self.a.audit["objects"][str(source)]["announced"]["cast"]["completed"])
        self.assertEqual([e["event"] for e in self.a.state["events"] if e["event"] in ("mana_added","cast")],["mana_added","cast"])
        # The capture retains raw before->pending->answer frames; none claims
        # that the initial cast quote had already completed.
        captured={"state":deepcopy(self.a.state),"audit":deepcopy(self.a.audit),
                  "payment_continuation":self.a.last_payment_continuation}
        self.assertEqual(native_action(inverse,native,captured,pending=frozen["state"]["pending"]),choice)
        mutations=[(lambda c:c.pop("before"),"needs its exact retained before, prefix and answer capture"),
            (lambda c:c["prefix"]["suspended"].update(source=999),"ordered payment slices do not reach the exact captured before frame"),
            (lambda c:c["answer"]["productions"][0].update(mana=["R"]),"output does not belong to the answered trigger invocation"),
            (lambda c:c["answer"]["payments"].clear(),"did not reach its retained root cost"),
            (lambda c:c["answer"]["move"]["pending_payment"].update(path=[0]),"does not bind this exact native move"),
            (lambda c:c["before"]["state"]["players"][0].update(pool={"G":9}),"disagrees with captured before/after pools")]
        for mutate,reason in mutations:
            tampered=deepcopy(captured);mutate(tampered["payment_continuation"])
            with self.subTest(reason=reason),self.assertRaisesRegex(Refusal,reason) as caught:
                native_action(inverse,native,tampered,pending=frozen["state"]["pending"])
            self.assertEqual((caught.exception.code,caught.exception.path),("unsupported_fact","do"))
        bad=copy(inverse);bad.state=deepcopy(inverse.state)
        bad.state["asked"]["source"]=999
        with self.assertRaisesRegex(Refusal,"ordered payment slices do not reach the exact captured before frame"):
            native_action(bad,native,captured,pending=frozen["state"]["pending"])
        with self.assertRaisesRegex(Refusal,"no decision is pending"):
            self.a.act(4,"A",choice,no_random)
        self.assertIsNotNone(self.a.last_payment_continuation)
        self.a.default(None,no_random)
        self.assertIsNone(self.a.last_payment_continuation,"an unrelated accepted pass has no continuation evidence")

    def test_second_sight_arranges_opponents_whole_private_group(self):
        # Full Oracle first mode; CR 401.2 and 701.20e keep this look private.
        names=['Forest','Island','Swamp','Mountain','Plains']
        cards={'sight':('Second Sight','A'), **{f'i{i}':('Island','A') for i in range(3)},
               **{f'b{i}':(n,'B') for i,n in enumerate(names)}}
        record=position_record(cards,{'A.hand':['sight'],'battlefield':[f'i{i}' for i in range(3)],
            'B.library':[f'b{i}' for i in range(5)]},MAIN_A,PRIORITY_A)
        self.a.setup(setup_of(record))
        self.a.act(0,'A',{'kind':'cast','card':'sight','modes':[1],'targets':[['B']],
            'pay':{'abilities':[{'source':f'i{i}','ability':{'rule':'305.6'},'adding':['U']} for i in range(3)]}},no_random)
        self.a.default(None,no_random); reply=self.a.default(None,no_random)
        pending=reply['state']['pending']
        self.assertEqual(pending,{'seat':'A','kind':'arrange','about':{'object':'sight','cards':[f'b{i}' for i in range(5)]}})
        for seat,visible in [('p0',True),('p1',False)]:
            prompt=self.a.engine.send({'cmd':'prompt','seat':seat})['prompt']
            seen={o.get('name') for o in prompt['objects'].values() if o.get('zone')=='library' and o.get('owner')=='p1'}
            for name in names:
                self.assertEqual(name in seen,visible,name)
        frozen=self.a._reply_state()
        for seat,top,away,reason in [('B',[f'b{i}' for i in range(5)],[],'request belongs to player 0, not 1'),
                ('A',['b0']*5,[],'order every offered card exactly once'),
                ('A',['b0'],[],'order every offered card exactly once'),
                ('A',['b0'],['b1','b2','b3','b4'],'puts every card back on top')]:
            with self.subTest(seat=seat,top=top,away=away),self.assertRaisesRegex(Refusal,reason):
                self.a.act(1,seat,{'kind':'arrange','top':top,'away':away},no_random)
            self.assertEqual(self.a._reply_state(),frozen)
        choice={'kind':'arrange','top':[f'b{i}' for i in reversed(range(5))],'away':[]}
        native=self.a._move(choice)
        inverse=copy(self.a); inverse.state=deepcopy(self.a.state); inverse.audit=deepcopy(self.a.audit)
        result=self.a.act(2,'A',choice,no_random)
        self.assertEqual(result['state']['zones']['B.library'],choice['top'])
        self.assertEqual(result['state']['zones']['A.library'],[])
        self.assertEqual(native_action(inverse,native,{'state':self.a.state,'audit':self.a.audit},pending=pending),choice)
        for marker in (None,{'object':999}):
            bad=copy(inverse);bad.state=deepcopy(inverse.state)
            bad.state['asked']['arrange']=marker
            with self.assertRaisesRegex(Refusal,"lacks its exact native instruction or permutation"):
                native_action(bad,native,{'state':self.a.state,'audit':self.a.audit},pending=pending)

    def test_ordinary_native_payment_keeps_its_preselected_spend_without_audit_opt_in(self):
        action=self.start()
        native=self.a._move(action)
        native['mana']={'G':1,'U':1}
        def act(move):
            result=self.a.engine.send({'cmd':'act','session_id':self.a.session_id,
                'request_id':self.a.state['request_id'],'player':0,'move':move})
            self.assertTrue(result['ok'],result)
            self.a._observe(result['state'])
        act(native)
        self.assertEqual(self.a.state['asked']['kind'],'mana')
        self.assertIsNone(self.a.engine.send({'cmd':'quote_payment','session_id':self.a.session_id,
            'request_id':self.a.state['request_id'],'player':0,'move':{'kind':'choose','answer':{'mana':['U']}}})['state'].get('continuation'))
        act({'kind':'choose','answer':{'mana':['U']}})
        self.assertEqual(self.a._reply_state()['state']['players']['A']['mana'],{})
        source=self.a._engine_id('bears')
        self.assertTrue(self.a.audit['objects'][str(source)]['announced']['cast']['completed'])

    def test_trigger_before_remaining_fixed_activation_retains_the_whole_authored_plan(self):
        # Centaur Courser costs 2G. Forest G, Fertile U and the later Forest G
        # pay exactly three mana; the later land must not activate before choice.
        action=self.start("Centaur Courser")
        action["pay"].pop("allocation")
        action["pay"]["abilities"].append({"source":"f4","ability":{"rule":"305.6"},"adding":["G"]})
        native,prefix=self.a._explicit_payment(0,self.a._move(action),action)
        self.assertEqual([r["path"] for r in prefix["payments"]],[[0]])
        self.assertEqual(native["plan"][1]["mana"],{})
        reply=self.a.act(1,"A",action,no_random)
        self.assertFalse(reply["state"]["objects"]["f4"].get("tapped",False))
        self.assertEqual(reply["state"]["players"]["A"]["mana"],{"G":1})
        inverse=copy(self.a);inverse.state=deepcopy(self.a.state);inverse.audit=deepcopy(self.a.audit)
        choice={"kind":"choose_mana","mana":["U"]}
        native,proof,_=self.a._continue_payment(0,self.a._move(choice))
        self.assertEqual([r["path"] for r in proof["payments"]],[[1],[]])
        self.assertEqual(native["pending_payment"]["mana"],{"U":1,"G":2})
        result=self.a.act(2,"A",choice,no_random)
        self.assertTrue(result["state"]["objects"]["f4"]["tapped"])
        self.assertEqual(result["state"]["players"]["A"]["mana"],{})
        self.assertEqual(native_action(inverse,native,{"state":self.a.state,"audit":self.a.audit,
            "payment_continuation":self.a.last_payment_continuation},pending=reply["state"]["pending"]),choice)

    def test_two_triggered_choices_remain_two_real_answers_before_one_final_spend(self):
        # Hill Giant costs 3R. Both enchanted Forests contribute G; their land
        # controller chooses U then R, for exactly GGUR. No earlier floating.
        action=self.start("Hill Giant",second=True)
        action["pay"].pop("allocation")
        action["pay"]["abilities"].append({"source":"f4","ability":{"rule":"305.6"},"adding":["G"]})
        self.a.act(2,"A",action,no_random)
        first={"kind":"choose_mana","mana":["U"]}
        native,proof,_=self.a._continue_payment(0,self.a._move(first))
        self.assertNotIn("pending_payment",native)
        self.assertEqual(proof["suspended"]["trigger"]["source"],self.a._engine_id("fertile2"))
        inverse=copy(self.a);inverse.state=deepcopy(self.a.state);inverse.audit=deepcopy(self.a.audit)
        pending=self.a._reply_state()["state"]["pending"]
        result=self.a.act(3,"A",first,no_random)
        self.assertEqual(result["state"]["pending"],{"seat":"A","kind":"choose_mana","about":{"object":"fertile2"}})
        self.assertEqual(result["state"]["players"]["A"]["mana"],{"G":2,"U":1})
        source=self.a._engine_id("bears")
        self.assertFalse(self.a.audit["objects"][str(source)]["announced"]["cast"]["completed"])
        captured={"state":deepcopy(self.a.state),"audit":deepcopy(self.a.audit),
            "payment_continuation":deepcopy(self.a.last_payment_continuation)}
        self.assertEqual(native_action(inverse,native,captured,pending=pending),first)
        final={"kind":"choose_mana","mana":["R"]}
        native,proof,_=self.a._continue_payment(0,self.a._move(final))
        self.assertEqual(native["pending_payment"]["mana"],{"U":1,"R":1,"G":2})
        inverse=copy(self.a);inverse.state=deepcopy(self.a.state);inverse.audit=deepcopy(self.a.audit)
        result=self.a.act(4,"A",final,no_random)
        self.assertEqual(result["state"]["players"]["A"]["mana"],{})
        self.assertTrue(self.a.audit["objects"][str(source)]["announced"]["cast"]["completed"])
        captured={"state":deepcopy(self.a.state),"audit":deepcopy(self.a.audit),
            "payment_continuation":deepcopy(self.a.last_payment_continuation)}
        self.assertEqual(native_action(inverse,native,captured,pending={"kind":"choose_mana"}),final)
        self.assertEqual(len(captured["payment_continuation"]["history"]),2)
        for change,reason in [(lambda c:c["history"].pop(0),"ordered payment slices must begin with the original cast"),
                (lambda c:c["history"].reverse(),"ordered payment slices must begin with the original cast"),
                (lambda c:c["history"][1]["quote"]["productions"][0].update(mana=["W"]),"output does not belong to the answered trigger invocation")]:
            tampered=deepcopy(captured);change(tampered["payment_continuation"])
            with self.subTest(reason=reason),self.assertRaisesRegex(Refusal,reason):
                native_action(inverse,native,tampered,pending={"kind":"choose_mana"})
