"""Step bindings for the location prototype.

The steps that set a date and parameters are the canonical ones from
`bdd/grammar.yaml`. New are: the location as a shape, the question "may I do
this here?", its answer, and the outcomes of the law behind the answer.
"""

import sys
from pathlib import Path

from behave import given, then, when

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from resolver import ask  # noqa: E402

QUESTION = "may_i_do_this_here"


def inferred(text: str) -> object:
    """A table cell or quoted value read as the grammar reads it."""
    if text in ("true", "false"):
        return text == "true"
    try:
        return float(text) if "." in text else int(text)
    except ValueError:
        return text


@given('the calculation date is "{date}"')
def set_date(context, date):
    context.date = date
    context.params = {}


@given("the location is the shape:")
def set_location(context):
    context.location = context.text.strip()


@given('parameter "{name}" is "{value}"')
def set_parameter(context, name, value):
    context.params[name] = inferred(value)


@given("the following parameters:")
def set_parameters(context):
    # A parameter table has no header row; behave reads the first row as one.
    rows = [context.table.headings] + [row.cells for row in context.table]
    for name, value in rows:
        context.params[name] = inferred(value)


@when('I ask whether I may "{activity}" at the location')
def ask_whether(context, activity):
    context.verdict = ask(QUESTION, activity, context.location, context.params, context.date)


@then('the answer is "{answer}"')
def assert_answer(context, answer):
    assert context.verdict.answer == answer, f"the answer is {context.verdict.answer!r}: {context.verdict.because}"


@then("because of:")
def assert_because(context):
    expected = sorted((row["article"], row["output"], row["through"]) for row in context.table)
    actual = sorted(
        (r.article, r.output, ", ".join(f"{f.rulework} {f.article}" for f in r.through))
        for r in context.verdict.because
    )
    assert actual == expected, f"expected {expected}, got {actual}"
