"""Step bindings for the location prototype.

The steps that set a date and a parameter, evaluate an output and assert its
value are the canonical ones from `bdd/grammar.yaml`, bound here to the
resolver. New are: the location as a shape, the question "may I do this
here?", what the articles answer, and which decentral articles filled an
output.
"""

import sys
from pathlib import Path

from behave import given, then, when

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from resolver import ask, evaluate_at  # noqa: E402


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


@given('parameter "{name}" is {value:g}')
def set_number_parameter(context, name, value):
    context.params[name] = value


@when('I ask whether I may "{activity}" at the location')
def ask_whether(context, activity):
    context.statements = ask(activity, context.location, context.params, context.date)


@then("these articles speak:")
def assert_statements(context):
    expected = sorted((row["rulework"], row["article"], row["output"], row["value"]) for row in context.table)
    actual = [(s.rulework, s.article, s.output, str(s.value).lower()) for s in context.statements]
    assert actual == expected, f"expected {expected}, got {actual}"


@then("no article speaks")
def assert_silence(context):
    assert context.statements == [], f"expected none, got {context.statements}"


@when('I evaluate "{output}" of "{law}"')
def evaluate(context, output, law):
    context.answers[output] = evaluate_at(law, output, context.location, context.params, context.date)


@then('output "{output}" is true')
def assert_true(context, output):
    assert context.answers[output].value is True, f"{output} is {context.answers[output].value!r}"


@then('output "{output}" is false')
def assert_false(context, output):
    assert context.answers[output].value is False, f"{output} is {context.answers[output].value!r}"


@then('output "{output}" is absent')
def assert_absent(context, output):
    assert context.answers[output].value is None, f"{output} is {context.answers[output].value!r}"


@then('output "{output}" is filled by:')
def assert_filled_by(context, output):
    expected = sorted((row["rulework"], row["article"]) for row in context.table)
    actual = [(f.rulework, f.article) for f in context.answers[output].filled_by]
    assert actual == expected, f"expected {expected}, got {actual}"
