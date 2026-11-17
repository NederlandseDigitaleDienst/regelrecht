"""Step bindings for the location prototype.

Three of these steps are the canonical ones from `bdd/grammar.yaml`, bound
here to the resolver. Two are new: the location as a shape, and which
decentral articles filled an output.
"""

import sys
from pathlib import Path

from behave import given, then, when

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from resolver import evaluate_at  # noqa: E402


@given('the calculation date is "{date}"')
def set_date(context, date):
    context.date = date
    context.params = {}
    context.answers = {}


@given("the location is the shape:")
def set_location(context):
    context.location = context.text.strip()


@given('parameter "{name}" is "{value}"')
def set_parameter(context, name, value):
    context.params[name] = value


@when('I evaluate "{output}" of "{law}"')
def evaluate(context, output, law):
    context.answers[output] = evaluate_at(law, output, context.location, context.params, context.date)


@then('output "{output}" is true')
def assert_true(context, output):
    assert context.answers[output].value is True, f"{output} is {context.answers[output].value!r}"


@then('output "{output}" is false')
def assert_false(context, output):
    assert context.answers[output].value is False, f"{output} is {context.answers[output].value!r}"


@then('output "{output}" is filled by:')
def assert_filled_by(context, output):
    expected = sorted((row["rulework"], row["article"]) for row in context.table)
    actual = [(f.rulework, f.article) for f in context.answers[output].filled_by]
    assert actual == expected, f"expected {expected}, got {actual}"
