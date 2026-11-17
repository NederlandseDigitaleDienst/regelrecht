"""Step bindings for the location prototype. Not the canonical grammar:
these three steps are what `bdd/grammar.yaml` would have to gain."""

import sys
from pathlib import Path

from behave import given, then, when

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from resolver import permit_duties  # noqa: E402


@given('the calculation date is "{date}"')
def set_date(context, date):
    context.date = date


@given("the location is the shape:")
def set_location(context):
    context.location = context.text.strip()


@when('I ask what "{activity}" needs at the location')
def ask(context, activity):
    context.duties = permit_duties(context.location, activity, context.date)


@then("the permit duties are:")
def assert_duties(context):
    expected = sorted((row["rulework"], row["article"]) for row in context.table)
    actual = [(d.rulework, d.article) for d in context.duties]
    assert actual == expected, f"expected {expected}, got {actual}"


@then("there are no permit duties")
def assert_none(context):
    assert context.duties == [], f"expected none, got {context.duties}"
