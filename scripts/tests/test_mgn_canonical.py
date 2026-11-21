"""RFC 8785 canonicalization: the RFC's own samples and number table, then MGN's identities."""
import math
import struct
import unittest

from mgn.canonical import canonical, es_number, loads, sha256


def double(hex_bits):
    return struct.unpack(">d", bytes.fromhex(hex_bits))[0]


class CanonicalizationTests(unittest.TestCase):
    def test_rfc_sample(self):
        # RFC 8785 section 3.2.2 input and section 3.2.3 output
        text = ('{"numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],'
                ' "string": "\\u20ac$\\u000F\\u000aA\'\\u0042\\u0022\\u005c\\\\\\"\\/",'
                ' "literals": [null, true, false]}')
        expected = ('{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],'
                    '"string":"€$\\u000f\\nA\'B\\"\\\\\\\\\\"/"}')
        self.assertEqual(canonical(loads(text)), expected)

    def test_rfc_number_table(self):
        # RFC 8785 appendix B
        table = {"0000000000000000": "0", "8000000000000000": "0", "0000000000000001": "5e-324",
                 "7fefffffffffffff": "1.7976931348623157e+308", "4340000000000000": "9007199254740992",
                 "4430000000000000": "295147905179352830000", "44b52d02c7e14af6": "1e+23",
                 "444b1ae4d6e2ef50": "1e+21", "3eb0c6f7a0b5ed8d": "0.000001",
                 "41b3de4355555555": "333333333.3333333", "43143ff3c1cb0959": "1424953923781206.2"}
        for bits, text in table.items():
            with self.subTest(bits=bits):
                self.assertEqual(es_number(double(bits)), text)
                self.assertEqual(canonical(loads(text)), text)
        self.assertEqual(es_number(9.999999999999997e-7), "9.999999999999997e-7")
        self.assertEqual(es_number(1e16), "10000000000000000")
        with self.assertRaises(ValueError):
            es_number(float("nan"))
        with self.assertRaises(ValueError):
            es_number(float("inf"))

    def test_large_integer_tokens_without_silent_precision_loss(self):
        for value in (2**53 - 1, 2**53, -(2**53)):
            self.assertIs(type(loads(str(value))), int)
            self.assertEqual(loads(str(value)), value)
        for value in (2**53 + 2, -(2**53 + 2), 10**20, 295147905179352825856):
            parsed = loads(str(value))
            self.assertIs(type(parsed), float)
            self.assertEqual(int(parsed), value)
        for token in ("295147905179352830000", "-295147905179352830000"):
            # Appendix B's canonical spelling preserves bytes, despite not
            # being the exact decimal integer represented by this double.
            self.assertEqual(canonical(loads(token)), token)
        for token in (str(2**53 + 1), str(-(2**53 + 1)), "100000000000000000001", "1" + "0" * 400):
            with self.subTest(token=token), self.assertRaisesRegex(ValueError, "precision loss"):
                loads(token)
        for boundary in (float(2**53), 1e20, 1e21):
            for value in (boundary, -boundary, math.nextafter(boundary, -math.inf), math.nextafter(boundary, math.inf)):
                with self.subTest(value=value):
                    text = canonical(value)
                    self.assertEqual(canonical(loads(text)), text)
        # The new loading policy does not change direct Python integer validation.
        with self.assertRaises(ValueError):
            canonical(10**20)

    def test_rfc_sorting_by_utf16_code_units(self):
        # RFC 8785 section 3.2.3: the expected order of these member names
        value = {"€": "Euro Sign", "\r": "Carriage Return", "דּ": "Hebrew Letter Dalet With Dagesh",
                 "1": "One", "\U0001f600": "Emoji: Grinning Face", "\u0080": "Control",
                 "ö": "Latin Small Letter O With Diaeresis"}
        order = ["Carriage Return", "One", "Control", "Latin Small Letter O With Diaeresis", "Euro Sign",
                 "Emoji: Grinning Face", "Hebrew Letter Dalet With Dagesh"]
        text = canonical(value)
        positions = [text.index('"' + name + '"') for name in order]
        self.assertEqual(positions, sorted(positions))

    def test_escapes_and_rejections(self):
        self.assertEqual(canonical("a\u0001b\u001fc"), '"a\\u0001b\\u001fc"')
        self.assertEqual(canonical(["\x7f", "é"]), '["\x7f","é"]')
        self.assertEqual(canonical({"b": 1, "a": [True, None, False]}), '{"a":[true,null,false],"b":1}')
        with self.assertRaises(ValueError):
            loads('{"a": 1, "a": 2}')
        with self.assertRaises(ValueError):
            canonical(2 ** 53 + 1)
        with self.assertRaises(TypeError):
            canonical({1: "number keys are not JSON"})

    def test_identity_is_over_the_canonical_form(self):
        a = {"x": 1, "y": [1, 2], "z": {"q": "s"}}
        b = {"z": {"q": "s"}, "y": [1, 2], "x": 1}
        self.assertEqual(sha256(a), sha256(b))
        self.assertEqual(len(sha256(a)), 64)
        self.assertNotEqual(sha256(a), sha256({"x": 1, "y": [2, 1], "z": {"q": "s"}}))


if __name__ == "__main__":
    unittest.main()
