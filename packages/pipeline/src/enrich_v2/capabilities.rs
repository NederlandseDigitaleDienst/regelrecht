//! What a step needs against what the runtime grants.
//!
//! The failure this module exists for is recorded in issue #1036 and was
//! measured in round 2 of the enricher archive. The enrichment prompt tells
//! the agent to read `law-mvt-research/SKILL.md` and search for parliamentary
//! documents; that skill declares `WebFetch` and `WebSearch`; the agent is
//! spawned with `Read,Edit,Write,Grep,Glob`. The instruction is therefore
//! impossible. Nothing compared the two, the step ran to a clean exit, and
//! what came back was a `kst-` citation with a URL for a document the agent
//! had not read. That the number happened to be right makes it worse: the
//! agent applied the "dossier number plus 3" convention, which holds often
//! enough to survive a spot check and fails silently on any dossier where
//! number 3 is the Council of State's advice.
//!
//! Removing the two offending lines from the prompt would fix this round and
//! nothing after it: the next skill edit reintroduces the mismatch silently.
//! So the comparison happens here, from the skill's own frontmatter, and it
//! produces one of three outcomes per step.
//!
//! A step whose own minimum is not granted cannot produce its artefact at
//! all. Where the step is required the chain fails and says why; where it is
//! optional it is left out of the prompt and recorded as left out, which is
//! the difference between a known gap and a silent one.
//!
//! A step whose minimum is granted while its skill asks for more is the
//! interesting case, and it is where `law-generate` sits: writing YAML needs
//! no shell, and the generate-validate-test loop in that skill needs `just`.
//! Cutting the loop out of the skill would be wrong, because the skill is
//! also used by hand where the shell does exist. Instead the prompt names the
//! part that cannot run here and says what takes its place, so the agent does
//! not report a validation it never performed.

use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Tools the enrichment lane grants an agent run.
///
/// Kept beside the `--allowedTools` argument in `enrich.rs` rather than
/// derived from it, because the two providers spell their allowlists
/// differently while the capability question is the same for both.
///
/// This list was fiction until [`ungranted`] existed. `--allowedTools` is an
/// auto-approve list and not a restriction: a tool left off it is still there,
/// it merely has to be asked for, and the settings in the checkout already
/// answered yes. So the plan said "Generate machine_readable: degraded,
/// missing Bash" while the agent made twenty successful `Bash` calls in the
/// same session, and "MvT research: skipped, this runtime grants no WebFetch"
/// while nothing stopped it from fetching. Both halves of that are damaging:
/// the agent is told a step runs degraded when it does not, and the operator
/// is told a step was left out when it was available all along. What the
/// planner reports and what the process withholds are now the same set.
pub const ENRICH_GRANT: &[&str] = &["Read", "Edit", "Write", "Grep", "Glob"];

/// Every tool the chain asks for anywhere that this lane does not grant.
///
/// The union over both halves of the question, because they fail differently
/// and both were live. A step's own `needs` are what it cannot write its
/// artefact without, and leaving those reachable lets a skipped step run
/// anyway. A skill's `allowed-tools` are what it asks for on top, and leaving
/// those reachable lets a degraded step do the thing the prompt just told it
/// it could not do — and then report it, which is the round-2 failure exactly.
///
/// A tool named here is passed to the provider as denied, so the plan is not a
/// description of the runtime but the thing that sets it.
#[must_use]
pub fn ungranted(grant: &BTreeSet<String>, steps: &[(&StepSpec, Option<String>)]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for (spec, markdown) in steps {
        let declared = markdown.as_deref().map(skill_tools).unwrap_or_default();
        for tool in spec.needs.iter().map(|t| (*t).to_owned()).chain(declared) {
            if !grant.contains(&tool) {
                out.insert(tool);
            }
        }
    }
    out.into_iter().collect()
}

/// One step of the chain, with what it needs stated separately from what the
/// skill it reads happens to declare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepSpec {
    /// Name used in the prompt heading and in the report.
    pub name: &'static str,
    /// Repository-relative path of the skill this step delegates to.
    pub skill: &'static str,
    /// The minimum this step needs to produce its artefact at all. Smaller
    /// than the skill's declaration whenever the skill covers uses beyond
    /// this lane.
    pub needs: &'static [&'static str],
    /// A required step that cannot run fails the chain. An optional step that
    /// cannot run is left out and reported.
    pub required: bool,
}

/// What the planner decided about one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepPlan {
    /// Everything the step and its skill ask for is granted.
    Run,
    /// The step can produce its artefact, but part of the skill it reads
    /// cannot run here. The prompt must say which part and what replaces it.
    Degraded {
        /// Tools the skill declares that this runtime does not grant.
        missing: Vec<String>,
    },
    /// The step cannot produce its artefact and is optional.
    Skipped {
        /// Tools the step itself needs that this runtime does not grant.
        missing: Vec<String>,
    },
    /// The step cannot produce its artefact and is required.
    Blocked {
        /// Tools the step itself needs that this runtime does not grant.
        missing: Vec<String>,
    },
}

impl StepPlan {
    /// Whether this step contributes a section to the prompt.
    #[must_use]
    pub fn is_in_prompt(&self) -> bool {
        matches!(self, StepPlan::Run | StepPlan::Degraded { .. })
    }
}

/// Read the `allowed-tools:` list from a skill's YAML frontmatter.
///
/// Deliberately a line scan rather than a YAML parse. The frontmatter of a
/// skill is written by hand and the rest of it (multi-line `description`
/// blocks, unquoted colons in prose) is not worth modelling to answer one
/// question. A skill without the key yields an empty set, which the caller
/// reads as "asks for nothing beyond its own minimum".
#[must_use]
pub fn skill_tools(skill_markdown: &str) -> BTreeSet<String> {
    let mut in_frontmatter = false;
    for line in skill_markdown.lines() {
        if line.trim_end() == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }
        if let Some(rest) = line.strip_prefix("allowed-tools:") {
            return rest
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect();
        }
    }
    BTreeSet::new()
}

/// Decide what happens to one step, given the grant and the skill text.
///
/// `skill_markdown` is `None` when the skill file could not be read. That is
/// treated as a skill declaring nothing rather than as an error: a missing
/// skill file is caught by the step failing to produce its artefact, and
/// guessing here would turn a readable failure into an unreadable one.
#[must_use]
pub fn plan_step(
    spec: &StepSpec,
    grant: &BTreeSet<String>,
    skill_markdown: Option<&str>,
) -> StepPlan {
    let missing_for_step: Vec<String> = spec
        .needs
        .iter()
        .filter(|t| !grant.contains(**t))
        .map(|t| (*t).to_owned())
        .collect();

    if !missing_for_step.is_empty() {
        return if spec.required {
            StepPlan::Blocked {
                missing: missing_for_step,
            }
        } else {
            StepPlan::Skipped {
                missing: missing_for_step,
            }
        };
    }

    let declared = skill_markdown.map(skill_tools).unwrap_or_default();
    let missing_for_skill: Vec<String> = declared
        .into_iter()
        .filter(|t| !grant.contains(t))
        .collect();

    if missing_for_skill.is_empty() {
        StepPlan::Run
    } else {
        StepPlan::Degraded {
            missing: missing_for_skill,
        }
    }
}

/// The steps of the enrichment chain, in prompt order.
///
/// `needs` is what the step must have to write its own artefact, which is why
/// every entry here is a subset of the file tools: this lane's whole design is
/// that the worker fetches and validates while the agent reads and writes.
pub const CHAIN: &[StepSpec] = &[
    // No MvT step. It was here as a retrieval step needing `WebFetch`, and this
    // lane grants no network, so every run since round 1 has reported it
    // skipped and none has ever run it. Reporting a skip for something that
    // cannot happen this way is noise, and it hid that a third of the designed
    // chain was missing from every measurement.
    //
    // Network is not the fix. The prompt tells the agent not to cite what it
    // was not given, because a recalled kamerstuk reads exactly like a read
    // one to whoever comes after, and round 2 produced precisely that. Handing
    // it retrieval makes that likelier.
    //
    // The replacement is RFC-030: harvest the parliamentary history into the
    // corpus per law version, and the agent reads it from disk the way it reads
    // the statute. Then the step returns as a file step, and it belongs in this
    // list again.
    StepSpec {
        name: "Generate machine_readable",
        skill: ".claude/skills/law-generate/SKILL.md",
        needs: &["Read", "Edit", "Write"],
        required: true,
    },
    // `Write` is not in here and it is in the step above, which is the whole
    // difference between the two: generating puts a `machine_readable` section
    // into a law that has none, reverse validation corrects one that is
    // already there. The skill declares `Read, Edit, Bash, Grep, Glob` and no
    // `Write`, and `needs` used to say otherwise — a step's minimum asked for
    // a tool its own instructions never use, and nothing compared the two.
    StepSpec {
        name: "Reverse validation",
        skill: ".claude/skills/law-reverse-validate/SKILL.md",
        needs: &["Read", "Edit"],
        required: true,
    },
];

/// Sentence appended to a degraded step's prompt section.
///
/// Naming the tool is not enough on its own: an agent told that `Bash` is
/// unavailable still has to decide what to do about the instruction that
/// needed it, and the failure mode measured in round 2 is that it decides to
/// report success. So the replacement is stated too.
#[must_use]
pub fn degraded_note(missing: &[String]) -> String {
    let mut note =
        String::from("This runtime does not grant every tool the skill declares. Missing here: ");
    note.push_str(&missing.join(", "));
    note.push_str(
        ".\nSkip the parts of the skill that need them and do not simulate their result.\n\
         Validation and testing run outside this session: the worker validates the file \n\
         you write and sends you the findings in a following pass. Reporting a check you \n\
         did not run is the one outcome worse than not running it.",
    );
    note
}

/// Human-readable record of what the planner left out, for the run result.
///
/// A skipped step that leaves no trace is indistinguishable from a step that
/// ran and found nothing, which is the distinction round 2 lost.
#[must_use]
pub fn plan_report(planned: &[(&StepSpec, StepPlan)]) -> String {
    let mut out = String::new();
    for (spec, plan) in planned {
        match plan {
            StepPlan::Run => {}
            StepPlan::Degraded { missing } => {
                let _ = writeln!(
                    out,
                    "{}: degraded, missing {}",
                    spec.name,
                    missing.join(", ")
                );
            }
            StepPlan::Skipped { missing } => {
                let _ = writeln!(
                    out,
                    "{}: skipped, this runtime grants no {}",
                    spec.name,
                    missing.join(", ")
                );
            }
            StepPlan::Blocked { missing } => {
                let _ = writeln!(
                    out,
                    "{}: blocked, required step needs {}",
                    spec.name,
                    missing.join(", ")
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(tools: &[&str]) -> BTreeSet<String> {
        tools.iter().map(|t| (*t).to_owned()).collect()
    }

    fn enrich_grant() -> BTreeSet<String> {
        grant(ENRICH_GRANT)
    }

    #[test]
    fn reads_allowed_tools_from_frontmatter() {
        let skill = "---\nname: x\nallowed-tools: Read, Write, WebFetch\n---\n\n# Body\n";
        assert_eq!(skill_tools(skill), grant(&["Read", "Write", "WebFetch"]));
    }

    #[test]
    fn ignores_allowed_tools_outside_frontmatter() {
        // The body of law-generate quotes tool names in prose; only the
        // frontmatter declares.
        let skill = "---\nname: x\n---\n\nallowed-tools: Bash, WebFetch\n";
        assert!(skill_tools(skill).is_empty());
    }

    #[test]
    fn skill_without_declaration_yields_empty() {
        assert!(skill_tools("---\nname: x\n---\n\nbody\n").is_empty());
    }

    #[test]
    fn no_step_in_the_chain_needs_retrieval() {
        // The MvT step used to sit here needing `WebFetch`, and this lane
        // grants no network, so it reported a skip on every run and never
        // ran once. A step that cannot happen does not belong in the plan.
        //
        // This holds the shape rather than the absence: a retrieval step
        // added here would report a skip every run again, and the answer is
        // to read the harvested material from disk (RFC-030).
        for spec in CHAIN {
            for needed in spec.needs {
                assert!(
                    ENRICH_GRANT.contains(needed),
                    "{} needs {needed}, which this lane does not grant",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn generate_step_runs_degraded_rather_than_blocked() {
        // Writing YAML needs no shell; the validate loop in the skill does.
        // The step must still run, with the loop named as unavailable.
        //
        // `CHAIN[0]`, and the index is the point. This asserted on `CHAIN[1]`
        // with the frontmatter of `law-generate` — left over from the removed
        // MvT step, which used to be first — and it passed because the two
        // steps declare the same `needs` and therefore plan identically. The
        // one required step that actually writes YAML had no statement about
        // its own degradation.
        let spec = &CHAIN[0];
        assert_eq!(spec.name, "Generate machine_readable");
        let plan = plan_step(
            spec,
            &enrich_grant(),
            Some("---\nallowed-tools: Read, Edit, Write, Bash, Grep, Glob\n---\n"),
        );
        assert_eq!(
            plan,
            StepPlan::Degraded {
                missing: vec!["Bash".to_owned()]
            }
        );
        assert!(plan.is_in_prompt());
    }

    #[test]
    fn required_step_without_its_minimum_blocks() {
        let spec = &CHAIN[0];
        let plan = plan_step(spec, &grant(&["Read"]), None);
        assert_eq!(
            plan,
            StepPlan::Blocked {
                missing: vec!["Edit".to_owned(), "Write".to_owned()]
            }
        );
        assert!(!plan.is_in_prompt());
    }

    /// Every step planned against its own skill file, which is the comparison
    /// the two tests above cannot make: they hand one frontmatter to whichever
    /// step they name, so a step planned with a neighbour's declarations reads
    /// exactly like a step planned with its own.
    ///
    /// Two things have to hold, and the second is the one that was untrue. A
    /// step's `needs` is its minimum, so a tool in it that the skill never
    /// declares is a minimum nobody asked for; and this lane grants no shell,
    /// so a skill that names `Bash` degrades and one that does not, runs.
    #[test]
    fn elke_stap_plant_tegen_zijn_eigen_skillbestand() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for spec in CHAIN {
            let markdown = std::fs::read_to_string(root.join(spec.skill))
                .unwrap_or_else(|e| panic!("{} is niet te lezen: {e}", spec.skill));
            let declared = skill_tools(&markdown);
            for needed in spec.needs {
                assert!(
                    declared.contains(*needed),
                    "{} eist {needed} als minimum, maar {} noemt dat gereedschap niet",
                    spec.name,
                    spec.skill
                );
            }

            let plan = plan_step(spec, &enrich_grant(), Some(&markdown));
            let expected = if declared.contains("Bash") {
                StepPlan::Degraded {
                    missing: vec!["Bash".to_owned()],
                }
            } else {
                StepPlan::Run
            };
            assert_eq!(plan, expected, "{}", spec.name);
            assert!(plan.is_in_prompt(), "{}", spec.name);
        }
    }

    #[test]
    fn full_grant_runs_every_step_undegraded() {
        let wide = grant(&[
            "Read",
            "Edit",
            "Write",
            "Grep",
            "Glob",
            "Bash",
            "WebFetch",
            "WebSearch",
        ]);
        for spec in CHAIN {
            let plan = plan_step(
                spec,
                &wide,
                Some(
                    "---\nallowed-tools: Read, Edit, Write, Bash, WebFetch, WebSearch, Grep, Glob\n---\n",
                ),
            );
            assert_eq!(plan, StepPlan::Run, "{} should run", spec.name);
        }
    }

    #[test]
    fn unreadable_skill_does_not_degrade_a_step() {
        let spec = &CHAIN[1];
        assert_eq!(plan_step(spec, &enrich_grant(), None), StepPlan::Run);
    }

    #[test]
    fn report_names_what_was_left_out() {
        let planned = vec![
            (
                &CHAIN[0],
                StepPlan::Degraded {
                    missing: vec!["Bash".to_owned()],
                },
            ),
            (&CHAIN[1], StepPlan::Run),
        ];
        let report = plan_report(&planned);
        assert!(report.contains("Generate machine_readable: degraded"));
        assert!(report.contains("Bash"));
        assert!(!report.contains("Reverse validation"));
    }

    #[test]
    fn what_the_plan_reports_missing_is_what_the_runtime_withholds() {
        // The measured contradiction: the plan said "Generate machine_readable:
        // degraded, missing Bash" and "MvT research: skipped, this runtime
        // grants no WebFetch", and the same session then made twenty
        // successful Bash calls. Both halves of the report come from the same
        // set as the deny list now, so they cannot say different things.
        let steps: Vec<(&StepSpec, Option<String>)> = vec![
            (
                &CHAIN[0],
                Some("---\nallowed-tools: Read, Edit, Write, Bash, Grep, Glob\n---\n".to_owned()),
            ),
            (
                &CHAIN[1],
                Some("---\nallowed-tools: Read, Edit, Write, Grep, Glob\n---\n".to_owned()),
            ),
        ];
        let denied = ungranted(&enrich_grant(), &steps);
        assert_eq!(denied, vec!["Bash"]);

        // The direction that matters: nothing the report calls missing may
        // still be reachable. The deny list may be wider — a skipped step
        // reports only its own minimum while its skill asks for more — but it
        // can never be narrower, because that is the gap the report lies over.
        let planned: Vec<(&StepSpec, StepPlan)> = steps
            .iter()
            .map(|(spec, md)| (*spec, plan_step(spec, &enrich_grant(), md.as_deref())))
            .collect();
        for (_, plan) in &planned {
            let missing = match plan {
                StepPlan::Run => continue,
                StepPlan::Degraded { missing }
                | StepPlan::Skipped { missing }
                | StepPlan::Blocked { missing } => missing,
            };
            for tool in missing {
                assert!(
                    denied.contains(tool),
                    "{tool} is reported missing but reachable"
                );
            }
        }
    }

    #[test]
    fn a_fully_granted_lane_withholds_nothing() {
        let wide = grant(&["Read", "Edit", "Write", "Grep", "Glob", "Bash", "WebFetch"]);
        let steps: Vec<(&StepSpec, Option<String>)> = CHAIN
            .iter()
            .map(|s| {
                (
                    s,
                    Some("---\nallowed-tools: Read, Edit, Write, Bash, WebFetch\n---\n".to_owned()),
                )
            })
            .collect();
        assert!(ungranted(&wide, &steps).is_empty());
    }

    #[test]
    fn degraded_note_forbids_simulating_the_result() {
        let note = degraded_note(&["Bash".to_owned()]);
        assert!(note.contains("Bash"));
        assert!(note.contains("do not simulate"));
        assert!(note.contains("worker validates"));
    }
}
