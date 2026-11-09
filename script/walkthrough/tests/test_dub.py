import array

import pytest
from walkthrough.dub import (
    RATE,
    SPEED_MAX,
    TEMPO_LIMIT,
    TEMPO_MAX,
    aligned_words,
    assemble,
    basis,
    budget,
    fit,
    sentences,
)


def w(text, s, e):
    return {"word": text, "start": s, "end": e}


WORDS = [w("Dit", 1.0, 1.2), w("is", 1.3, 1.4), w("de", 1.5, 1.6), w("wet.", 1.7, 2.0), w("Kijk", 4.0, 4.3), w("zelf", 4.4, 4.6), w("rond.", 4.7, 5.0), w("Verder.", 8.0, 8.5)]


def test_a_slot_runs_until_the_next_sentence_starts():
    out = sentences(WORDS, [], 10.0)
    assert [s["nl"] for s in out] == ["Dit is de wet.", "Kijk zelf rond.", "Verder."]
    assert [(s["start"], s["end"], s["until"]) for s in out] == [(1.0, 2.0, 4.0), (4.0, 5.0, 8.0), (8.0, 8.5, 10.0)]


def test_a_pause_ends_the_slot_of_the_sentence_before_it():
    out = sentences(WORDS, [5.0], 10.0)
    # The player stops at 5.0; the English must be done by then.
    assert out[1]["until"] == 5.0
    assert out[0]["until"] == 4.0


def test_a_sentence_that_crosses_a_break_is_split_there():
    words = [w("Dit", 1.0, 1.2), w("loopt", 1.3, 1.6), w("door.", 2.5, 3.0)]
    out = sentences(words, [2.0], 4.0)
    assert [s["nl"] for s in out] == ["Dit loopt", "door."]
    assert out[0]["until"] == 2.0


def test_a_pause_inside_the_last_word_still_ends_the_slot():
    # A pause set by hand can land a moment before the last word ends.
    out = sentences(WORDS, [4.9], 10.0)
    assert out[1]["nl"] == "Kijk zelf rond."
    assert out[1]["until"] == 5.0  # never earlier than the sentence itself
    assert out[2]["start"] == 8.0


def test_a_build_keeps_a_dub_only_while_the_dutch_is_the_same(tmp_path):
    from walkthrough.cli import kept_dubs

    said = {"main": {"duration": 10.0, "sentences": sentences(WORDS, [], 10.0)}}
    d = {"audio": {"src": "main-voice-en-a.m4a"}, "captions": "main-a.en.dub.vtt", "basis": basis(said["main"])}
    old = {"main": {"dub": {"en": d}}, "dub": {"en": {"bumpers": {"wait": [{"src": "bumper-wait-en-b.m4a"}]}}}}
    dubs, keep = kept_dubs(old, said)
    assert dubs == {"main": {"en": d}}
    assert keep == {"main-voice-en-a.m4a", "main-a.en.dub.vtt", "bumper-wait-en-b.m4a"}
    moved = {"main": {"duration": 10.0, "sentences": sentences(WORDS, [5.0], 10.0)}}
    dubs, keep = kept_dubs(old, moved)
    assert dubs == {}
    assert keep == {"bumper-wait-en-b.m4a"}


def test_the_budget_follows_the_slot():
    assert budget({"start": 1.0, "until": 5.0}) == 10
    assert budget({"start": 1.0, "until": 1.1}) == 2


def test_the_basis_changes_with_the_dutch_but_not_with_anything_else():
    track = {"duration": 10.0, "sentences": sentences(WORDS, [], 10.0)}
    same = {"duration": 10.0, "sentences": sentences(WORDS, [], 10.0), "dub": {"en": {}}}
    assert basis(track) == basis(same)
    moved = {"duration": 10.0, "sentences": sentences(WORDS, [5.0], 10.0)}
    assert basis(track) != basis(moved)


def test_the_fit_ladder():
    assert fit(2.0, 3.0).action == "place"
    # Too long: first faster speech, a little more than the shortfall.
    step = fit(3.3, 3.0)
    assert step.action == "speed" and 1.1 < step.speed <= SPEED_MAX
    # At full speed and only slightly long: stretch it.
    step = fit(3.2, 3.0, speed=SPEED_MAX)
    assert step.action == "place" and step.tempo == pytest.approx(3.2 / 3.0, abs=1e-3)
    # Much too long: shorten, and after that squeeze, never beyond the limit.
    assert fit(4.0, 3.0, speed=SPEED_MAX).action == "shorten"
    step = fit(4.0, 3.0, speed=SPEED_MAX, shortened=True)
    assert step.action == "squeeze" and step.tempo == TEMPO_LIMIT
    assert TEMPO_MAX < TEMPO_LIMIT
    # A voice without a speed setting goes straight to stretching.
    assert fit(3.2, 3.0, can_speed=False).action == "place"
    # A slower voice may be stretched further before it is shortened.
    assert fit(3.6, 3.0, can_speed=False).action == "shorten"
    step = fit(3.6, 3.0, can_speed=False, tempo_max=1.2)
    assert step.action == "place" and step.tempo == pytest.approx(1.2)


def test_a_shortening_that_is_the_dutch_again_is_refused():
    from walkthrough.dub import echoes

    assert echoes("Dat is onze interpretatie.", "Dat is onze interpretatie van die wet.")
    assert not echoes("That is our interpretation.", "Dat is onze interpretatie van die wet.")
    # Numbers and names are the same in both languages; that alone is no echo.
    assert not echoes("Average age 54, 36% partnered, 40% tenants.", "Gemiddelde leeftijd 54, 36% met partner, 40% huurders.")


def test_words_land_on_the_track_clock_and_follow_a_stretch():
    text = "Hi there."
    starts = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]
    ends = [x + 0.1 for x in starts]
    out = aligned_words(text, {"character_start_times_seconds": starts, "character_end_times_seconds": ends}, at=10.0, tempo=2.0)
    assert out == [{"word": "Hi", "start": 10.0, "end": 10.1}, {"word": "there.", "start": 10.15, "end": 10.45}]


def test_assemble_is_exactly_the_track_long_and_mixes_an_overlap():
    one = array.array("h", [1000] * RATE)  # one second
    samples = assemble([(one, 0.5), (one, 1.0), (one, 9.5)], total=10.0)
    assert len(samples) == 10 * RATE
    # Silence before, the mix where two clips meet, cut off at the end.
    assert samples[0] == 0
    assert samples[int(0.75 * RATE)] == 1000
    assert samples[int(1.25 * RATE)] == 2000
    assert samples[-1] == 1000
