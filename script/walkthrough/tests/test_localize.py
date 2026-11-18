from walkthrough.localize import is_sentence, localize_slide, localize_typing, parse_vtt, write_captions
from walkthrough.timeline import to_vtt


def test_a_slide_takes_every_translated_text_and_names_what_is_missing():
    slide = {"kind": "demo", "route": "/regelwerken", "title": "Wetten hangen samen", "bullets": ["Volg de lijn.", "Nog niet vertaald"]}
    out, missing = localize_slide(slide, {"Wetten hangen samen": "Laws are connected", "Volg de lijn.": "Follow the line."})
    assert out == {"kind": "demo", "route": "/regelwerken", "title": "Laws are connected", "bullets": ["Follow the line.", "Nog niet vertaald"]}
    assert missing == ["Nog niet vertaald"]
    assert slide["title"] == "Wetten hangen samen"


def test_a_typed_sentence_fills_in_its_translation_at_the_presenters_pace():
    target = [{"tag": "textarea"}]
    events = [{"t": i, "type": "input", "target": target, "value": v} for i, v in enumerate(["D", "Dit k", "Dit klopt niet"])]
    events.append({"t": 9, "type": "input", "target": [{"tag": "input"}], "value": "bezwaar"})
    assert localize_typing(events, "en", {"Dit klopt niet": "This is wrong"}) == []
    assert [e.get("values", {}).get("en") for e in events] == ["T", "This ", "This is wrong", None]


def test_a_search_term_or_a_number_stays_as_typed():
    assert not is_sentence("bezwaar")
    assert not is_sentence("4 112,00")
    assert is_sentence("Ik heb mijn onderneming niet meer")


def test_captions_get_a_file_of_their_own_on_the_same_timing(tmp_path):
    (tmp_path / "main-0123456789ab.nl.vtt").write_text(to_vtt([{"start": 1.0, "end": 2.5, "text": "Dat is nog\nsteeds zo."}, {"start": 3.0, "end": 4.0, "text": "Onvertaald."}]))
    track = {"captions": {"nl": "main-0123456789ab.nl.vtt"}}
    missing = write_captions(track, "en", {"Dat is nog steeds zo.": "That is still the case."}, tmp_path)
    assert missing == ["Onvertaald."]
    assert track["captions"]["en"].startswith("main-") and track["captions"]["en"].endswith(".en.vtt")
    cues = parse_vtt((tmp_path / track["captions"]["en"]).read_text())
    assert [(c["start"], c["end"], c["text"]) for c in cues] == [(1.0, 2.5, "That is still the case."), (3.0, 4.0, "Onvertaald.")]


def test_an_empty_translation_drops_the_caption_and_none_at_all_writes_no_file(tmp_path):
    (tmp_path / "main-0123456789ab.nl.vtt").write_text(to_vtt([{"start": 1.0, "end": 2.0, "text": "In bezwaar"}, {"start": 2.0, "end": 2.5, "text": "gaan."}]))
    track = {"captions": {"nl": "main-0123456789ab.nl.vtt"}}
    assert write_captions(track, "en", {"In bezwaar": "Lodge an objection.", "gaan.": ""}, tmp_path) == []
    assert [c["text"] for c in parse_vtt((tmp_path / track["captions"]["en"]).read_text())] == ["Lodge an objection."]
    untranslated = {"captions": {"nl": "main-0123456789ab.nl.vtt"}}
    assert write_captions(untranslated, "en", {}, tmp_path) == ["In bezwaar", "gaan."]
    assert "en" not in untranslated["captions"]
