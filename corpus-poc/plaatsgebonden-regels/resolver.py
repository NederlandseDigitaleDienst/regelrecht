"""Prototype: answer a question about an activity at a shape on the map.

Three things are proposed here that the schema does not have: the value type
`shape`, the operation OVERLAPS, and `combine` on an open term. This file
stands in for an engine that knows all three, by sitting in front of the
engine that knows none:

- it works out the value of every shape an article uses: the location given
  with the question, or an area that another article designates. Both are
  written as WKT in RD coordinates;
- it decides every OVERLAPS and writes the outcome into the action tree;
- it works out every open term that combines its fillers, and writes that
  outcome into the action tree as well;
- it takes what is left of the three out of the rulework, and hands the
  engine a rulework it can read.

It holds no rule of law of its own. Which outcomes answer a question is in
`questions.yaml`; how the fillers of a term combine is on the term; what an
article says is in the article.
"""

from __future__ import annotations

import copy
import json
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

import yaml
from shapely import wkt
from shapely.geometry import shape
from shapely.geometry.base import BaseGeometry

ROOT = Path(__file__).parent
REPO = ROOT.parent.parent
REGULATION = ROOT / "regulation"
TERRITORIES = ROOT / "territories"
QUESTIONS = ROOT / "questions.yaml"

SHAPE = "shape"

# The codes that tie a rulework to a territory. The geometry of a territory
# comes from a register of administrative boundaries, not from the rulework.
TERRITORY_KEYS = ("gemeente_code", "provincie_code", "waterschap_code")


@dataclass(frozen=True)
class Filler:
    rulework: str
    article: str


@dataclass(frozen=True)
class Answer:
    value: object
    filled_by: list[Filler]


@dataclass(frozen=True)
class Reason:
    """One outcome of the law that holds, and the articles it holds through."""

    article: str
    output: str
    through: list[Filler]


@dataclass(frozen=True)
class Verdict:
    answer: str
    because: list[Reason]


@dataclass(frozen=True)
class Rulework:
    law: dict
    directory: Path

    @property
    def id(self) -> str:
        return self.law["$id"]

    @property
    def articles(self) -> list[dict]:
        return self.law.get("articles", [])

    def article(self, number: object) -> dict:
        return next(a for a in self.articles if str(a["number"]) == str(number))

    def territory(self) -> BaseGeometry | None:
        """The territory of the rulework, or None for a national one."""
        for key in TERRITORY_KEYS:
            if key in self.law:
                return read_geometry(TERRITORIES / f"{self.law[key]}.geojson")
        return None

    def territory_codes(self) -> dict[str, str]:
        return {k: self.law[k] for k in TERRITORY_KEYS if k in self.law}


def read_geometry(path: Path) -> BaseGeometry:
    return shape(json.loads(path.read_text())["geometry"])


def overlaps(a: BaseGeometry, b: BaseGeometry) -> bool:
    """Whether two shapes share more than a boundary.

    Their interiors must meet. A shed half on a strip overlaps it, a shed that
    stands against it does not, and a point overlaps an area when it lies
    inside it and not on its edge.
    """
    return a.relate_pattern(b, "T********")


def is_unknown(value: object) -> bool:
    """Whether the engine could not decide for lack of a fact."""
    return isinstance(value, dict) and value.get("__unknown") is True


def section_of(article: dict) -> dict:
    return article.get("machine_readable", {})


def execution_of(article: dict) -> dict:
    return section_of(article).get("execution", {})


def output_names(article: dict) -> set[str]:
    return {o["name"] for o in execution_of(article).get("output", []) if o.get("type") != SHAPE}


def designating_action(rulework: Rulework, output: str) -> object:
    """The value an article of the rulework gives to a shape output."""
    for article in rulework.articles:
        execution = execution_of(article)
        declared = any(o["name"] == output and o.get("type") == SHAPE for o in execution.get("output", []))
        if not declared:
            continue
        for action in execution.get("actions", []):
            if action.get("output") == output:
                return action["value"]
    raise ValueError(f"{rulework.id}: no article designates the shape {output!r}")


@dataclass
class Case:
    """One question at one place on one date: everything an article may read."""

    ruleworks: list[Rulework]
    location: BaseGeometry
    params: dict
    date: str
    # Open terms already worked out, by (rulework, article, term).
    terms: dict[tuple[str, str, str], object] = field(default_factory=dict)
    # Open terms a filler could not decide for lack of a fact, with what lacks.
    undecided: dict[tuple[str, str, str], list] = field(default_factory=dict)

    def rulework(self, rulework_id: str) -> Rulework:
        return next(r for r in self.ruleworks if r.id == rulework_id)

    def covers(self, rulework: Rulework) -> bool:
        """A rulework with a territory speaks only about locations in it."""
        bounds = rulework.territory()
        return bounds is None or overlaps(self.location, bounds)

    def combines(self, implemented: dict) -> bool:
        """Whether an `implements` entry fills a term that combines its fillers."""
        for rulework in self.ruleworks:
            if rulework.id != implemented["law"]:
                continue
            for article in rulework.articles:
                if str(article["number"]) != str(implemented["article"]):
                    continue
                for term in section_of(article).get("open_terms", []):
                    if term["id"] == implemented["open_term"]:
                        return "combine" in term
        return False

    def others(self, rulework: Rulework) -> list[str]:
        """Every other rulework that speaks at the location, as the engine may read it."""
        return [
            self.for_engine(other, other.articles)
            for other in self.ruleworks
            if other.id != rulework.id and self.covers(other)
        ]

    def scope(self) -> dict:
        """The parameters, with the code of every territory the location lies in."""
        scope = dict(self.params)
        for rulework in self.ruleworks:
            if rulework.territory() is not None and self.covers(rulework):
                scope.update(rulework.territory_codes())
        return scope

    def shape_value(self, rulework: Rulework, article: dict, reference: object) -> BaseGeometry:
        name = str(reference).removeprefix("$")
        execution = execution_of(article)
        for parameter in execution.get("parameters", []):
            if parameter["name"] == name and parameter.get("type") == SHAPE:
                return self.location
        for declared in execution.get("input", []):
            if declared["name"] == name and declared.get("type") == SHAPE:
                source = declared["source"]
                owner = self.rulework(source.get("regulation", rulework.id))
                return wkt.loads(str(designating_action(owner, source["output"])))
        raise ValueError(f"{rulework.id} art. {article['number']}: {name!r} is not a shape here")

    def decide(self, rulework: Rulework, article: dict, node: object) -> object:
        """An action tree with every OVERLAPS and every worked-out term decided."""
        if isinstance(node, list):
            return [self.decide(rulework, article, item) for item in node]
        if isinstance(node, str) and node.startswith("$"):
            key = (rulework.id, str(article["number"]), node.removeprefix("$"))
            return self.terms.get(key, node)
        if not isinstance(node, dict):
            return node
        if node.get("operation") == "OVERLAPS":
            subject = self.shape_value(rulework, article, node["subject"])
            area = self.shape_value(rulework, article, node["value"])
            return overlaps(subject, area)
        return {key: self.decide(rulework, article, value) for key, value in node.items()}

    def for_engine(self, rulework: Rulework, articles: list[dict]) -> str:
        """The rulework as the engine may read it: nothing proposed left in it."""
        prepared = []
        for original in articles:
            article = copy.deepcopy(original)
            section = section_of(article)
            for term in section.get("open_terms", []):
                term.pop("combine", None)
            # A filler of a combining term has been asked on its own; left in,
            # the engine would refuse two of them for one term.
            kept = [i for i in section.get("implements", []) if not self.combines(i)]
            if kept:
                section["implements"] = kept
            else:
                section.pop("implements", None)
            execution = section.get("execution")
            if execution:
                shape_outputs = {o["name"] for o in execution.get("output", []) if o.get("type") == SHAPE}
                for part in ("parameters", "input", "output"):
                    kept = [f for f in execution.get(part, []) if f.get("type") != SHAPE]
                    if kept:
                        execution[part] = kept
                    else:
                        execution.pop(part, None)
                actions = [a for a in execution.get("actions", []) if a.get("output") not in shape_outputs]
                execution["actions"] = self.decide(rulework, original, actions)
                if not execution.get("output"):
                    # An article that only designates an area has nothing left to say.
                    del section["execution"]
                    if not section:
                        del article["machine_readable"]
            prepared.append(article)
        law = dict(rulework.law)
        law["articles"] = prepared
        return yaml.safe_dump(law, sort_keys=False, allow_unicode=True)

    def alongside(self, rulework: Rulework, article: dict) -> list[dict]:
        """The article with the others of its rulework that do not clash with it.

        The engine finds an article by the name of its output, so two articles
        that give an output of the same name cannot be offered together.
        """
        names = output_names(article)
        return [a for a in rulework.articles if a is article or not (output_names(a) & names)]

    def fillers(self, holder: Rulework, article: dict, term: str) -> list[tuple[Rulework, dict]]:
        """Every article in scope that fills the term."""
        number = str(article["number"])
        found = []
        for rulework in self.ruleworks:
            if rulework.id == holder.id or not self.covers(rulework):
                continue
            for candidate in rulework.articles:
                for implemented in section_of(candidate).get("implements", []):
                    same = (
                        implemented["law"] == holder.id
                        and str(implemented["article"]) == number
                        and implemented["open_term"] == term
                    )
                    if same:
                        found.append((rulework, candidate))
        return found

    def work_out_terms(self, holder: Rulework) -> dict[tuple[str, str], list[Filler]]:
        """Work out every open term of a rulework that combines its fillers.

        A term with `combine: OR` holds when any filler says so. A filler that
        gives nothing does not speak about the case. Returns who spoke.
        """
        spoke: dict[tuple[str, str], list[Filler]] = {}
        for article in holder.articles:
            for term in section_of(article).get("open_terms", []):
                found = self.fillers(holder, article, term["id"])
                combine = term.get("combine")
                if combine is None:
                    if len(found) > 1:
                        raise RuntimeError(
                            f"{holder.id} art. {article['number']}: term {term['id']!r} has "
                            f"{len(found)} fillers and no `combine`; the engine takes one"
                        )
                    continue
                if combine != "OR":
                    raise ValueError(f"{holder.id}: combine {combine!r} is not known here")
                speakers = []
                number = str(article["number"])
                for rulework, filler in found:
                    filler_yaml = self.for_engine(rulework, self.alongside(rulework, filler))
                    said = run_engine(filler_yaml, self.others(rulework), term["id"], self.scope(), self.date)
                    if said is True:
                        speakers.append(Filler(rulework.id, str(filler["number"])))
                    elif is_unknown(said):
                        lacking = self.undecided.setdefault((holder.id, number, term["id"]), [])
                        lacking.extend(said.get("missing", []))
                spoke[(number, term["id"])] = sorted(speakers, key=lambda f: (f.rulework, f.article))
                if speakers:
                    self.terms[(holder.id, number, term["id"])] = True
                # With no speaker the term keeps its default, which the engine applies.
        return spoke

    def terms_read(self, rulework: Rulework, article: dict, output: str) -> set[tuple[str, str]]:
        """The open terms an output reads, in its own article or through another."""
        execution = execution_of(article)
        names: set[str] = set()

        def collect(node: object) -> None:
            if isinstance(node, list):
                for item in node:
                    collect(item)
            elif isinstance(node, dict):
                for value in node.values():
                    collect(value)
            elif isinstance(node, str) and node.startswith("$"):
                names.add(node.removeprefix("$"))

        for action in execution.get("actions", []):
            if action.get("output") == output:
                collect(action.get("value"))

        own_terms = {t["id"] for t in section_of(article).get("open_terms", [])}
        read = {(str(article["number"]), name) for name in names & own_terms}
        for declared in execution.get("input", []):
            source = declared.get("source", {})
            if declared["name"] not in names or "regulation" in source or "output" not in source:
                continue
            for candidate in rulework.articles:
                if source["output"] in output_names(candidate):
                    read |= self.terms_read(rulework, candidate, source["output"])
        return read

    def evaluate(self, rulework: Rulework, article: dict, output: str) -> Answer:
        """One output of one article, with the articles that filled its terms."""
        spoke = self.work_out_terms(rulework)
        law_yaml = self.for_engine(rulework, self.alongside(rulework, article))
        value = run_engine(law_yaml, self.others(rulework), output, self.scope(), self.date)
        through: set[Filler] = set()
        lacking: list = []
        for key in self.terms_read(rulework, article, output):
            through.update(spoke.get(key, []))
            lacking.extend(self.undecided.get((rulework.id, *key), []))
        if value is not True and lacking:
            # With OR, one filler that says yes decides the term. Without one,
            # a filler that could not decide leaves the term undecided.
            value = {"__unknown": True, "missing": lacking}
        return Answer(value, sorted(through, key=lambda f: (f.rulework, f.article)))


def in_force(directory: Path, date: str) -> Rulework | None:
    """The version of a rulework in force on the date."""
    candidates = []
    for path in sorted(directory.glob("*.yaml")):
        law = yaml.safe_load(path.read_text())
        starts = str(law.get("valid_from", path.stem))
        ends = law.get("valid_to")
        if starts <= date and (ends is None or date <= str(ends)):
            candidates.append((starts, law))
    if not candidates:
        return None
    return Rulework(max(candidates, key=lambda c: c[0])[1], directory)


def ruleworks_in_force(date: str) -> list[Rulework]:
    directories = sorted({p.parent for p in REGULATION.rglob("*.yaml")})
    found = (in_force(d, date) for d in directories)
    return [f for f in found if f is not None]


def run_engine(law_yaml: str, extra_laws: list[str], output: str, params: dict, date: str) -> object:
    request = {
        "law_yaml": law_yaml,
        "extra_laws": extra_laws,
        "output_names": [output],
        "params": params,
        "date": date,
    }
    cargo = os.environ.get("CARGO", "cargo")
    result = subprocess.run(
        [cargo, "run", "--quiet", "--bin", "evaluate"],
        cwd=REPO / "packages" / "engine",
        input=json.dumps(request),
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"engine failed: {result.stderr or result.stdout}")
    response = json.loads(result.stdout)
    if response.get("error"):
        raise RuntimeError(f"engine error: {response['error']}")
    return response["outputs"][output]


# The answers to "may I do this here?". They are answers to a question, not
# values of the law, so they are here and not in a rulework.
ONLY_WITH_A_PERMIT = "only with a permit"
WITHOUT_A_PERMIT = "without a permit"
CANNOT_SAY = "cannot say"


def ask(question: str, activity: str, location_wkt: str, params: dict, date: str) -> Verdict:
    """Answer a question from `questions.yaml` for an activity at a location.

    The question names the bans of one law that answer it. Each ban stands on
    its own, so the activity needs a permit when any of them holds. When none
    holds and the engine could not decide one of them for lack of a fact, the
    answer is that it cannot be said.
    """
    asked = yaml.safe_load(QUESTIONS.read_text())[question]
    case = Case(ruleworks_in_force(date), wkt.loads(location_wkt), {**params, "activiteit": activity}, date)
    law = case.rulework(asked["law"])
    because = []
    undecided = []
    for ban in asked["bans"]:
        article = law.article(ban["article"])
        answer = case.evaluate(law, article, ban["output"])
        if is_unknown(answer.value):
            undecided.append((ban["output"], answer.value.get("missing")))
        if answer.value is not True:
            continue
        because.append(Reason(str(ban["article"]), ban["output"], answer.filled_by))
        for extra in ban.get("also_report", []):
            reported = case.evaluate(law, law.article(extra["article"]), extra["output"])
            if reported.value is True:
                because.append(Reason(str(extra["article"]), extra["output"], reported.filled_by))
    if because:
        return Verdict(ONLY_WITH_A_PERMIT, because)
    if undecided:
        return Verdict(CANNOT_SAY, [Reason("", f"{output}: {missing}", []) for output, missing in undecided])
    return Verdict(WITHOUT_A_PERMIT, [])
