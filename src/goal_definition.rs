use crate::ir::module::Module;
use std::{collections::HashSet, sync::Arc};
use crate::ir::value::Instruction;

#[derive(Clone)]
pub struct GoalDefinition {
    pub id: String,
    pub description: String,
    pub success_threshold: f64,
    pub max_generations: i32,
    pub fitness_fn: Arc<dyn Fn(&Module) -> f64 + Send + Sync>,
}

impl GoalDefinition {
    pub fn minimize_instructions(target_count: i64) -> Self {
        Self {
            id: "minimize_instrs".to_string(),
            description: format!("Minimize instruction count to {}", target_count),
            success_threshold: -(target_count as f64),
            max_generations: 500,
            fitness_fn: Arc::new(move |module: &Module| {
                -(module.instruction_count() as f64)
            }),
        }
    }

    pub fn minimize_time(target_us: f64) -> Self {
        Self {
            id: "minimize_time".to_string(),
            description: format!("Minimize execution time to {} us", target_us),
            success_threshold: -target_us,
            max_generations: 500,
            fitness_fn: Arc::new(move |module: &Module| {
                // In C++, this involves Interpreter execution and Profiler.
                // For now, return a placeholder until Interpreter is ported.
                // Placeholder behavior: just use instruction count as a proxy
                -(module.instruction_count() as f64 * 0.1) // Placeholder
            }),
        }
    }

    pub fn maximize_branch_elimination() -> Self {
        Self {
            id: "max_branch_elim".to_string(),
            description: "Eliminate as many branches as possible".to_string(),
            success_threshold: -2.0, // C++ used -2.0 as threshold
            max_generations: 500,
            fitness_fn: Arc::new(move |module: &Module| {
                -(module.branch_count() as f64)
            }),
        }
    }

    pub fn token_communication() -> Self {
        Self {
            id: "token_comm".to_string(),
            description: "Maximize surviving named token variables".to_string(),
            success_threshold: 5.0,
            max_generations: 500,
            fitness_fn: Arc::new(move |module: &Module| {
                let mut written_vars = HashSet::new();
                let mut read_vars = HashSet::new();

                for func in &module.functions {
                    for bb in &func.basic_blocks {
                        for instr in &bb.instructions {
                            match instr {
                                Instruction::Store { var_name, value: _ } => {
                                    written_vars.insert(var_name.clone());
                                }
                                Instruction::Variable { name } => {
                                    read_vars.insert(name.clone());
                                }
                                // Recursively check operands for variables
                                Instruction::BinaryOp { lhs, rhs, .. } |
                                Instruction::Compare { lhs, rhs, .. } => {
                                    if let Instruction::Variable { name } = lhs.as_ref() {
                                        read_vars.insert(name.clone());
                                    }
                                    if let Instruction::Variable { name } = rhs.as_ref() {
                                        read_vars.insert(name.clone());
                                    }
                                }
                                Instruction::Branch { condition, .. } => {
                                    if let Instruction::Variable { name } = condition.as_ref() {
                                        read_vars.insert(name.clone());
                                    }
                                }
                                Instruction::Return { value: Some(v) } => {
                                    if let Instruction::Variable { name } = v.as_ref() {
                                        read_vars.insert(name.clone());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                let surviving_vars = written_vars.intersection(&read_vars).count();
                surviving_vars as f64
            }),
        }
    }
}
