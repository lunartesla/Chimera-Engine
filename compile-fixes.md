# Metamorphic Engine — Compile Error Fixes

## Fix 1: `goal_definition.rs` — Add `#[derive(Clone)]`

```rust
// Line 5 — add Clone
#[derive(Clone)]
pub struct GoalDefinition {
    pub id: String,
    pub description: String,
    pub success_threshold: f64,
    pub max_generations: i32,
    // fitness_fn can't derive Clone — wrap it in Arc
    pub fitness_fn: Arc<dyn Fn(&Module) -> f64 + Send + Sync>,
}
```

---

## Fix 2: `interpreter.rs` — Wrong import path

```rust
// WRONG:
use crate::ir::module::{Module, ir::function::Function};

// CORRECT:
use crate::ir::{module::Module, function::Function};
```

---

## Fix 3: `passes/block_merging.rs` — String comparison borrows

Every `&b.name == &target_name` style comparison — just remove the `&` from both sides:

```rust
// Pattern throughout the file — global find+replace:
// WRONG:  &b.name == &target_name
// CORRECT: b.name == target_name

// WRONG:  label == &target_name
// CORRECT: label == target_name

// WRONG:  then_label == &target_name
// CORRECT: then_label == target_name

// WRONG:  else_label == &target_name
// CORRECT: else_label == target_name

// WRONG:  other_block.name == &target_name
// CORRECT: other_block.name == *target_name   ← note the deref here
```

---

## Fix 4: `self_evolving_engine.rs` — `rng.choose()` wrong API

`choose` is on slices via the `SliceRandom` trait, not on `ThreadRng`.

Add import at top of file:
```rust
use rand::seq::SliceRandom;
```

Then fix every call site:
```rust
// WRONG:
applied_mutation_type = *rng.choose(&[Mutation::Add, ...]);

// CORRECT:
let options = [Mutation::Add, Mutation::Remove, Mutation::Reorder, Mutation::Duplicate, Mutation::Tune];
applied_mutation_type = *options.choose(&mut rng).unwrap();

// For pass_id choose:
// WRONG:
if let Some(pass_id) = rng.choose(&all_pass_ids) {

// CORRECT:
if let Some(pass_id) = all_pass_ids.choose(&mut rng) {
```

---

## Fix 5: `self_evolving_engine.rs` — `generations` not in scope

Lines 406 and 480. The variable `generations` needs to be accessible. Either pass it as a parameter or use a field. Quickest fix — add a `total_generations` field to `SelfEvolvingEngine` and reference it:

```rust
// In struct definition add:
pub total_generations: usize,

// In new():
total_generations: 1000, // or whatever default

// Replace the broken lines:
// WRONG:
gen as f64 / generations as f64

// CORRECT:
gen as f64 / self.total_generations as f64
```

---

## Fix 6: `neural_predictor.rs` — Wrong neuralneat API

The neuralneat 0.1.0 crate doesn't expose `neuralneat::neat` or `neuralneat::genome` publicly. Run:
```
cargo doc --open
```
in the Rust dir to see the actual public API. Then fix the imports to match.

In the meantime, stub it out so the rest compiles:

```rust
// Replace the broken imports with:
use neuralneat::Neat;
// Remove the Genome import entirely — it's private

// If neuralneat::Neat doesn't exist at root either, check:
// neuralneat::NeatAlgorithm or similar
// The crate is small (0.1.0) — check lib.rs via cargo doc
```

---

## Fix 7: `neural_predictor.rs` — `build_expanded_features` is private

```rust
// WRONG:
fn build_expanded_features(

// CORRECT:
pub fn build_expanded_features(
```

---

## Fix 8: `neural_predictor.rs` — `load_brain` is associated function

```rust
// WRONG (in strain.rs):
origin_engine.get_predictor().load_brain(Path::new(tmp_model_path))

// CORRECT — load_brain returns a new Self, call it as:
match NeuralPredictor::load_brain(Path::new(tmp_model_path)) {
    Ok(loaded) => { *origin_engine.get_predictor_mut() = loaded; }
    Err(e) => { log::warn!("Could not load brain: {e}"); }
}
```

This requires `get_predictor_mut()` on whatever type `origin_engine` is — add that getter.

---

## Fix 9: `engine.rs` — Private fields accessed externally

Add these getter/setter methods to `OptimizationEngine`:

```rust
impl OptimizationEngine {
    // existing methods...

    pub fn pass_manager(&self) -> &PassManager {
        &self.pass_manager
    }

    pub fn pass_manager_mut(&mut self) -> &mut PassManager {
        &mut self.pass_manager
    }

    pub fn validator(&self) -> &Validator {
        &self.validator
    }

    pub fn get_predictor(&self) -> &NeuralPredictor {
        &self.predictor
    }

    pub fn get_predictor_mut(&mut self) -> &mut NeuralPredictor {
        &mut self.predictor
    }
}
```

Then in `engine_server.rs` and `self_evolving_engine.rs`, replace direct field access with these methods:
```rust
// WRONG:
session.opt_engine.pass_manager = custom_pm;

// CORRECT:
*session.opt_engine.pass_manager_mut() = custom_pm;
```

---

## Fix 10: `self_evolving_engine.rs` — `base_engine` and `best_known_fitness` private

Add to `SelfEvolvingEngine`:
```rust
pub fn base_engine(&self) -> &OptimizationEngine {
    &self.base_engine
}

pub fn best_known_fitness(&self) -> f64 {
    self.best_known_fitness
}
```

---

## Fix 11: `strain.rs` — `should_promote` needs `&mut self`

```rust
// WRONG:
pub fn should_promote(&self) -> bool {

// CORRECT:
pub fn should_promote(&mut self) -> bool {
```

---

## Fix 12: `evolution_daemon.rs` — `pass_registry` not a field

Add it to `EvolutionDaemon` struct and init in `new()`:

```rust
pub struct EvolutionDaemon {
    // existing fields...
    pass_registry: PassRegistry,
}

// In new():
pass_registry: PassRegistry::new(),
```

---

## Fix 13: `evolution_daemon.rs` — Move issues with `opt_engine` and `strain_engine`

```rust
// WRONG (opt_engine moved into SelfEvolvingEngine::new then cloned):
let mut opt_engine = OptimizationEngine::new(...);
// ...
opt_engine,        // moved here into SE::new
// ...
opt_engine: opt_engine.clone(),  // ERROR: already moved

// CORRECT — clone before moving:
let mut opt_engine = OptimizationEngine::new(...);
let opt_engine_clone = opt_engine.clone(); // clone first
// pass opt_engine into SE::new, use opt_engine_clone for ActiveStrain

// For OptimizationEngine to be cloneable, add to engine.rs:
#[derive(Clone)]
pub struct OptimizationEngine { ... }
```

```rust
// WRONG (strain_engine moved into Arc then used):
let strain_arc = Arc::new(Mutex::new(strain_engine)); // moved
// ...
engine: strain_engine,  // ERROR: already moved

// CORRECT:
let active_strain = ActiveStrain {
    engine: strain_engine, // use it here first
    // ...
};
let strain_arc = Arc::new(Mutex::new(active_strain));
// OR just don't put it in Arc if ActiveStrain already has the engine
```

---

## Fix 14: `teacher.rs` — `Message` struct missing `#[derive(Deserialize)]`

```rust
// Find the inner Message struct inside call_llm() and add:
#[derive(Deserialize)]
struct Message {
    // fields...
}
```

Also need to add `chat` method since it's called in evolution_daemon and terminal_chat:

```rust
impl Teacher {
    pub fn chat(&mut self, system: &str, user: &str) -> Option<String> {
        self.call_llm(system, user).ok()
    }
}
```

---

## Fix 15: `dashboard.rs` — `stdout()` variable shadows function

```rust
// WRONG (line 63 and 72 both declare stdout):
let mut stdout = stdout();  // line 63
// ...
let mut stdout = stdout();  // line 72 — stdout is now the variable, not the fn

// CORRECT — remove the second declaration at line 72, reuse the one from line 63
// OR rename the variable:
let mut out = std::io::stdout();
```

---

## Fix 16: `dashboard.rs` — `print_line` closure needs `mut`

```rust
// WRONG:
let print_line = |s: String| {

// CORRECT:
let mut print_line = |s: String| {
```

---

## Fix 17: `dashboard.rs` — `print_event` is associated function

```rust
// WRONG (in terminal_chat.rs):
self.dashboard.lock().unwrap().print_event("h3", msg);

// CORRECT:
Dashboard::print_event("h3", msg);
```

Apply everywhere `print_event` is called through the dashboard lock.

---

## Fix 18: `dashboard.rs` — `state` field is private

Add getter to Dashboard:
```rust
pub fn get_state(&self) -> std::sync::MutexGuard<DashboardState> {
    self.state.lock().unwrap()
}
```

Then in terminal_chat.rs:
```rust
// WRONG:
let ds = self.dashboard.lock().unwrap().state.lock().unwrap().clone();

// CORRECT:
let ds = self.dashboard.lock().unwrap().get_state().clone();
```

---

## Fix 19: `terminal_chat.rs` — `stdout` cast

```rust
// WRONG:
(stdout as &mut dyn IoWrite).flush().unwrap();

// CORRECT:
(&mut stdout as &mut dyn IoWrite).flush().unwrap();
```

Apply to all 4 occurrences.

---

## Fix 20: `evolution_daemon.rs` — `persistent_predictor` private

```rust
// In EvolutionDaemon, change:
persistent_predictor: NeuralPredictor,

// To:
pub persistent_predictor: NeuralPredictor,
```

---

## Fix 21: `evolution_daemon.rs` — `base_engine` private on SelfEvolvingEngine

Already covered in Fix 10 — use the `base_engine()` getter.

```rust
// WRONG:
let module_for_hash = se.base_engine.get_module()...

// CORRECT:
let module_for_hash = se.base_engine().get_module()...
```

---

## Fix 22: `evolution_daemon.rs` — `archive.directory` private

Add getter to `BlueprintArchive`:
```rust
pub fn directory(&self) -> &Path {
    &self.directory
}
```

Then:
```rust
// WRONG:
let generated_dir = self.archive.directory.join("generated");

// CORRECT:
let generated_dir = self.archive.directory().join("generated");
```

---

## Fix 23: `engine_server.rs` — `ModuleHandle` missing `Clone`

```rust
// Add to ModuleHandle:
#[derive(Clone)]
pub struct ModuleHandle {
    // fields
}
```

---

## Fix 24: `lib.rs` — Ambiguous glob re-exports

Replace wildcard re-exports with explicit ones:

```rust
// WRONG:
pub use evolution_daemon::*;
pub use terminal_chat::*;
pub use engine_server::*;

// CORRECT — be explicit about what you export:
pub use evolution_daemon::EvolutionDaemon;
pub use terminal_chat::TerminalChat;
pub use engine_server::EngineServer;
```

---

## Fix 25: `strain.rs` — `StrainEngine` needs `Clone` or fix move

```rust
#[derive(Clone)]
pub struct StrainEngine {
    // all fields must also implement Clone
}
```

If any field can't derive Clone (like a `Box<dyn Trait>`), wrap it in `Arc` instead.

---

## Fix 26: `engine_server.rs` — `best_known_fitness` private

Already covered in Fix 10:
```rust
// WRONG:
se_engine.best_known_fitness

// CORRECT:
se_engine.best_known_fitness()
```

---

## Fix 27: `engine_server.rs` — `validator` private

Already covered in Fix 9 — use `opt_engine.validator()` getter.

---

## Priority Order to Apply These

Do them in this order to avoid cascading issues:

1. Fix 2 (interpreter import) — unblocks interpreter compilation
2. Fix 1 (GoalDefinition Clone) — unblocks evolution_daemon and strain
3. Fix 4 (SliceRandom import) — unblocks self_evolving_engine
4. Fix 3 (block_merging String) — quick find+replace
5. Fix 9 (engine getters) — unblocks server and evolving engine
6. Fix 10 (SEEngine getters) — unblocks daemon and server
7. Fix 6 (neuralneat API) — run cargo doc first to confirm actual API
8. Fix 7 (build_expanded_features pub)
9. Fix 14 (Teacher Message + chat method)
10. Fix 15-16 (dashboard stdout/closure)
11. Fix 17-19 (terminal_chat)
12. Fix 13 (move issues)
13. Fix 23-25 (Clone derives)
14. Fix 24 (lib.rs glob)
15. Everything else

After applying all fixes run `cargo build` again — there may be a second wave of smaller errors but the count should drop dramatically.
