//! Generates the parser from `src/grammar.lalrpop` into `OUT_DIR`.

fn main() {
    lalrpop::process_src().unwrap();
}
