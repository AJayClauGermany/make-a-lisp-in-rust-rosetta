# The mal test suite

These files are copied unchanged from `impls/tests/` of
[kanaka/mal](https://github.com/kanaka/mal) at commit
[`2bbfaa54cca4`](https://github.com/kanaka/mal/tree/2bbfaa54cca4908efc90b4173b1406e260788e8a/impls/tests).
Mal is licensed under the MPL 2.0, the same license as this repository;
copyright (C) 2015 Joel Martin and the mal contributors.

The files are in `tests/`. `../conformance.rs` runs every registered row
against them with the same rules as mal's `runtest.py`: a form is sent, the
output up to the next prompt is matched against the `;=>` return value and
`;/` output patterns, and failures after `;>>> soft=True` are reported but do
not fail the step. The tests reference their fixtures as `../tests/<file>`,
so the runner starts the interpreter in `tests/`, where that path resolves
to the same directory.

To update, copy the files again from a newer commit and change the commit
above.
