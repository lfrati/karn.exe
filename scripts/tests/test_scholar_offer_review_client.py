"""Scholar's resolving cast permission (CR 608.2g) and exile rider, through the client."""
import unittest
import test_toxic_review_client as base

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class ScholarOfferReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)

    def test_scholar_offers_its_free_cast_and_the_rider_exiles_it(self):
        self.new([{"hand": ["Scholar of the Lost Trove"], "graveyard": ["Unsummon"], "battlefield": ["Island"] * 7},
                  {"battlefield": ["Grizzly Bears"]}])
        self.main()
        bears = self.one("Grizzly Bears")
        self.cast("Scholar of the Lost Trove", mana=self.ids("Island", zone="battlefield", controller=0)[:7], spend={'U': 7})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        unsummon = self.ids("Unsummon")[0]
        self.assertEqual(self.asked()["kind"], "targets")
        self.engine.act({"kind": "targets", "targets": [[{"object": unsummon}]]})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        asked = self.asked()
        self.assertEqual((asked["kind"], asked["player"]), ("cast", 0), asked)
        self.assertEqual(asked["cards"], [unsummon])
        self.engine.act({"kind": "cast", "card": unsummon, "route": "normal", "plan": [], "targets": [[{"object": bears}]]})
        for _ in range(4):
            if not self.engine.state["stack"]:
                break
            self.engine.act({"kind": "pass"})
        self.assertNotIn(bears, self.engine.state["battlefield"])
        self.assertEqual([o for o in self.engine.state["exile"] if self.obj(o)["name"] == "Unsummon"].__len__(), 1)


if __name__ == "__main__":
    unittest.main()
