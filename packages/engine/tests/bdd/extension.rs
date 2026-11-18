//! The actions a runner adds to the shared dispatch, and the state they keep
//! in the world. The engine's runner adds none. The cell's runner
//! (`packages/cel/tests/bdd/extension.rs`) brings its own version of this
//! module, with the steps of `packages/cel/bdd/grammar.yaml`. The shared world
//! and dispatch name only `crate::extension`, so each runner decides what it
//! holds.

use regelrecht_engine::LawExecutionService;

use crate::dispatch::ArgValue;
use crate::helpers::table::Rows;
use crate::world::RegelrechtWorld;

/// What the extra actions keep in the world: nothing here.
pub struct State;

impl State {
    /// Set up before the first step; loads nothing extra.
    pub fn new(_service: &mut LawExecutionService) -> Self {
        Self
    }
}

/// Run `action` if it is one of this runner's own; `false` when it is not.
pub fn dispatch(
    _world: &mut RegelrechtWorld,
    _action: &str,
    _args: Vec<ArgValue>,
    _table: Option<Rows>,
) -> bool {
    false
}
