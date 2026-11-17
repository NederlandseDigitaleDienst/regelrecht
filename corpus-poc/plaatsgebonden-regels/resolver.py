"""Prototype: evaluate an output of a law at a shape on the map.

This sits in front of the unchanged engine and reads what the schema does not
have yet:

- `areas` on a rulework: named variables of type shape, each with the geometry
  adopted with the rulework;
- `applies_in` on an article: the area outside which the article does not
  exist;
- a parameter of type `shape` and the operation OVERLAPS, for an area that
  sits inside the logic of an article.

The question goes to one law. Which decentral ruleworks speak follows from the
shape (their territory) and from delegation (`implements`), which the engine
resolves itself.

What the engine cannot do and this file imitates: several articles filling the
same open term, each for its own cases. The engine takes one filler per term,
so every filler in scope is offered on its own and the answers are combined
with OR. That only makes sense for a boolean term.
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

    def area(self, name: str) -> BaseGeometry:
        declared = self.law.get("areas", {})
        if name not in declared:
            raise ValueError(f"{self.id}: area {name!r} is not declared")
        return read_geometry(self.directory / declared[name]["geometry"])

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


def overlaps(location: BaseGeometry, area: BaseGeometry) -> bool:
    """Whether the location lies at least partly in the area.

    Sharing only a boundary is not lying in it, so the overlap must have a
    surface of its own. A point or a line has no surface and never matches;
    this prototype only takes polygons.
    """
    return location.intersection(area).area > 0


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


def ruleworks(date: str) -> list[Rulework]:
    directories = sorted({p.parent for p in REGULATION.rglob("*.yaml")})
    found = (in_force(d, date) for d in directories)
    return [f for f in found if f is not None]


def article_in_scope(rulework: Rulework, article: dict, location: BaseGeometry) -> bool:
    name = article.get("applies_in")
    return name is None or overlaps(location, rulework.area(name))


def resolve_overlaps(node: object, rulework: Rulework, location: BaseGeometry) -> object:
    """Replace every OVERLAPS in an action tree by its outcome."""
    if isinstance(node, list):
        return [resolve_overlaps(item, rulework, location) for item in node]
    if not isinstance(node, dict):
        return node
    if node.get("operation") == "OVERLAPS":
        return overlaps(location, rulework.area(str(node["value"]).removeprefix("$")))
    return {key: resolve_overlaps(value, rulework, location) for key, value in node.items()}


def for_engine(rulework: Rulework, articles: list[dict], location: BaseGeometry) -> str:
    """The rulework as the engine may read it.

    Only the given articles, without the proposed keys, without parameters of
    type shape, and with every OVERLAPS already decided.
    """
    law = {k: v for k, v in rulework.law.items() if k != "areas"}
    prepared = []
    for article in articles:
        article = copy.deepcopy({k: v for k, v in article.items() if k != "applies_in"})
        execution = article.get("machine_readable", {}).get("execution")
        if execution is not None:
            execution["parameters"] = [p for p in execution.get("parameters", []) if p.get("type") != "shape"]
            execution["actions"] = resolve_overlaps(execution.get("actions", []), rulework, location)
        prepared.append(article)
    law["articles"] = prepared
    return yaml.safe_dump(law, sort_keys=False, allow_unicode=True)


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


def fills(article: dict, law_id: str) -> bool:
    implements = article.get("machine_readable", {}).get("implements", [])
    return any(i["law"] == law_id for i in implements)


def evaluate_at(law_id: str, output: str, location_wkt: str, params: dict, date: str) -> Answer:
    location = wkt.loads(location_wkt)
    available = ruleworks(date)
    asked = next(r for r in available if r.id == law_id)
    asked_yaml = for_engine(asked, asked.law.get("articles", []), location)

    # Every article of another rulework that fills a term of the asked law and
    # is in scope at the location: its rulework covers the location, and the
    # article itself applies there.
    candidates: list[tuple[Rulework, dict]] = []
    for rulework in available:
        if rulework.id == law_id:
            continue
        bounds = rulework.territory()
        if bounds is not None and not overlaps(location, bounds):
            continue
        for article in rulework.law.get("articles", []):
            if fills(article, law_id) and article_in_scope(rulework, article, location):
                candidates.append((rulework, article))

    if not candidates:
        return Answer(run_engine(asked_yaml, [], output, params, date), [])

    filled_by = []
    for rulework, article in candidates:
        filler_yaml = for_engine(rulework, [article], location)
        scoped = {**params, **rulework.territory_codes()}
        if run_engine(asked_yaml, [filler_yaml], output, scoped, date) is True:
            filled_by.append(Filler(rulework.id, str(article["number"])))
    filled_by.sort(key=lambda f: (f.rulework, f.article))
    return Answer(bool(filled_by), filled_by)
