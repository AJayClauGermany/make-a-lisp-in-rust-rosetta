//! Every reader row reads every benchmark form to the same value as the
//! `std` row, so the reader table compares the same work, never an error
//! path, and the printer and evaluator fixtures (read with `std`) are what
//! every reader would have produced.

use mal_bench::corpus;
use mal_traits::{ReadOutcome, Reader};
use mal_types::Value;

fn read_all<R: Reader<Value = Value>>(mut reader: R) -> Vec<Value> {
    corpus::FORMS
        .iter()
        .chain(corpus::PROGRAMS)
        .map(|form| match reader.read_str(form.source) {
            Ok(ReadOutcome::Form(value)) => value,
            Ok(ReadOutcome::Empty) => panic!("{} read as Empty", form.name),
            Err(e) => panic!("{} failed to read: {e}", form.name),
        })
        .collect()
}

#[test]
fn readers_agree_on_the_corpus() {
    let expected = read_all(reader_std::StdReader);
    let rows = [
        ("nom", read_all(reader_nom::NomReader)),
        ("winnow", read_all(reader_winnow::WinnowReader)),
        ("chumsky", read_all(reader_chumsky::ChumskyReader)),
        ("pest", read_all(reader_pest::PestReader)),
        ("peg", read_all(reader_peg::PegReader)),
        (
            "lalrpop",
            read_all(reader_lalrpop::LalrpopReader::default()),
        ),
        ("logos", read_all(reader_logos::LogosReader)),
        ("regex", read_all(reader_regex::RegexReader)),
    ];
    for (name, values) in rows {
        // `Value`'s `PartialEq` is mal's `=`, which ignores list/vector and
        // metadata differences; compare the debug form to be exact.
        assert_eq!(
            format!("{values:?}"),
            format!("{expected:?}"),
            "reader {name} disagrees with std"
        );
    }
}
