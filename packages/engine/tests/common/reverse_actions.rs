//! Loading a law with its actions reversed, to show that the order of the
//! actions in the file never changes a value (used by the BDD runner).

/// `BDD_REVERSE_ACTIONS=1` loads every law with the actions of each article
/// (and of each open-term default) in reverse order. The order of the actions
/// in the file must never change a value, so every scenario passes either way;
/// CI runs the buckets both ways. The assignments of an output that is set
/// more than once do keep their order among themselves, so they stay together
/// and in place: what is reversed is the order of the outputs.
pub fn enabled() -> bool {
    std::env::var("BDD_REVERSE_ACTIONS").is_ok_and(|v| v.trim() == "1")
}

/// `content` with every `actions` list under `machine_readable` reversed.
pub fn reversed_actions(content: &str) -> Result<String, serde_yaml_ng::Error> {
    fn reverse(node: &mut serde_yaml_ng::Value) {
        match node {
            serde_yaml_ng::Value::Mapping(map) => {
                for (key, value) in map.iter_mut() {
                    if key.as_str() == Some("actions")
                        && let serde_yaml_ng::Value::Sequence(actions) = value
                    {
                        reverse_outputs(actions);
                    }
                    reverse(value);
                }
            }
            serde_yaml_ng::Value::Sequence(items) => items.iter_mut().for_each(reverse),
            _ => {}
        }
    }
    // Group the actions by output in order of first appearance, reverse the
    // groups and keep each group's own order.
    fn reverse_outputs(actions: &mut Vec<serde_yaml_ng::Value>) {
        let mut groups: Vec<(Option<serde_yaml_ng::Value>, Vec<serde_yaml_ng::Value>)> = Vec::new();
        for action in actions.drain(..) {
            let output = action.get("output").cloned();
            match groups
                .iter_mut()
                .find(|(o, _)| output.is_some() && *o == output)
            {
                Some((_, group)) => group.push(action),
                None => groups.push((output, vec![action])),
            }
        }
        actions.extend(groups.into_iter().rev().flat_map(|(_, group)| group));
    }
    let mut law: serde_yaml_ng::Value = serde_yaml_ng::from_str(content)?;
    if let Some(articles) = law.get_mut("articles") {
        reverse(articles);
    }
    serde_yaml_ng::to_string(&law)
}
