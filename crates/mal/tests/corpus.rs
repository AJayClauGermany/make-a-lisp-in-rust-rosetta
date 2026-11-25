//! Every reader row reads every benchmark form to the same value as the
//! `std` row, so the reader table compares the same work, never an error
//! path, and the printer and evaluator fixtures (read with `std`) are what
//! every reader would have produced.

use mal::stack::{self, PickReader, ReaderRow};
use mal_bench::corpus;
use mal_traits::{ReadOutcome, Reader, Step};
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

struct ReadAll;

impl PickReader for ReadAll {
    type Output = Vec<Value>;
    fn reader<R: ReaderRow>(self) -> Vec<Value> {
        read_all(R::default())
    }
}

#[test]
fn readers_agree_on_the_corpus() {
    let expected = read_all(reader_std::StdReader);
    let mut checked = 0;
    for imp in stack::READERS {
        // lexpr cannot read mal syntax; its step column says so.
        if imp.name == "std" || !imp.supports(Step::ReadPrint) {
            continue;
        }
        let values = stack::pick_reader(imp.name, ReadAll).expect("listed rows can be picked");
        checked += 1;
        // `Value`'s `PartialEq` is mal's `=`, which ignores list/vector and
        // metadata differences; compare the debug form to be exact.
        assert_eq!(
            format!("{values:?}"),
            format!("{expected:?}"),
            "reader {} disagrees with std",
            imp.name
        );
    }
    assert!(checked >= 10, "only {checked} readers checked");
}
