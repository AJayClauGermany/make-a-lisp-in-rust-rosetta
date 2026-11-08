# The Make-a-Lisp steps

The [mal process guide](https://github.com/kanaka/mal/blob/master/process/guide.md)
builds the interpreter in eleven cumulative steps. Each has a test file in the
mal repository, and `mal_traits::Step` names each one. The table shows which
components a step first exercises, which is what decides where a new feature
lands in this workspace.

| step | name | adds | components exercised |
|------|------|------|----------------------|
| 0 | `step0_repl` | prompt, read a line, print it back | line editor |
| 1 | `step1_read_print` | tokenising, parsing into values, printing them | reader, printer |
| 2 | `step2_eval` | evaluating symbols and function calls | evaluator |
| 3 | `step3_env` | `def!`, `let*`, nested environments | env |
| 4 | `step4_if_fn_do` | `if`, `do`, `fn*`, the core namespace | evaluator, core |
| 5 | `step5_tco` | tail-call optimisation | evaluator |
| 6 | `step6_file` | `read-string`, `slurp`, `eval`, atoms, `load-file` | core, reader |
| 7 | `step7_quote` | `quote`, `quasiquote`, `cons`, `concat`, `vec` | evaluator, reader |
| 8 | `step8_macros` | `defmacro!`, macro expansion, `nth`, `first`, `rest` | evaluator |
| 9 | `step9_try` | `try*`/`catch*`, `throw`, `apply`, `map`, predicates | evaluator, core |
| A | `stepA_mal` | `readline`, metadata, `*host-language*`, self-hosting | all |

Because the steps are cumulative, a row that reaches step 8 has passed the
tests for steps 0 through 8. The `Step` enum is ordered the same way, so
`Step::Macros > Step::Quoting`.
