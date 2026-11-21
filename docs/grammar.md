# The grammar: how it is built, and how to find what it does not read

The grammar reads Oracle text into typed syntax (`crates/ir`) and prints it back
(`crates/grammar`). Reading every card is the first goal of the [roadmap](roadmap.md). This
document says how the grammar is extended without becoming a pile of patches, and how to find
what the unread cards lack.

## A rule is a way of saying a kind of thing

Every rule belongs to a part of the language, and each part has one owner:

| Part | What it says | Owner |
| --- | --- | --- |
| Vocabulary | types, colors, zones, counters, keywords, steps | `ir/vocab.rs`, `grammar/words.rs` |
| Noun phrases | which objects and players: kind, predicates, quantity, reference | `selector.rs` |
| Expressions | amounts, conditions, times | `amount.rs`, `condition.rs` |
| Lexicon | each action once, with the sorts of its roles | `ir/lexicon.rs`, `action.rs` |
| Constructions | do it, when it happens, instead of it, can't, must, may, has | `event.rs`, `statics.rs`, `becomes.rs` |
| Composition | sentences, joins, modifiers, choices, durations | `effect.rs`, `choice.rs`, `procedure.rs` |
| Abilities | spell text, activated, triggered, static, keyword rows | `ability.rs`, `cost.rs`, `keyword.rs` |

- **A change names the concept it adds.** A sort, an action, a predicate, a kind of amount, a
  construction: something the rules define, cited by rule number. A rule written so that one
  card's sentence passes is a patch. Do not write it.
- **A rule covers its family.** Before writing one, list every unread phrase that says the same
  kind of thing (below), read them together, and write the rule that covers them. A family
  with one member is still a concept if the rules name it ("End the turn", 724).
- **Meaning before wording.** Two spellings of one meaning are one value: one prints, the other
  is an alias. A distinction is kept only where the meaning or the scope differs.
- **Typed slots.** A slot takes what the action's rule allows and nothing else: "destroy target
  player" does not read. A new rule comes with the near misses it refuses.
- **Reading is not running.** What the grammar newly reads, the engine either runs correctly or
  refuses with a reason when the card is compiled. A card that newly runs gets rules tests.

Signs of a patch: the rule's name or comment mentions one card; a constructor serves one
sentence; the same meaning is read in two places; a slot's type was chosen from what cards
happen to put there; nothing new is refused.

## Finding what is missing

```sh
scripts/gates.sh                                         # the seven gates: no mismatch, no ambiguity
cargo build --locked --profile fast -p grammar --bin grammar-gaps
target/fast/grammar-gaps .cache/gaps.jsonl                # about 20 minutes
uv run scripts/grammar_gaps.py .cache/gaps.jsonl
uv run scripts/grammar_gaps.py GAPS --kind "an extra phrase" --grep 'instead of'
cargo run -p grammar --example try_ability -- "Destroy target creature."
```

The gates say where reading stopped, which is a word. `grammar-gaps` asks the grammar what is
missing there: for each unread row it finds the shortest span touching the stop that, removed
or replaced by a phrase of a known kind, lets the sentence read. That gives each gap a kind:

| Kind | Meaning | The work |
| --- | --- | --- |
| An extra phrase | the sentence reads without it | a modifier or rider the grammar lacks |
| A new way to say a noun phrase, amount, condition, event, instruction, duration or cost | the sentence reads with a known phrase of that kind in its place | that sub-grammar learns the phrase |
| A known phrase out of place | the phrase reads alone, but not where it stands | the sentence takes that kind of phrase there |
| A missing word | the sentence reads with a word added | an ellipsis ("chooses one", "the top card") |
| A whole sentence, or a sentence no edit explains | the frame itself is unknown | a construction, usually a rule the card states |

`scripts/grammar_gaps.py` weighs the gaps by roster main decks and, with `--kind` and `--grep`,
lists a family's members. Work is taken family by family: most decks first, the whole family at
once. After a merge the survey is rerun; its numbers are not kept in this document.

The text that is not read is rarely a whole sentence. It is usually one phrase, most often
inside a noun phrase: a zone said more freely, a place in an ordered zone, what happened to a
thing, a quantity word, a combat relation.
