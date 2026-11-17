"""Prototype: find what applies to an activity at a shape on the map.

This sits in front of the unchanged engine. It reads two keys the schema does
not have yet, `areas` on a rulework and `applies_in` on an article, decides
from geometry which articles are in scope, and asks the engine for the
`vergunningplicht` output of each of those articles.

The engine never sees a geometry. An article that is out of scope is not
handed to the engine at all, the same way a version outside its validity
dates is not.
"""

from __future__ import annotations

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

# The output every article that imposes a permit duty gives. Discovery is
# "every article in scope that has this output".
DUTY_OUTPUT = "vergunningplicht"

# Keys of the proposal. The engine rejects unknown keys, so they are taken out
# before a rulework is handed over.
PROPOSED_LAW_KEYS = ("areas",)
PROPOSED_ARTICLE_KEYS = ("applies_in",)

# The codes that tie a rulework to a territory. The geometry of a territory
# comes from a register of administrative boundaries, not from the rulework.
TERRITORY_KEYS = ("gemeente_code", "provincie_code", "waterschap_code")


@dataclass(frozen=True)
class Duty:
    rulework: str
    article: str


def read_geometry(path: Path) -> BaseGeometry:
    return shape(json.loads(path.read_text())["geometry"])


def overlaps(location: BaseGeometry, area: BaseGeometry) -> bool:
    """Whether the location lies at least partly in the area.

    Sharing only a boundary is not lying in it, so the overlap must have a
    surface of its own.
    """
    return location.intersection(area).area > 0


def in_force(directory: Path, date: str) -> tuple[dict, Path] | None:
    """The version of a rulework in force on the date, with its directory."""
    candidates = []
    for path in sorted(directory.glob("*.yaml")):
        law = yaml.safe_load(path.read_text())
        starts = str(law.get("valid_from", path.stem))
        ends = law.get("valid_to")
        if starts <= date and (ends is None or date <= str(ends)):
            candidates.append((starts, law))
    if not candidates:
        return None
    return max(candidates, key=lambda c: c[0])[1], directory


def ruleworks(date: str) -> list[tuple[dict, Path]]:
    directories = sorted({p.parent for p in REGULATION.rglob("*.yaml")})
    found = (in_force(d, date) for d in directories)
    return [f for f in found if f is not None]


def territory(law: dict) -> BaseGeometry | None:
    """The territory of a rulework, or None for a national one."""
    for key in TERRITORY_KEYS:
        if key in law:
            return read_geometry(TERRITORIES / f"{law[key]}.geojson")
    return None


def article_area(law: dict, directory: Path, article: dict) -> BaseGeometry | None:
    """The area an article applies in, or None when it follows the rulework."""
    name = article.get("applies_in")
    if name is None:
        return None
    declared = law.get("areas", {})
    if name not in declared:
        raise ValueError(f"{law['$id']} art. {article['number']}: area {name!r} is not declared")
    return read_geometry(directory / declared[name]["geometry"])


def in_scope(law: dict, directory: Path, article: dict, location: BaseGeometry) -> bool:
    bounds = territory(law)
    if bounds is not None and not overlaps(location, bounds):
        return False
    area = article_area(law, directory, article)
    return area is None or overlaps(location, area)


def has_duty_output(article: dict) -> bool:
    execution = article.get("machine_readable", {}).get("execution", {})
    return any(o["name"] == DUTY_OUTPUT for o in execution.get("output", []))


def for_engine(law: dict, article: dict) -> str:
    """The rulework as the engine may read it: this one article, no proposed keys."""
    stripped_article = {k: v for k, v in article.items() if k not in PROPOSED_ARTICLE_KEYS}
    stripped = {k: v for k, v in law.items() if k not in PROPOSED_LAW_KEYS}
    stripped["articles"] = [stripped_article]
    return yaml.safe_dump(stripped, sort_keys=False, allow_unicode=True)


def evaluate(law_yaml: str, activity: str, date: str) -> bool:
    request = {
        "law_yaml": law_yaml,
        "output_names": [DUTY_OUTPUT],
        "params": {"activiteit": activity},
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
    return response["outputs"][DUTY_OUTPUT] is True


def permit_duties(location_wkt: str, activity: str, date: str) -> list[Duty]:
    location = wkt.loads(location_wkt)
    duties = []
    for law, directory in ruleworks(date):
        for article in law.get("articles", []):
            if not has_duty_output(article):
                continue
            if not in_scope(law, directory, article, location):
                continue
            if evaluate(for_engine(law, article), activity, date):
                duties.append(Duty(law["$id"], str(article["number"])))
    return sorted(duties, key=lambda d: (d.rulework, d.article))
