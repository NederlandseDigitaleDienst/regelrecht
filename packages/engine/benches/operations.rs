use criterion::{Criterion, criterion_group, criterion_main};
use regelrecht_engine::operations::{ValueResolver, evaluate_value, execute_operation};
use regelrecht_engine::{ActionOperation, ActionValue, Case, Value};
use std::collections::HashMap;
use std::hint::black_box;

/// Simple resolver backed by a HashMap (no tracing overhead).
struct SimpleResolver {
    vars: HashMap<String, Value>,
}

impl SimpleResolver {
    fn new() -> Self {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), Value::Int(42));
        vars.insert("y".to_string(), Value::Int(17));
        vars.insert("name".to_string(), Value::String("test".to_string()));
        vars.insert("flag".to_string(), Value::Bool(true));
        vars.insert("income".to_string(), Value::Int(35000));
        vars.insert("threshold".to_string(), Value::Int(25000));
        Self { vars }
    }
}

impl ValueResolver for SimpleResolver {
    fn resolve(&self, name: &str) -> regelrecht_engine::Result<Value> {
        self.vars.get(name).cloned().ok_or_else(|| {
            regelrecht_engine::error::EngineError::VariableNotFound(name.to_string())
        })
    }
}

fn literal(v: Value) -> ActionValue {
    ActionValue::Literal(v)
}

fn var_ref(name: &str) -> ActionValue {
    ActionValue::Literal(Value::String(format!("${}", name)))
}

fn bench_operations(c: &mut Criterion) {
    let resolver = SimpleResolver::new();

    let mut group = c.benchmark_group("operations");

    // EQUALS with literals
    let equals_op = ActionOperation::Equals {
        subject: literal(Value::Int(42)),
        value: literal(Value::Int(42)),
    };
    group.bench_function("equals_literals", |b| {
        b.iter(|| execute_operation(black_box(&equals_op), &resolver, 0))
    });

    // EQUALS with variable resolution
    let equals_var_op = ActionOperation::Equals {
        subject: var_ref("x"),
        value: literal(Value::Int(42)),
    };
    group.bench_function("equals_with_var", |b| {
        b.iter(|| execute_operation(black_box(&equals_var_op), &resolver, 0))
    });

    // ADD with values
    let add_op = ActionOperation::Add {
        values: vec![var_ref("x"), var_ref("y"), literal(Value::Int(100))],
    };
    group.bench_function("add_three_values", |b| {
        b.iter(|| execute_operation(black_box(&add_op), &resolver, 0))
    });

    // MULTIPLY
    let mul_op = ActionOperation::Multiply {
        values: vec![var_ref("income"), literal(Value::from(0.1345))],
    };
    group.bench_function("multiply", |b| {
        b.iter(|| execute_operation(black_box(&mul_op), &resolver, 0))
    });

    // IF conditional (cases/default syntax)
    let if_op = ActionOperation::If {
        cases: vec![Case {
            when: ActionValue::Operation(Box::new(ActionOperation::GreaterThan {
                subject: var_ref("income"),
                value: var_ref("threshold"),
            })),
            then: literal(Value::Int(100)),
        }],
        default: Some(literal(Value::Int(0))),
    };
    group.bench_function("if_conditional", |b| {
        b.iter(|| execute_operation(black_box(&if_op), &resolver, 0))
    });

    // AND with 3 conditions
    let and_op = ActionOperation::And {
        conditions: vec![
            var_ref("flag"),
            ActionValue::Operation(Box::new(ActionOperation::GreaterThan {
                subject: var_ref("x"),
                value: literal(Value::Int(0)),
            })),
            ActionValue::Operation(Box::new(ActionOperation::Equals {
                subject: var_ref("name"),
                value: literal(Value::String("test".to_string())),
            })),
        ],
    };
    group.bench_function("and_three_conditions", |b| {
        b.iter(|| execute_operation(black_box(&and_op), &resolver, 0))
    });

    // evaluate_value with literal
    group.bench_function("evaluate_literal", |b| {
        let val = literal(Value::Int(42));
        b.iter(|| evaluate_value(black_box(&val), &resolver, 0))
    });

    // evaluate_value with variable
    group.bench_function("evaluate_variable", |b| {
        let val = var_ref("income");
        b.iter(|| evaluate_value(black_box(&val), &resolver, 0))
    });

    group.finish();
}

criterion_group!(benches, bench_operations);
criterion_main!(benches);
