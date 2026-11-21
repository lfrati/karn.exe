//! Fixtures of the test tooling itself, not rules tests: the rule evidence scanner skips this
//! folder and ignores its receipts (scripts/rule_evidence.py). Each fixture does nothing unless
//! a tool test asks for it (scripts/tests/test_rule_evidence_fixtures.py), except a contract of
//! `cite!` that holds on every run and must not be counted as rule evidence.

mod cite_fixtures;
