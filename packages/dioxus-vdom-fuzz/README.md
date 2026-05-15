# Dioxus VirtualDom Repro Corpus

This crate carries minimized `VirtualDom` regression cases extracted from the
coverage-guided fuzzer on the fix branch. On `main`, several tests are expected
to fail because they reproduce existing core diffing bugs.

Run the corpus with:

```sh
cargo test -p dioxus-vdom-fuzz
```

The crate intentionally keeps only the deterministic replay tests in this repro
branch. The randomized runner used to discover these cases is not required for
reproducing the failures.
