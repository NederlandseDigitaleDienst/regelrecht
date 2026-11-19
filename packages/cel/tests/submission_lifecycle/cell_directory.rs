//! The cell's directory: the reductions it keeps and the catalogue of the
//! answers it gives.

use super::*;

#[test]
fn a_cell_directory_that_is_not_there_is_refused() {
    let e = Cell::in_memory(RECORDING_ACTOR, cell_dir().join("elders")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("no directory of the cell")),
        "{e}"
    );
}

#[test]
fn a_cell_reads_the_reductions_it_keeps_from_its_directory() {
    let cell = Cell::in_memory(RECORDING_ACTOR, cell_dir()).unwrap();
    assert_eq!(
        cell.reductions()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "reductie_plaats_van_vestiging",
            "reductie_verlof",
            REDUCTION,
            "reductie_verlof_valt_buiten_schoolvakanties"
        ]
    );
}

#[test]
fn a_cell_keeps_each_reduction_as_a_directory_with_its_versions() {
    // `reductions/<$id>/<valid_from>.yaml`, and nothing else.
    for make in [
        |p: &Path| std::fs::write(p.join("los.yaml"), "").unwrap(),
        |p: &Path| std::fs::create_dir(p.join("reductie_zonder_versie")).unwrap(),
        |p: &Path| {
            std::fs::create_dir(p.join("reductie_x")).unwrap();
            std::fs::write(p.join("reductie_x/notities.txt"), "").unwrap();
        },
        |p: &Path| {
            std::fs::create_dir(p.join("reductie_x")).unwrap();
            std::fs::write(p.join("reductie_x/eerste.yaml"), "").unwrap();
        },
    ] {
        let dir = lid_2_dir();
        make(&dir.path().join("reductions"));
        let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
        assert!(
            matches!(&e, Error::Setup(m) if m.contains("a file per version")),
            "{e}"
        );
    }
}

#[test]
fn an_answer_from_a_reduction_the_cell_does_not_keep_is_refused() {
    let dir = cell_tempdir();
    std::fs::write(
        dir.path().join("cel.yaml"),
        format!("name: Het hoofd van OBS De Klimop\nanswers:\n  {ONCE}: {REDUCTION}\n"),
    )
    .unwrap();
    let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains(REDUCTION)
            && m.contains("only the answers of its own reductions")),
        "{e}"
    );
}

#[test]
fn the_reductions_of_no_cell_are_no_cell() {
    // The corpus holds no reduction of no cell any more; the directory
    // alone is enough.
    let root = common::cell_tempdir();
    let shared = root.path().join("cells/reductions");
    std::fs::create_dir_all(&shared).unwrap();
    let e = Cell::in_memory(RECORDING_ACTOR, &shared).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("reductions of no cell")),
        "{e}"
    );
}

#[test]
fn a_cell_lists_the_answers_it_gives() {
    // The school's own cell: five answers from three reductions.
    let own = Cell::in_memory(RECORDING_ACTOR, cell_dir()).unwrap();
    let listed: Vec<(&str, &str)> = own
        .answers()
        .iter()
        .map(|(answer, reduction)| (answer.as_str(), reduction.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            (
                "deel_van_gemeente_van_vestiging",
                "reductie_plaats_van_vestiging"
            ),
            ("gemeente_van_vestiging", "reductie_plaats_van_vestiging"),
            ("provincie_van_vestiging", "reductie_plaats_van_vestiging"),
            (ONCE, REDUCTION),
            (
                OUTSIDE_HOLIDAYS,
                "reductie_verlof_valt_buiten_schoolvakanties"
            ),
        ]
    );

    // A cell without answers gives none.
    assert!(without_reductions().answers().is_empty());
}

#[test]
fn a_catalogue_that_names_an_empty_answer_or_reduction_is_refused() {
    for answers in [[("''", REDUCTION)], [(ONCE, "''")]] {
        let dir = answers_dir(&answers);
        let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
        assert!(
            matches!(&e, Error::Setup(m) if m.contains("neither may be empty")),
            "{e}"
        );
    }
    let dir = lid_2_dir();
    common::write_identity(
        dir.path(),
        "name: Het hoofd van OBS De Klimop\nantwoorden: {}\n",
    );
    let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("antwoorden")),
        "{e}"
    );
}

/// The reduction of the Awb is of no cell: a reduction calls it, and no
/// cell gives its output as an answer, not even the cell of a body that
/// applies the Awb. A catalogue that names it is refused when the cell
/// opens.
#[test]
fn no_cell_gives_a_reduction_of_no_cell_as_its_answer() {
    const AWB_REDUCTION: &str = "reductie_algemene_wet_bestuursrecht";
    let dir = answers_dir(&[("besluiten_in_stand", AWB_REDUCTION)]);
    let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains(AWB_REDUCTION)
            && m.contains("only the answers of its own reductions")),
        "{e}"
    );
}
