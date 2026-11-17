"""Prototype: evaluate an output of a law at a shape on the map.

Two things are proposed here that the schema does not have: the value type
`shape` and the operation OVERLAPS. This file stands in for an engine that
knows both, by sitting in front of the engine that does not:

- it works out the value of every shape an article uses: the location given
  with the question, or an area that another article designates. Both are
  written as WKT in RD coordinates;
- it decides every OVERLAPS and writes the outcome into the action tree;
- it takes what is left of `shape` out of the rulework, and hands the engine
  a rulework it can read.

The question goes to one law. Which decentral ruleworks speak follows from the
shape, because a rulework with a territory speaks only about locations in it,
and from delegation (`implements`), which the engine resolves itself.
"""

from __future__ import annotations

import copy
import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

import yaml
from shapely import wkt
from shapely.geometry import shape
from shapely.geometry.base import BaseGeometry

ROOT = Path(__file__).parent
REPO = ROOT.parent.parent
REGULATION = ROOT / "regulation"
TERRITORIES = ROOT / "territories"

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
class Rulework:
    law: dict
    directory: Path

    @property
    def id(self) -> str:
        return self.law["$id"]

    @property
    def articles(self) -> list[dict]:
        return self.law.get("articles", [])

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


def execution_of(article: dict) -> dict:
    return article.get("machine_readable", {}).get("execution", {})


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


class Shapes:
    """The value of every shape an article can name."""

    def __init__(self, ruleworks: list[Rulework], location: BaseGeometry):
        self.ruleworks = {r.id: r for r in ruleworks}
        self.location = location

    def value(self, rulework: Rulework, article: dict, reference: object) -> BaseGeometry:
        name = str(reference).removeprefix("$")
        execution = execution_of(article)
        for parameter in execution.get("parameters", []):
            if parameter["name"] == name and parameter.get("type") == SHAPE:
                return self.location
        for declared in execution.get("input", []):
            if declared["name"] == name and declared.get("type") == SHAPE:
                source = declared["source"]
                owner = self.ruleworks[source.get("regulation", rulework.id)]
                return wkt.loads(str(designating_action(owner, source["output"])))
        raise ValueError(f"{rulework.id} art. {article['number']}: {name!r} is not a shape here")

    def decide(self, rulework: Rulework, article: dict, node: object) -> object:
        """The action tree with every OVERLAPS replaced by its outcome."""
        if isinstance(node, list):
            return [self.decide(rulework, article, item) for item in node]
        if not isinstance(node, dict):
            return node
        if node.get("operation") == "OVERLAPS":
            subject = self.value(rulework, article, node["subject"])
            area = self.value(rulework, article, node["value"])
            return overlaps(subject, area)
        return {key: self.decide(rulework, article, value) for key, value in node.items()}


def without_shapes(rulework: Rulework, articles: list[dict], shapes: Shapes) -> str:
    """The rulework as the engine may read it: no shape left anywhere."""
    prepared = []
    for original in articles:
        article = copy.deepcopy(original)
        execution = execution_of(article)
        if execution:
            shape_outputs = {o["name"] for o in execution.get("output", []) if o.get("type") == SHAPE}
            for section in ("parameters", "input", "output"):
                kept = [f for f in execution.get(section, []) if f.get("type") != SHAPE]
                if kept:
                    execution[section] = kept
                else:
                    execution.pop(section, None)
            actions = [a for a in execution.get("actions", []) if a.get("output") not in shape_outputs]
            execution["actions"] = shapes.decide(rulework, original, actions)
            if not execution.get("output"):
                # An article that only designates an area has nothing left to say.
                del article["machine_readable"]["execution"]
                if not article["machine_readable"]:
                    del article["machine_readable"]
        prepared.append(article)
    law = dict(rulework.law)
    law["articles"] = prepared
    return yaml.safe_dump(law, sort_keys=False, allow_unicode=True)


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


@dataclass(frozen=True)
class Statement:
    """What one article says about the activity at the location."""

    rulework: str
    article: str
    output: str
    value: object


def judges_at_a_place(article: dict) -> bool:
    """Whether an article judges something at a place.

    Both parts are declarations the format already has or would have: the
    article produces a TOETS, and it takes a shape as a parameter.
    """
    execution = execution_of(article)
    produces = execution.get("produces", {})
    takes_shape = any(p.get("type") == SHAPE for p in execution.get("parameters", []))
    return produces.get("legal_character") == "TOETS" and takes_shape


def output_names(article: dict) -> set[str]:
    return {o["name"] for o in execution_of(article).get("output", []) if o.get("type") != SHAPE}


def referenced(article: dict) -> set[str]:
    """The other ruleworks an article needs to be read."""
    section = article.get("machine_readable", {})
    laws = {i["law"] for i in section.get("implements", [])}
    for declared in section.get("execution", {}).get("input", []):
        regulation = declared.get("source", {}).get("regulation")
        if regulation is not None:
            laws.add(regulation)
    return laws


def ask(activity: str, location_wkt: str, params: dict, date: str) -> list[Statement]:
    """Every article that speaks about the activity at the location.

    The question names no rulework. A rulework with a territory is asked only
    when the location lies in it, and of what remains every article that
    judges at a place is asked. An article that gives nothing is silent.
    """
    location = wkt.loads(location_wkt)
    available = ruleworks_in_force(date)
    by_id = {r.id: r for r in available}
    shapes = Shapes(available, location)
    here = [r for r in available if r.territory() is None or overlaps(location, r.territory())]
    scope = {**params, "activiteit": activity}
    for rulework in here:
        scope.update(rulework.territory_codes())

    statements = []
    for rulework in here:
        for article in rulework.articles:
            if not judges_at_a_place(article):
                continue
            names = output_names(article)
            # Another article of the same rulework may give an output of the
            # same name; the engine finds an article by its output, so those
            # are left out.
            alongside = [a for a in rulework.articles if a is article or not (output_names(a) & names)]
            law_yaml = without_shapes(rulework, alongside, shapes)
            others = [without_shapes(by_id[i], by_id[i].articles, shapes) for i in sorted(referenced(article))]
            for name in sorted(names):
                value = run_engine(law_yaml, others, name, scope, date)
                if value is not None:
                    statements.append(Statement(rulework.id, str(article["number"]), name, value))
    return sorted(statements, key=lambda s: (s.rulework, s.article, s.output))


def fills(article: dict, law_id: str) -> bool:
    implements = article.get("machine_readable", {}).get("implements", [])
    return any(i["law"] == law_id for i in implements)


def evaluate_at(law_id: str, output: str, location_wkt: str, params: dict, date: str) -> Answer:
    location = wkt.loads(location_wkt)
    available = ruleworks_in_force(date)
    shapes = Shapes(available, location)
    asked = next(r for r in available if r.id == law_id)
    asked_yaml = without_shapes(asked, asked.articles, shapes)

    # The ruleworks that fill a term of the asked law. One with a territory
    # speaks only about locations in it.
    fillers = [r for r in available if r.id != law_id and any(fills(a, law_id) for a in r.articles)]
    here = [r for r in fillers if r.territory() is None or overlaps(location, r.territory())]
    scope = dict(params)
    for rulework in here:
        scope.update(rulework.territory_codes())

    # The engine takes one filler per open term and refuses two articles of
    # one rulework that fill the same term ("Ambiguous priority"). A plan fills
    # a term article by article, each for its own cases, so the rule imitated
    # here is: an article that does not speak about the case gives nothing,
    # and the articles that do speak must agree.
    without_fillers = run_engine(asked_yaml, [], output, scope, date)
    spoken: dict[Filler, object] = {}
    for rulework in here:
        for article in rulework.articles:
            if not fills(article, law_id):
                continue
            alone = without_shapes(rulework, [article], shapes)
            answer = run_engine(asked_yaml, [alone], output, scope, date)
            if answer != without_fillers:
                spoken[Filler(rulework.id, str(article["number"]))] = answer

    answers = {json.dumps(a, sort_keys=True) for a in spoken.values()}
    if len(answers) > 1:
        raise RuntimeError(f"{law_id}: fillers disagree about {output!r}: {spoken}")
    value = next(iter(spoken.values())) if spoken else without_fillers
    return Answer(value, sorted(spoken, key=lambda f: (f.rulework, f.article)))
