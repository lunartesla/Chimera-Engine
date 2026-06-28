use crate::ir::module::Module;
use crate::passes::{PassManager, PassRegistry, OptimizationLevel, PassError};
use crate::profiler::RuntimeProfiler;
use crate::validator::Validator;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Pass error: {0}")]
    PassError(#[from] PassError),
    #[error("Module not loaded")]
    ModuleNotLoaded,
    #[error("Function '{0}' not found in module")]
    FunctionNotFound(String),
}

impl Clone for OptimizationEngine {
    fn clone(&self) -> Self {
        let mut cloned_pass_manager = PassManager::new();
        // Re-populate pass manager based on the original opt_level and cloned registry
        cloned_pass_manager.populate_from_level(self.opt_level, &self.pass_registry);

        Self {
            original_module: self.original_module.clone(),
            working_module: self.working_module.clone(),
            pass_manager: cloned_pass_manager,
            pass_registry: self.pass_registry.clone(),
            profiler: self.profiler.clone(),
            validator: self.validator.clone(),
            opt_level: self.opt_level, // Copyable
            validation_runs: self.validation_runs, // Copyable
            hot_functions: self.hot_functions.clone(),
        }
    }
}

pub struct OptimizationEngine {
    original_module: Option<Module>,
    working_module: Option<Module>, // This will be the module that is actually optimized
    pass_manager: PassManager,
    pass_registry: PassRegistry, // The C++ engine stores a registry, so should Rust
    profiler: RuntimeProfiler,
    validator: Validator,
    opt_level: OptimizationLevel,
    validation_runs: u32, // default 10
    hot_functions: Vec<String>,
}

impl OptimizationEngine {
    pub fn new(opt_level: OptimizationLevel) -> Self {
        let registry = PassRegistry::new();
        let mut pass_manager = PassManager::new();
        pass_manager.populate_from_level(opt_level, &registry);

        Self {
            original_module: None,
            working_module: None,
            pass_manager,
            pass_registry: registry,
            profiler: RuntimeProfiler::new(),
            validator: Validator::new(),
            opt_level,
            // run_randomized_validation doesn't actually vary inputs between
            // runs (this IR's functions take no real call-time arguments —
            // see llvm_frontend.rs), so every one of the 10 "runs" executes
            // the exact same deterministic comparison and produces an
            // identical result. 10x was pure wasted interpretation plus 10x
            // duplicate eprintln spam for zero extra signal; 1 carries the
            // same information.
            validation_runs: 1,
            hot_functions: Vec::new(),
        }
    }

    pub fn load_module(&mut self, module: Module) {
        self.original_module = Some(module.clone());
        self.working_module = Some(module);
        self.profiler.reset();
        self.hot_functions.clear();
    }

    pub fn set_optimization_level(&mut self, level: OptimizationLevel) {
        self.opt_level = level;
        self.pass_manager.populate_from_level(level, &self.pass_registry);
    }

    pub fn set_validation_runs(&mut self, runs: u32) {
        self.validation_runs = runs;
    }

    pub fn profile(&mut self, func_name: &str) -> Result<(), EngineError> {
        let working_module = self.working_module.as_ref().ok_or(EngineError::ModuleNotLoaded)?;
        let func = working_module.get_function(func_name).ok_or(EngineError::FunctionNotFound(func_name.to_string()))?;

        // Need a mutable clone of func to pass to interpreter for execution
        let func_clone = func.clone();

        self.profiler.start_function(func_name);
        let _ = self.validator.interpreter.execute_function(working_module, &func_clone, &[], Some(&mut self.profiler)); // Note: Interpreter returns Result<i64, InterpreterError>
        self.profiler.end_function(func_name);

        Ok(())
    }


    pub fn identify_hot_paths(&mut self, top_n: usize) {
        self.hot_functions = self.profiler.get_hot_functions(top_n);
    }

    pub fn optimize_hot_paths(&mut self) -> Result<bool, EngineError> {
        if self.hot_functions.is_empty() {
            // C++ prints a message, then returns false.
            eprintln!("No hot functions identified. Profile first.");
            return Ok(false);
        }

        let mut changed_any_function = false;
        // C++ iterates through module functions and runs passes on each.
        // It's not clear if `optimizeHotPaths` runs the *entire* pass pipeline on *only* hot functions,
        // or on all functions in the module. From PassManager::run(Module& module) it looks like
        // the pipeline is applied to the whole module.

        // The C++ `OptimizationEngine::optimizeHotPaths` just calls `passManager->run(*workingModule)`.
        // The `PassManager::run` method then iterates over all functions in the module.
        // It means `hotFunctions` is used only for identification, not for restricting optimization.
        if let Some(module) = self.working_module.as_mut() {
            changed_any_function = self.pass_manager.run_all(module)?;
        }

        Ok(changed_any_function)
    }

    pub fn validate_optimization(&self, func_name: &str) -> bool {
        let result = self.validate_optimization_result(func_name);

        // C++ prints validation results, so should Rust.
        eprintln!("Validation result: {}", if result.passed { "PASSED" } else { "FAILED" });
        eprintln!("  Test runs: {}", result.test_count);
        eprintln!("  Failures: {}", result.failed_tests);
        eprintln!("  Corruption probability: {}%", result.corruption_probability);
        if !result.failure_details.is_empty() {
            eprintln!("  Details: {}", result.failure_details);
        }

        result.passed
    }

    /// Same correctness check as `validate_optimization`, without the
    /// eprintln! diagnostics. Needed because this now runs on every scored
    /// pipeline, every generation (see SelfEvolvingEngine::score_pipeline) —
    /// the printing version would spam thousands of lines per second.
    pub fn validate_optimization_result(&self, func_name: &str) -> crate::validator::ValidationResult {
        let original = self.original_module.as_ref().expect("Original module not loaded for validation");
        let optimized = self.working_module.as_ref().expect("Working module not loaded for validation");

        self.validator.validate(original, optimized, func_name, self.validation_runs)
    }

    pub fn get_module(&self) -> Option<&Module> {
        self.working_module.as_ref()
    }

    // New method to retrieve a mutable reference to the working module
    pub fn get_module_mut(&mut self) -> Option<&mut Module> {
        self.working_module.as_mut()
    }

    pub fn get_pass_manager(&self) -> &PassManager {
        &self.pass_manager
    }

    pub fn pass_manager_mut(&mut self) -> &mut PassManager {
        &mut self.pass_manager
    }

    pub fn validator(&self) -> &Validator {
        &self.validator
    }

    pub fn get_registry(&self) -> &PassRegistry {
        &self.pass_registry
    }

    /// Runs all passes on the working module. Avoids double-mutable-borrow
    /// that occurs when caller tries to get_module_mut() and pass_manager_mut() simultaneously.
    pub fn run_passes_on_module(&mut self) -> bool {
        if let Some(module) = self.working_module.as_mut() {
            self.pass_manager.run_all(module).unwrap_or(false)
        } else {
            false
        }
    }

    pub fn get_predictor_stub(&self) -> Option<()> { None }
}

impl From<Module> for OptimizationEngine {
    fn from(module: Module) -> Self {
        let mut engine = OptimizationEngine::new(OptimizationLevel::Conservative);
        engine.load_module(module);
        engine
    }
}