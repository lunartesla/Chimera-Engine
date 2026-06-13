use crate::ir::module::Module;
use crate::Function; // Re-export from crate root
use crate::interpreter::{Interpreter, InterpreterError};
use crate::profiler::RuntimeProfiler;
use rand::Rng; // For random input generation
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub struct ValidationResult {
    pub passed: bool,
    pub corruption_probability: i32,
    pub test_count: u32,
    pub failed_tests: u32,
    pub failure_details: String,
}

#[derive(Clone)]
pub struct Validator {
    pub interpreter: Interpreter,
}

impl Validator {
    pub fn new() -> Self {
        Self {
            interpreter: Interpreter::new(),
        }
    }

    pub fn validate(
        &self,
        original: &Module,
        optimized: &Module,
        func_name: &str,
        runs: u32,
    ) -> ValidationResult {
        let orig_func = original.get_function(func_name);
        let opt_func = optimized.get_function(func_name);

        if orig_func.is_none() || opt_func.is_none() {
            return ValidationResult {
                passed: false,
                corruption_probability: 0,
                test_count: 0,
                failed_tests: 0,
                failure_details: format!("Function not found: {}", func_name),
            };
        }

        let failures = self.run_randomized_validation(orig_func.unwrap(), opt_func.unwrap(), runs);
        let corruption_prob = if runs > 0 {
            ((failures as f64 / runs as f64) * 100.0) as i32
        } else {
            0
        };

        ValidationResult {
            passed: failures == 0,
            corruption_probability: corruption_prob,
            test_count: runs,
            failed_tests: failures,
            failure_details: if failures > 0 {
                "Optimization changed behavior".to_string()
            } else {
                String::new()
            },
        }
    }

    pub fn run_randomized_validation(
        &self,
        orig_func: &Function,
        opt_func: &Function,
        runs: u32,
    ) -> u32 {
        let mut failures = 0;
        let mut rng = rand::thread_rng();

        for i in 0..runs {
            let mut orig_profiler = RuntimeProfiler::new();
            let mut opt_profiler = RuntimeProfiler::new();

            // Generate random inputs - for now, assuming simple integer inputs
            // The C++ version doesn't explicitly pass arguments, suggesting functions are hardcoded or use internal state.
            // For now, an empty HashMap for arguments is assumed. This might need refinement based on IRGenerator.
            let args: HashMap<String, i64> = HashMap::new(); // Placeholder for actual args

            let orig_result = self.interpreter.execute_function(
                orig_func,
                Some(&mut orig_profiler),
            );
            let opt_result = self.interpreter.execute_function(
                opt_func,
                Some(&mut opt_profiler),
            );

            match (orig_result, opt_result) {
                (Ok(o_val), Ok(p_val)) => {
                    if o_val != p_val {
                        failures += 1;
                        // In a real scenario, log detailed differences
                        eprintln!(
                            "  Run {} failed: orig={} opt={}",
                            i, o_val, p_val
                        );
                    }
                }
                (Err(e1), Err(e2)) => {
                    // Both failed with an error, might be acceptable if errors are equivalent
                    // For strict 1:1, check if error types/messages are identical
                    if e1.to_string() != e2.to_string() {
                         failures += 1;
                         eprintln!(
                            "  Run {} failed: orig threw '{}' opt threw '{}'",
                            i, e1, e2
                        );
                    }
                }
                (Err(e), _) => {
                    failures += 1;
                    eprintln!("  Run {} failed: orig threw '{}'", i, e);
                }
                (_, Err(e)) => {
                    failures += 1;
                    eprintln!("  Run {} failed: opt threw '{}'", i, e);
                }
            }
        }
        failures
    }
}