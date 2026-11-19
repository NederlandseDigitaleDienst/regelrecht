#!/usr/bin/env python3
"""Tests for wetsgeschiedenis.py.

Most tests run on a small fictional timeline built below, so they do not break
when the bundled dossiers are regenerated. The last test walks every article of
every bundled timeline once, to catch a data shape the fixture does not have.

    python3 .claude/skills/law-wetsgeschiedenis/wetsgeschiedenis.test.py
"""
import contextlib
import importlib.util
import io
import json
import os
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("wetsgeschiedenis", HERE / "wetsgeschiedenis.py")
wg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wg)

LID1 = ["1. Deze wet geldt voor A.", [0], 0, "lid"]
LID1_NEW = ["1. Deze wet geldt voor A en C.", [1], 0, "lid"]
LID2 = ["2. Zij geldt niet voor B, tenzij:", [0], 0, "lid"]
LID2_SUB = ["a. B daarom vraagt.", [0], 1, "onderdeel"]
HEAD = ["Artikel 1. Toepassing", [0], 0, "kop"]


def doc(nr, kind, date, **extra):
    return {"stuk": f"kst-12345-{nr}", "ondernummer": str(nr), "soort": kind, "titel": kind, "datum": date,
            "kamer": "Tweede Kamer", "speelt_mee": extra.pop("speelt_mee", False), "uitslag": None, "niet_mee": None, **extra}


def chunk(stuk, date, kind, text, leden=None):
    return {"stuk": stuk, "datum": date, "soort": kind, "rol": "artikelsgewijs", "kop": "Artikelsgewijs",
            "titel": "Artikel 1", "artikelen": ["1"], "leden": leden or {}, "ids": ["b1"], "doelartikelen": ["7"],
            "tekst": [[text, "al"]]}


def fixture():
    """A bill of one article. Nota 8 replays; amendment 10 (lid 2 lapses) does not,
    so the Staatsblad, which lacks lid 2, differs from the replay."""
    return {
        "dossier": "12345", "naam": "Testwet", "soort": "nieuwe wet", "voorstel": "kst-12345-2", "identiteit": {},
        "stukken": [doc(1, "Koninklijke boodschap", "2020-01-01"), doc(2, "Voorstel van wet", "2020-01-01", speelt_mee=True),
                    doc(3, "Memorie van toelichting", "2020-01-01"), doc(8, "Nota van wijziging", "2020-06-01", speelt_mee=True),
                    doc(10, "Amendement", "2020-09-01", speelt_mee=True)],
        "momenten": [{"soort": "stuk", "stuk": "kst-12345-1", "datum": "2020-01-01"},
                     {"soort": "stuk", "stuk": "kst-12345-2", "datum": "2020-01-01"},
                     {"soort": "stuk", "stuk": "kst-12345-3", "datum": "2020-01-01"},
                     {"soort": "stuk", "stuk": "kst-12345-8", "datum": "2020-06-01", "stappen": [1, 1]},
                     {"soort": "stuk", "stuk": "kst-12345-10", "datum": "2020-09-01"},
                     {"soort": "stemming", "datum": "2020-10-01", "stukken": ["kst-12345-10"], "stappen": [2, 2]}],
        "begin": {"blokken": {"b1": {"soort": "artikel", "nr": "1", "regels": [HEAD, LID1, LID2, LID2_SUB]}},
                  "volgorde": ["b1"]},
        "refs": [{"stuk": "kst-12345-2", "onderdeel": None, "sub": None},
                 {"stuk": "kst-12345-8", "onderdeel": "A", "sub": None}],
        "stappen": [
            {"stuk": "kst-12345-8", "onderdeel": "A", "sub": None, "soort": "vervang",
             "opdracht": "In artikel 1, eerste lid, wordt «A» vervangen door «A en C».", "status": "ok", "reden": "",
             "wijzig": {"b1": {"soort": "artikel", "nr": "1", "regels": [HEAD, LID1_NEW, LID2, LID2_SUB]}}, "weg": []},
            {"stuk": "kst-12345-10", "onderdeel": None, "sub": None, "soort": "vervalt",
             "opdracht": "Artikel 1, tweede lid, vervalt.", "status": "niet", "reden": "geen adres gevonden",
             "wijzig": {}, "weg": []},
        ],
        "ijkpunten": [{"naam": "Staatsblad 2021, 1", "soort": "staatsblad", "datum": "2021-01-01", "gelijk": 0,
                       "verschillend": 1, "artikelen": {"1": ["Artikel 1. Toepassing", "1. Deze wet geldt voor A en C."]}}],
        "commentaar": {
            "brokken": [chunk("kst-12345-3", "2020-01-01", "memorie", "Artikel 1 regelt waar de wet geldt."),
                        chunk("kst-12345-3", "2020-01-01", "memorie", "Het tweede lid sluit B uit.", {"1": [2]}),
                        chunk("kst-12345-10", "2020-09-01", "amendement", "Dit amendement schrapt het tweede lid.", {"1": [2]})],
            "niet": [{"stuk": "kst-12345-3", "reden": "artikel 9 bestaat in geen stand",
                      "tekst": "Zie ook de artikelen 9 en 1 over de reikwijdte."},
                     {"stuk": "kst-12345-3", "reden": "artikel 9 bestaat in geen stand", "tekst": "Alleen artikel 9 hier."}],
            "tel": {}},
        "tel": {"afgespeeld": 1, "niet": 1, "aanname": 0},
    }


def args(**kw):
    base = {"id": "12345", "nr": "1", "lid": None, "op": None, "na": None, "stap": None, "doelwet": None,
            "van": None, "tot": None, "herkomst": False}
    return Namespace(**{**base, **kw})


class WithFixture(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name) / "gegevens"
        d.mkdir()
        (d / "12345.json").write_text(json.dumps(fixture()), encoding="utf-8")
        self.env = {k: os.environ.get(k) for k in ("WETSGESCHIEDENIS_GEGEVENS", "WETSGESCHIEDENIS_CACHE")}
        os.environ["WETSGESCHIEDENIS_GEGEVENS"] = str(d)
        os.environ["WETSGESCHIEDENIS_CACHE"] = str(Path(self.tmp.name) / "cache")

    def write(self, data, ident="12345"):
        Path(os.environ["WETSGESCHIEDENIS_GEGEVENS"], f"{ident}.json").write_text(json.dumps(data), encoding="utf-8")

    def tearDown(self):
        for k, v in self.env.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        self.tmp.cleanup()


class Artikel(WithFixture):
    def test_text_at_a_date_before_the_nota_comes_from_the_bill(self):
        x = wg.cmd_artikel(args(op="2020-03-01"))
        self.assertEqual(x["regels"][1]["tekst"], LID1[0])
        self.assertEqual(x["regels"][1]["herkomst"][0]["verwijzing"], "Kamerstuk 12345, nr. 2")
        self.assertEqual(x["controle"]["oordeel"], "geen ijkpunt")

    def test_text_after_the_nota_names_the_nota_and_its_part(self):
        x = wg.cmd_artikel(args(na="kst-12345-8"))
        self.assertEqual(x["regels"][1]["tekst"], LID1_NEW[0])
        h = x["regels"][1]["herkomst"][0]
        self.assertEqual(h["verwijzing"], "Kamerstuk 12345, nr. 8, onderdeel A")
        self.assertEqual(h["url"], "https://zoek.officielebekendmakingen.nl/kst-12345-8")

    def test_a_replay_that_differs_from_the_staatsblad_says_so_and_gives_the_official_text(self):
        x = wg.cmd_artikel(args(op="2021-06-01"))
        self.assertEqual(x["controle"]["oordeel"], "wijkt af")
        self.assertEqual(x["controle"]["officieel"][-1], "1. Deze wet geldt voor A en C.")

    def test_lid_returns_the_lid_with_its_parts_and_nothing_else(self):
        x = wg.cmd_artikel(args(lid="2"))
        self.assertEqual([r["tekst"] for r in x["regels"]], [LID2[0], LID2_SUB[0]])

    def test_an_unknown_lid_names_the_leden_that_exist(self):
        with self.assertRaisesRegex(wg.Fault, r"geen lid 5 \(wel: 1, 2\)"):
            wg.cmd_artikel(args(lid="5"))

    def test_the_check_compares_lines_so_a_different_split_is_a_difference(self):
        data = fixture()
        data["stappen"][1]["status"] = "ok"
        data["stappen"][1]["wijzig"] = {"b1": {"soort": "artikel", "nr": "1", "regels": [HEAD, LID1_NEW]}}
        data["ijkpunten"][0]["artikelen"]["1"] = ["Artikel 1. Toepassing 1. Deze wet geldt voor A en C."]
        self.write(data)
        self.assertEqual(wg.cmd_artikel(args())["controle"]["oordeel"], "wijkt af")

    def test_text_output_shows_the_origin_chain_in_order(self):
        data = fixture()
        data["stappen"][0]["wijzig"]["b1"]["regels"][1] = ["1. Deze wet geldt voor A en C.", [0, 1], 0, "lid"]
        self.write(data)
        out = wg.render("artikel", wg.cmd_artikel(args()))
        self.assertIn("Kamerstuk 12345, nr. 2 → Kamerstuk 12345, nr. 8, onderdeel A (huidige formulering)", out)

    def test_article_number_may_be_written_with_the_word_artikel(self):
        self.assertEqual(wg.cmd_artikel(args(nr="Artikel 1"))["artikel"], "1")

    def test_an_article_that_does_not_exist_is_an_error_not_an_empty_answer(self):
        with self.assertRaisesRegex(wg.Fault, "artikel 4 bestaat niet"):
            wg.cmd_artikel(args(nr="4"))


class Herkomst(WithFixture):
    def test_lists_the_nota_that_replayed_and_the_amendment_that_did_not(self):
        x = wg.cmd_herkomst(args())
        steps = x["handelingen"]
        self.assertEqual([s["stuk"] for s in steps], ["kst-12345-8", "kst-12345-10"])
        self.assertEqual(steps[1]["status"], "niet afgespeeld")
        self.assertEqual(steps[1]["reden"], "geen adres gevonden")

    def test_an_amendment_takes_effect_at_the_vote_not_when_filed(self):
        amendment = wg.cmd_herkomst(args())["handelingen"][1]
        self.assertEqual((amendment["datum"], amendment["werkt_door_op"], amendment["bij"]),
                         ("2020-09-01", "2020-10-01", "stemming"))

    def test_versions_start_with_the_bill_and_change_only_when_the_text_does(self):
        v = wg.cmd_herkomst(args())["versies"]
        self.assertEqual([([c["verwijzing"] for c in x["door"]], x["datum"]) for x in v],
                         [(["Kamerstuk 12345, nr. 2"], "2020-01-01"), (["Kamerstuk 12345, nr. 8, onderdeel A"], "2020-06-01")])

    def test_an_inserted_article_is_cited_to_the_nota_that_inserted_it_not_to_the_bill(self):
        data = fixture()
        new = {"soort": "artikel", "nr": "1a", "regels": [["Artikel 1a. Nieuw", [1], 0, "kop"], ["Dit is nieuw.", [1], 0, "al"]]}
        data["stappen"][0]["wijzig"]["b2"] = new
        data["stappen"][0]["volgorde"] = ["b1", "b2"]
        self.write(data)
        v = wg.cmd_herkomst(args(nr="1a"))["versies"]
        self.assertEqual([([c["verwijzing"] for c in x["door"]], x["datum"]) for x in v],
                         [(["Kamerstuk 12345, nr. 8, onderdeel A"], "2020-06-01")])

    def test_a_version_made_at_the_vote_cites_the_amendment(self):
        data = fixture()
        lapse = data["stappen"][1]
        lapse["status"], lapse["wijzig"] = "ok", {"b1": {"soort": "artikel", "nr": "1", "regels": [HEAD, LID1_NEW]}}
        self.write(data)
        last = wg.cmd_herkomst(args())["versies"][-1]
        self.assertEqual((last["moment"], last["datum"], [c["stuk"] for c in last["door"]]),
                         ("stemming", "2020-10-01", ["kst-12345-10"]))
        # And with lid 2 gone the replay now equals the Staatsblad.
        self.assertEqual(wg.cmd_artikel(args())["controle"]["oordeel"], "gelijk")

    def test_a_lid_that_does_not_exist_is_an_error_not_an_empty_history(self):
        with self.assertRaisesRegex(wg.Fault, "geen lid 9"):
            wg.cmd_herkomst(args(lid="9"))

    def test_lid_history_warns_that_leden_are_followed_by_number(self):
        self.assertTrue(any("op zijn nummer" in n for n in wg.cmd_herkomst(args(lid="1"))["let_op"]))

    def test_versions_of_one_lid_ignore_changes_to_another_lid(self):
        v = wg.cmd_herkomst(args(lid="2"))["versies"]
        self.assertEqual(len(v), 1)

    def test_a_moment_hides_what_came_later(self):
        x = wg.cmd_herkomst(args(op="2020-03-01"))
        self.assertEqual(x["handelingen"], [])


class Toelichting(WithFixture):
    def test_lid_keeps_whole_article_passages_and_drops_passages_about_another_lid(self):
        x = wg.cmd_toelichting(args(lid="1"))
        self.assertEqual([t["over"] for t in x["toelichting"]], ["het hele artikel"])

    def test_lid_two_gets_its_own_passages(self):
        texts = [t["tekst"][0] for t in wg.cmd_toelichting(args(lid="2"))["toelichting"]]
        self.assertIn("Het tweede lid sluit B uit.", texts)
        self.assertIn("Dit amendement schrapt het tweede lid.", texts)

    def test_a_moment_leaves_out_what_was_not_yet_written(self):
        stukken = {t["stuk"] for t in wg.cmd_toelichting(args(op="2020-06-01"))["toelichting"]}
        self.assertEqual(stukken, {"kst-12345-3"})

    def test_unlinked_passages_that_name_the_article_are_reported(self):
        data = fixture()
        data["commentaar"]["niet"] += [
            {"stuk": "kst-12345-3", "reden": "r", "tekst": "Zie art. 1 voor de kern."},
            {"stuk": "kst-12345-3", "reden": "r", "tekst": "Artikel 1.2 is een ander artikel."},
            {"stuk": "kst-12345-3", "reden": "r", "tekst": "Zo ook artikel 10."}]
        self.write(data)
        loose = wg.cmd_toelichting(args())["niet_gekoppeld"]
        self.assertEqual([t["tekst"] for t in loose],
                         ["Zie ook de artikelen 9 en 1 over de reikwijdte.", "Zie art. 1 voor de kern."])

    def test_lid_passages_follow_the_number_the_passage_used_after_renumbering(self):
        # Article 1 becomes article 2; the passage still says "lid 2 of article 1".
        data = fixture()
        data["stappen"][0]["wijzig"]["b1"]["nr"] = "2"
        self.write(data)
        lid1 = wg.cmd_toelichting(args(nr="2", lid="1"))["toelichting"]
        self.assertEqual([t["tekst"][0] for t in lid1], ["Artikel 1 regelt waar de wet geldt."])
        lid2 = [t["over"] for t in wg.cmd_toelichting(args(nr="2", lid="2"))["toelichting"]]
        self.assertIn("lid 2 van artikel 1", lid2)

    def test_doelwet_with_lid_is_refused_rather_than_ignored(self):
        with self.assertRaisesRegex(wg.Fault, "--lid gaat niet samen met --doelwet"):
            wg.cmd_toelichting(args(nr=None, doelwet="7", lid="2"))

    def test_a_passage_hung_on_a_version_is_cited_by_its_kamerstuk(self):
        # In a law through time, the chain hangs a passage on the version a bill made,
        # and records the Kamerstuk it came from under `bron`.
        data = fixture()
        data["commentaar"]["brokken"][0]["bron"] = {"soort": "Kamerstuk", "stuk": "kst-54321-3", "datum": "2019-05-01"}
        Path(os.environ["WETSGESCHIEDENIS_GEGEVENS"], "12345.json").write_text(json.dumps(data), encoding="utf-8")
        first = wg.cmd_toelichting(args())["toelichting"][0]
        self.assertEqual((first["verwijzing"], first["datum"], first["url"], first["bij_versie"]),
                         ("Kamerstuk 54321, nr. 3", "2019-05-01",
                          "https://zoek.officielebekendmakingen.nl/kst-54321-3", "kst-12345-3"))

    def test_doelwet_finds_passages_about_the_amended_law(self):
        x = wg.cmd_toelichting(args(nr=None, doelwet="7"))
        self.assertEqual(len(x["toelichting"]), 3)
        self.assertTrue(x["doelwet"])


class Moments(WithFixture):
    def test_stap_zero_is_the_bill_and_stap_one_is_after_the_nota(self):
        self.assertEqual(wg.cmd_artikel(args(stap=0))["regels"][1]["tekst"], LID1[0])
        self.assertEqual(wg.cmd_artikel(args(stap=1))["regels"][1]["tekst"], LID1_NEW[0])

    def test_a_step_inside_a_document_is_described_as_that_step(self):
        data = fixture()
        data["momenten"][3]["stappen"] = [1, 2]
        del data["momenten"][5]["stappen"]
        self.write(data)
        self.assertIn("na handeling 1 van 2 (Kamerstuk 12345, nr. 8, onderdeel A)", wg.cmd_artikel(args(stap=1))["omschrijving"])

    def test_documents_on_the_same_day_are_told_apart(self):
        self.assertTrue(wg.cmd_artikel(args(na="kst-12345-3"))["omschrijving"].endswith("na Kamerstuk 12345, nr. 3"))

    def test_one_moment_only(self):
        with self.assertRaisesRegex(wg.Fault, "kies één moment"):
            wg.cmd_artikel(args(op="2020-03-01", stap=1))

    def test_a_step_out_of_range_says_the_range(self):
        with self.assertRaisesRegex(wg.Fault, "van 0 .* tot 2"):
            wg.cmd_artikel(args(stap=3))


class Wet(WithFixture):
    def test_the_whole_law_at_a_moment_with_the_check_per_article(self):
        x = wg.cmd_wet(args(id="12345"))
        self.assertEqual([b["nr"] for b in x["blokken"]], ["1"])
        self.assertEqual((x["controle"]["ijkpunt"], x["controle"]["wijkt_af"]), ("Staatsblad 2021, 1", ["1"]))

    def test_before_the_checkpoint_there_is_nothing_to_check_against(self):
        x = wg.cmd_wet(args(id="12345", op="2020-03-01"))
        self.assertIsNone(x["controle"]["ijkpunt"])
        self.assertEqual(x["blokken"][0]["regels"][1]["tekst"], LID1[0])

    def test_herkomst_flag_prints_the_origin_in_the_text(self):
        wg.show_origin = True
        try:
            out = wg.render("wet", wg.cmd_wet(args(id="12345")))
        finally:
            wg.show_origin = False
        self.assertIn("⟨Kamerstuk 12345, nr. 8, onderdeel A⟩", out)


class Vergelijk(WithFixture):
    def test_a_changed_line_is_shown_word_by_word_with_the_document_that_changed_it(self):
        x = wg.cmd_vergelijk(args(van="stap:0", tot="stap:1"))
        (c,) = x["wijzigingen"]
        self.assertEqual((c["wat"], [d["verwijzing"] for d in c["door"]]), ("gewijzigd", ["Kamerstuk 12345, nr. 8, onderdeel A"]))
        self.assertEqual(c["regels"], [{"soort": "gewijzigd", "oud": LID1[0], "nieuw": LID1_NEW[0],
                                        "verschil": "1. Deze wet geldt voor [-A.-] {+A en C.+}"}])

    def test_moments_may_be_dates_documents_or_steps(self):
        by_date = wg.cmd_vergelijk(args(van="2020-03-01", tot="2020-07-01"))["wijzigingen"]
        by_doc = wg.cmd_vergelijk(args(van="kst-12345-3", tot="kst-12345-8"))["wijzigingen"]
        self.assertEqual(by_date, by_doc)

    def test_nothing_changed_is_an_empty_answer(self):
        self.assertEqual(wg.cmd_vergelijk(args(van="stap:1", tot="stap:2"))["wijzigingen"], [])

    def test_an_inserted_article_is_new(self):
        data = fixture()
        data["stappen"][0]["wijzig"]["b2"] = {"soort": "artikel", "nr": "1a", "regels": [["Artikel 1a. Nieuw", [1], 0, "kop"]]}
        data["stappen"][0]["volgorde"] = ["b1", "b2"]
        self.write(data)
        kinds = {c["artikel"]: c["wat"] for c in wg.cmd_vergelijk(args(nr=None, van="stap:0", tot="stap:1"))["wijzigingen"]}
        self.assertEqual(kinds, {"1": "gewijzigd", "1a": "nieuw"})

    def test_a_renumbered_article_is_one_change_that_names_its_old_number(self):
        data = fixture()
        data["stappen"][0]["wijzig"]["b1"]["nr"] = "2"
        self.write(data)
        (c,) = wg.cmd_vergelijk(args(nr=None, van="stap:0", tot="stap:1"))["wijzigingen"]
        self.assertEqual((c["artikel"], c["was"], c["wat"]), ("2", "1", "gewijzigd"))

    def test_an_inserted_lid_is_new_and_the_leden_after_it_are_renumbered_not_rewritten(self):
        old = ["1. Eerste lid blijft.", "2. Het oude tweede lid over het beheer van documenten.", "3. Bij AMvB worden regels gesteld."]
        new = ["1. Eerste lid blijft.", "2. Een geheel nieuw lid dat iets anders regelt dan alles hiervoor.",
               "3. Het oude tweede lid over het beheer van documenten.", "4. Bij AMvB worden regels gesteld."]
        self.assertEqual([(d["soort"], d.get("nieuw", d.get("oud"))[:2]) for d in wg.line_diff(old, new)],
                         [("nieuw", "2."), ("vernummerd", "3."), ("vernummerd", "4.")])

    def test_a_removed_lid_is_removed_and_a_changed_one_is_diffed_against_itself(self):
        old = ["3. In uitzonderlijke gevallen kan iets worden beperkt.", "4. Het vierde lid over milieu-informatie.",
               "5. Een lid dat een woord verandert."]
        new = ["3. Het vierde lid over milieu-informatie.", "4. Een lid dat twee woorden verandert."]
        self.assertEqual([d["soort"] for d in wg.line_diff(old, new)], ["weg", "vernummerd", "gewijzigd"])
        self.assertEqual(wg.line_diff(old, new)[2]["verschil"], "[-5.-] {+4.+} Een lid dat [-een woord-] {+twee woorden+} verandert.")

    def test_bwb_lid_labels_without_a_period_are_recognised_as_renumbering(self):
        self.assertEqual([d["soort"] for d in wg.line_diff(["2 Tekst van het lid."], ["3 Tekst van het lid."])], ["vernummerd"])

    def test_an_article_missing_from_the_official_text_is_flagged_whatever_the_moment(self):
        data = fixture()
        data["stappen"][0]["wijzig"]["b2"] = {"soort": "artikel", "nr": "13", "regels": [["Artikel 13. Van een andere wet", [1], 0, "kop"]]}
        data["stappen"][0]["volgorde"] = ["b1", "b2"]
        self.write(data)
        change = [c for c in wg.cmd_vergelijk(args(nr=None, van="stap:0", tot="stap:1"))["wijzigingen"] if c["artikel"] == "13"][0]
        self.assertIs(change["in_ijkpunt"], False)
        self.assertIn("niet in de officiële eindtekst", wg.render("vergelijk", {"dossier": "12345", "artikel": None, "van": "", "tot": "",
                                                                              "wijzigingen": [change]}))
        self.assertTrue(any("staat niet in Staatsblad 2021, 1" in n for n in wg.cmd_artikel(args(nr="13", stap=1))["let_op"]))

    def test_from_after_to_is_refused(self):
        with self.assertRaisesRegex(wg.Fault, "ligt na --tot"):
            wg.cmd_vergelijk(args(van="stap:2", tot="stap:0"))


class Errors(WithFixture):
    def run_cli(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = wg.main(list(argv))
        return code, out.getvalue(), err.getvalue()

    def test_an_invalid_id_is_refused_before_it_becomes_a_file_name(self):
        code, _, err = self.run_cli("artikel", "../etc/passwd", "1")
        self.assertEqual(code, 1)
        self.assertIn("geen dossiernummer", err)

    def test_a_missing_timeline_says_how_to_fetch_it(self):
        code, _, err = self.run_cli("artikel", "54321", "1")
        self.assertEqual(code, 1)
        self.assertIn("haal 54321", err)

    def test_a_malformed_date_is_refused(self):
        self.assertEqual(self.run_cli("artikel", "12345", "1", "--op", "1-1-2020")[0], 2)

    def test_json_output_parses(self):
        code, out, _ = self.run_cli("--json", "herkomst", "12345", "1")
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(out)["artikel"], "1")

    def test_text_output_states_the_check(self):
        code, out, _ = self.run_cli("artikel", "12345", "1")
        self.assertEqual(code, 0)
        self.assertIn("wijkt af van Staatsblad 2021, 1", out)

    def test_quality_counts_come_from_the_timeline(self):
        code, out, _ = self.run_cli("stukken", "12345")
        self.assertIn("Afspeling: 1 afgespeeld, 0 afgespeeld met aanname, 1 niet afgespeeld.", out)

    def test_a_corrupt_timeline_is_an_error_not_a_traceback(self):
        Path(os.environ["WETSGESCHIEDENIS_GEGEVENS"], "12345.json").write_text("{", encoding="utf-8")
        code, _, err = self.run_cli("artikel", "12345", "1")
        self.assertEqual(code, 1)
        self.assertIn("geen leesbare tijdlijn", err)

    def test_fetch_options_that_would_be_ignored_are_refused_before_any_network(self):
        for argv, msg in ((["haal", "BWBR0000001", "--alles"], "--alles hoort bij --keten"),
                          (["haal", "BWBR0000001", "12345", "--keten"], "geen dossiernummer"),
                          (["haal", "12345", "--keten"], "BWB-id"),
                          (["haal", "BWBR0000001", "--opbouw"], "nog geen keten")):
            with self.subTest(argv=argv):
                code, _, err = self.run_cli(*argv)
                self.assertEqual(code, 1)
                self.assertIn(msg, err)


def chain_fixture():
    """A law through time in BWB shape: lid lines without a period, the oldest
    version sharing its id with the act that made it, a repeal and a pending bill."""
    head = ["Artikel 1", [0], 0, "kop"]
    lid1, lid2 = ["1 Deze wet geldt.", [0], 0, "lid"], ["2 Zij geldt ook hier.", [0], 0, "lid"]
    lid2_new = ["2 Zij geldt ook daar.", [1], 0, "lid"]

    def version(stuk, kind, title, date, order, nr="–"):
        return {"stuk": stuk, "ondernummer": nr, "soort": kind, "titel": title, "datum": date, "volgorde": order,
                "kamer": "", "speelt_mee": False, "uitslag": None, "niet_mee": None}
    return {
        "dossier": "BWBR0000001", "naam": "Testwet door de tijd", "soort": "keten", "voorstel": "versie-2010-01-01",
        "identiteit": {}, "afgekapt": None,
        "stukken": [version("versie-2005-01-01", "Nieuwe wet", "Wet van 2004", "2005-01-01", 1, "11111"),
                    version("versie-2010-01-01", "Versie van de wet", "Testwet, zoals zij gold op 2010-01-01", "2010-01-01", 0),
                    version("versie-2010-01-01", "Wijzigingswet", "Wet van 2009, Stb. 2009, 1", "2010-01-01", 2, "22222"),
                    version("versie-2015-01-01", "Wijzigingswet", "Wet van 2014, Stb. 2014, 9", "2015-01-01", 3, "33333"),
                    version("aanhangig-44444", "Aanhangig voorstel", "Kamerdossier 44444, aanhangig; raakt artikel 2", None, 4, "44444"),
                    version("intrekking-stb-2020-5", "Intrekking", "De wet wordt ingetrokken: stb-2020-5", "2020-01-01", 5, "55555")],
        "momenten": [{"soort": "stuk", "stuk": "versie-2005-01-01", "datum": "2005-01-01"},
                     {"soort": "stuk", "stuk": "versie-2010-01-01", "datum": "2010-01-01"},
                     {"soort": "stuk", "stuk": "versie-2015-01-01", "datum": "2015-01-01", "stappen": [1, 1]},
                     {"soort": "stuk", "stuk": "intrekking-stb-2020-5", "datum": "2020-01-01"}],
        "begin": {"blokken": {"artikel:1": {"soort": "artikel", "nr": "1", "regels": [head, lid1, lid2]}},
                  "volgorde": ["artikel:1"]},
        "refs": [{"stuk": "versie-2010-01-01", "onderdeel": None, "sub": None},
                 {"stuk": "versie-2015-01-01", "onderdeel": None, "sub": None}],
        "stappen": [{"stuk": "versie-2015-01-01", "onderdeel": "33333", "sub": None, "soort": "versie",
                     "opdracht": "Wijzigingswet 33333: 1 artikel gewijzigd", "status": "ok", "reden": "", "ronde": 1,
                     "wijzig": {"artikel:1": {"soort": "artikel", "nr": "1", "regels": [head, lid1, lid2_new]}}, "weg": [],
                     "bronnen": {"dossiers": [{"nr": "33333", "status": "niet opgehaald"}], "publicaties": ["stb-2014-9"]}}],
        "ijkpunten": [{"naam": "De geldende tekst (2015-01-01)", "soort": "bwb", "datum": "2015-01-01", "gelijk": 1,
                       "verschillend": 0, "artikelen": {"1": ["Artikel 1", "1 Deze wet geldt.", "2 Zij geldt ook daar."]}}],
        "commentaar": {"brokken": [], "niet": [], "tel": {}},
        "tel": {"afgespeeld": 1, "niet": 0, "aanname": 0},
    }


class Chain(WithFixture):
    def setUp(self):
        super().setUp()
        self.write(chain_fixture(), "BWBR0000001")

    def a(self, **kw):
        return args(id="BWBR0000001", **kw)

    def test_the_oldest_text_is_cited_as_the_bwb_text_not_as_the_act_sharing_its_id(self):
        line = wg.cmd_artikel(self.a(op="2012-01-01"))["regels"][1]["herkomst"][0]
        self.assertIn("de tekst zoals op wetten.overheid.nl op 2010-01-01", line["verwijzing"])
        self.assertNotIn("Wet van 2009", line["verwijzing"])

    def test_a_date_before_the_oldest_text_is_refused_not_answered_with_a_newer_text(self):
        with self.assertRaisesRegex(wg.Fault, "begint bij 2010-01-01"):
            wg.cmd_artikel(self.a(op="2006-01-01"))

    def test_bwb_leden_without_a_period_are_leden(self):
        self.assertEqual([r["tekst"] for r in wg.cmd_artikel(self.a(lid="2"))["regels"]], ["2 Zij geldt ook daar."])

    def test_a_repeal_is_said_and_a_pending_bill_only_for_the_articles_it_touches(self):
        notes = wg.cmd_artikel(self.a())["let_op"]
        self.assertTrue(any(n.startswith("ingetrokken") for n in notes))
        self.assertFalse(any(n.startswith("aanhangig") for n in notes))
        self.assertEqual(wg.cmd_artikel(self.a(op="2016-01-01"))["let_op"], [])

    def test_the_first_version_is_dated_at_the_oldest_text_and_the_change_cites_its_act(self):
        v = wg.cmd_herkomst(self.a())["versies"]
        self.assertEqual([x["datum"] for x in v], ["2010-01-01", "2015-01-01"])
        self.assertEqual(v[1]["door"][0]["verwijzing"], "Wet van 2014, Stb. 2014, 9")

    def test_a_version_is_the_bwb_text_itself_not_a_replay(self):
        self.assertEqual(wg.cmd_artikel(self.a(op="2012-01-01"))["controle"]["oordeel"], "bwb")
        self.assertTrue(wg.cmd_wet(self.a(op="2012-01-01"))["controle"]["bwb"])

    def test_comparing_versions_names_the_act_and_the_staatsblad_check(self):
        (c,) = wg.cmd_vergelijk(self.a(van="2012-01-01", tot="2016-01-01"))["wijzigingen"]
        self.assertEqual(c["door"][0]["verwijzing"], "Wet van 2014, Stb. 2014, 9")
        self.assertEqual(c["door"][0]["status"], "Staatsblad nagespeeld")
        self.assertEqual(c["regels"][0]["verschil"], "2 Zij geldt ook [-hier.-] {+daar.+}")

    def test_the_fetch_hint_includes_the_rebuild(self):
        out = wg.render("herkomst", wg.cmd_herkomst(self.a()))
        self.assertIn("haal BWBR0000001 33333, daarna wetsgeschiedenis.py haal BWBR0000001 --opbouw", out)


@unittest.skipUnless(wg.BUNDLED.is_dir(), "no bundled timelines in this checkout")
class Bundled(unittest.TestCase):
    """Every article of every bundled timeline answers every question once."""

    def test_every_article_answers_and_its_first_version_cites_where_it_began(self):
        for path in sorted(wg.BUNDLED.glob("*.json")):
            if not wg.valid_id(path.stem):
                continue
            h = wg.History(json.loads(path.read_text(encoding="utf-8")))
            # The whole law, and everything that changed from the first text to the last.
            wg.render("wet", wg.cmd_wet(args(id=path.stem)))
            changed = wg.cmd_vergelijk(args(id=path.stem, nr=None, van="stap:0"))["wijzigingen"]
            wg.render("vergelijk", {"dossier": path.stem, "artikel": None, "van": "", "tot": "", "wijzigingen": changed})
            self.assertEqual(bool(changed), any(s["wijzig"] or s["weg"] for s in h.steps), path.stem)
            blocks, order = h.state(h.positions[-1]["steps"])
            keys = [k for k in order if blocks[k]["soort"] == "artikel"]
            self.assertTrue(keys, path.name)
            for k in keys:
                nr = blocks[k]["nr"]
                with self.subTest(timeline=path.stem, artikel=nr):
                    a = args(id=path.stem, nr=nr)
                    for cmd in (wg.cmd_artikel, wg.cmd_herkomst, wg.cmd_toelichting):
                        wg.render(cmd.__name__[4:], cmd(a))
                    first = wg.cmd_herkomst(a)["versies"][0]["door"]
                    in_bill = h.article_by_identity(0, h.identity(k)) is not None
                    # The bill is the source exactly when the article was in it.
                    self.assertEqual(any(c["stuk"] == h.d["voorstel"] for c in first), in_bill, first)
                    leden = [n for n, _ in wg.members(blocks[k]) if n]
                    if leden:
                        b = args(id=path.stem, nr=nr, lid=leden[-1])
                        self.assertTrue(wg.cmd_artikel(b)["regels"])
                        wg.cmd_herkomst(b)
                        wg.cmd_toelichting(b)


if __name__ == "__main__":
    unittest.main()
