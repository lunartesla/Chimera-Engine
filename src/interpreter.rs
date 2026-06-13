use std::collections::HashMap;
use crate::ir::{module::Module, function::Function};
use crate::ir::value::{Instruction, BinaryOp, CompareCondition};
use crate::ir::basic_block::BasicBlock;
use crate::profiler::RuntimeProfiler;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InterpreterError {
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    #[error("Invalid instruction for evaluation: {0}")]
    InvalidInstruction(String),
    #[error("Unknown block: {0}")]
    UnknownBlock(String),
    #[error("Block '{0}' doesn't end with branch/jump/return")]
    MissingTerminator(String),
    #[error("Too many iterations or recursion depth exceeded")]
    TooManyIterations,
}

#[derive(Clone)]
pub struct Interpreter;

impl Interpreter {
    pub fn new() -> Self {
        Self
    }

    pub fn execute_function(
        &self,
        function: &Function,
        // C++ takes a reference to a profiler, then passes it down.
        // Rust can pass an Option<&mut RuntimeProfiler>
        mut profiler: Option<&mut RuntimeProfiler>,
    ) -> Result<i64, InterpreterError> {
        let mut variable_values: HashMap<String, i64> = HashMap::new();
        let current_function_name = function.name.clone();

        if let Some(p) = profiler.as_mut() {
            p.start_function(&current_function_name);
        }

        let mut block_map: HashMap<String, &BasicBlock> = HashMap::new();
        for bb in &function.basic_blocks {
            block_map.insert(bb.name.clone(), bb);
        }

        if function.basic_blocks.is_empty() {
            if let Some(p) = profiler.as_mut() {
                p.end_function(&current_function_name);
            }
            return Ok(0);
        }

        let mut current_block_name = function.basic_blocks[0].name.clone();
        let mut final_result = 0i64;
        let mut instruction_count_in_func = 0; // Guard against infinite loops

        loop {
            if instruction_count_in_func > 100000 { // Max instruction execution guard
                return Err(InterpreterError::TooManyIterations);
            }

            let block = block_map
                .get(&current_block_name)
                .ok_or_else(|| InterpreterError::UnknownBlock(current_block_name.clone()))?;

            if let Some(p) = profiler.as_mut() {
                p.start_block(&current_function_name, &block.name);
            }

            let mut finished = false;
            let mut block_result = 0i64;

            for inst in &block.instructions {
                block_result = self.evaluate_instruction(inst, &mut variable_values)?;
                instruction_count_in_func += 1;

                if inst.is_terminator() {
                    if let Instruction::Return { .. } = inst {
                        finished = true;
                        final_result = block_result;
                    }
                    break; // Terminator ends block execution
                }
            }

            if let Some(p) = profiler.as_mut() {
                p.end_block(&current_function_name, &block.name);
                p.track_memory(&current_function_name, variable_values.len());
            }

            if finished {
                break;
            }

            // Handle control flow
            let last_inst = block.instructions.last().ok_or_else(|| {
                InterpreterError::MissingTerminator(block.name.clone())
            })?;

            match last_inst {
                Instruction::Branch { condition, then_label, else_label } => {
                    let cond_val = self.evaluate_instruction(&condition, &mut variable_values)?;
                    if cond_val != 0 {
                        current_block_name = then_label.clone();
                    } else {
                        current_block_name = else_label.clone();
                    }
                }
                Instruction::Jump { label } => {
                    current_block_name = label.clone();
                }
                _ => {
                    return Err(InterpreterError::MissingTerminator(block.name.clone()));
                }
            }
        }

        if let Some(p) = profiler.as_mut() {
            p.end_function(&current_function_name);
        }
        Ok(final_result)
    }

    fn evaluate_instruction(
        &self,
        inst: &Instruction,
        variable_values: &mut HashMap<String, i64>,
    ) -> Result<i64, InterpreterError> {
        match inst {
            Instruction::Constant { value } => Ok(*value),
            Instruction::Variable { name } => {
                variable_values
                    .get(name)
                    .cloned()
                    .ok_or_else(|| InterpreterError::UndefinedVariable(name.clone()))
            }
            Instruction::BinaryOp { op, lhs, rhs } => {
                let l = self.evaluate_instruction(lhs, variable_values)?;
                let r = self.evaluate_instruction(rhs, variable_values)?;
                match op {
                    BinaryOp::Add => Ok(l + r),
                    BinaryOp::Sub => Ok(l - r),
                    BinaryOp::Mul => Ok(l * r),
                    BinaryOp::Div => {
                        if r == 0 {
                            Err(InterpreterError::DivisionByZero)
                        } else {
                            Ok(l / r)
                        }
                    }
                }
            }
            Instruction::Store { var_name, value } => {
                let val = self.evaluate_instruction(value, variable_values)?;
                variable_values.insert(var_name.clone(), val);
                Ok(val)
            }
            Instruction::Compare { condition, lhs, rhs } => {
                let l = self.evaluate_instruction(lhs, variable_values)?;
                let r = self.evaluate_instruction(rhs, variable_values)?;
                let result = match condition {
                    CompareCondition::Eq => l == r,
                    CompareCondition::Ne => l != r,
                    CompareCondition::Lt => l < r,
                    CompareCondition::Gt => l > r,
                    CompareCondition::Le => l <= r,
                    CompareCondition::Ge => l >= r,
                };
                Ok(if result { 1 } else { 0 })
            }
            Instruction::Return { value: Some(val_inst) } => {
                self.evaluate_instruction(&val_inst, variable_values)
            }
            Instruction::Return { value: None } => Ok(0), // C++ returns 0 for void return
            Instruction::Branch { .. } | Instruction::Jump { .. } => {
                // These are terminators handled in execute_function control flow
                Ok(0) // Should not be evaluated directly
            }
        }
    }
}