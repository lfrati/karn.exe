"""Planetarium casts the inspected library card during resolution (608.2g).
Full Oracle and pinned rulings define free cost, timing and no land casting.
"""
import unittest
import test_free_cast_zone_review_client as base

HELPERS = {k: v for k, v in vars(base.FreeCastZoneReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class PlanetariumLibraryCastReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)

    def planetarium(self, top, *, off_turn=False):
        """The library is shuffled as the game starts, so it holds only that card; the scry
        keeps both on top."""
        self.new([{"battlefield": ["Planetarium of Wan Shi Tong", "Mountain", "Mountain"], "library": [top] * 5}, {"battlefield": ["Hill Giant"]}])
        self.main()
        if off_turn:
            self.advance(lambda state: state["turn"] == 2 and state["part"] == "PrecombatMain"
                         and state["active"] == 1 and state["asked"]["kind"] == "priority"
                         and state["asked"]["player"] == 0)
        planetarium = self.one("Planetarium of Wan Shi Tong")
        self.engine.act({"kind": "activate", "source": planetarium, "ability": 0, "mana": {"R": 1},
                         "targets": [], "plan": [{"source": o, "ability": 0} for o in self.ids("Mountain", zone="battlefield", controller=0)[:1]]})
        a = self.until_asked()
        self.assertEqual((a["kind"], a.get("verb")), ("arrange", "Scry"), a)
        self.engine.act({"kind": "choose", "answer": {"top": a["cards"], "away": []}})
        return self.until_asked()

    def test_planetarium_casts_the_top_card_of_the_library_free(self):
        self.planetarium("Lightning Bolt")
        offers = self.offered()
        self.assertEqual({self.view(0)["objects"][str(m)]["name"] for m in offers}, {"Lightning Bolt"})
        self.cast_offer(offers[0], [[{"player": 1}]])
        self.until_asked()
        self.assertEqual(self.state()["players"][1]["life"], 17)
        self.assertEqual(self.graveyard(), ["Lightning Bolt"], "no rider")
        self.assertEqual(self.state()["players"][0]["library"], 4)

    def test_planetarium_casts_a_creature_at_instant_speed(self):
        self.planetarium("Hill Giant", off_turn=True)
        self.assertEqual(self.state()["active"], 1)
        offers = self.offered()
        self.cast_offer(offers[0])
        self.until_asked()
        self.assertEqual(self.nonland(), ["Hill Giant", "Planetarium of Wan Shi Tong"])

    def test_planetarium_offers_nothing_for_a_land(self):
        a = self.planetarium("Forest")
        self.assertNotEqual(a["kind"], "cast", "a land is played, not cast (305.9)")
        self.assertEqual(self.state()["players"][0]["library"], 5)


if __name__ == "__main__":
    unittest.main()
