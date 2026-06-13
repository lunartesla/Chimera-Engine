use std::collections::HashMap;
use rand::{self, Rng};
use crate::ir::module::Module;
use crate::ir::function::Function; // Correct import for Function
use crate::ir::basic_block::BasicBlock; // Correct import for BasicBlock
use crate::ir::value::Instruction; // Correct import for Instruction

pub struct IRGenerator;

impl IRGenerator {
    pub fn specialize_function(
        func: &Function,
        constants: &HashMap<String, i64>,
    ) -> Function {
        let mut specialized_func = func.clone();

        for bb in &mut specialized_func.basic_blocks {
            for inst in &mut bb.instructions {
                if let Instruction::Store { var_name, value } = inst {
                    if let Some(&const_val) = constants.get(var_name) {
                        *value = Box::new(Instruction::Constant { value: const_val });
                    }
                }
            }
        }
        specialized_func
    }

    pub fn generate_variants(
        func: &Function,
        known_values: &HashMap<String, i64>,
        count: usize,
    ) -> Vec<Function> {
        let mut variants = Vec::new();
        let mut rng = rand::thread_rng();

        for _ in 0..count {
            let mut variant = func.clone();
            for bb in &mut variant.basic_blocks {
                for inst in &mut bb.instructions {
                    Self::randomly_substitute_constants_in_instruction(inst, known_values, &mut rng);
                }
            }
            variants.push(variant);
        }
        variants
    }

    // Helper to recursively apply constant substitution
    fn randomly_substitute_constants_in_instruction(
        inst: &mut Instruction,
        known_values: &HashMap<String, i64>,
        rng: &mut impl Rng,
    ) -> bool {
        let mut changed = false;
        match inst {
            Instruction::Variable { name } => {
                if let Some(&const_val) = known_values.get(name) {
                    if rng.gen_bool(0.5) { // 50% chance to substitute
                        *inst = Instruction::Constant { value: const_val };
                        changed = true;
                    }
                }
            }
            Instruction::BinaryOp { lhs, rhs, .. } | Instruction::Compare { lhs, rhs, .. } => {
                changed |= Self::randomly_substitute_constants_in_instruction(lhs, known_values, rng);
                changed |= Self::randomly_substitute_constants_in_instruction(rhs, known_values, rng);
            }
            Instruction::Store { value, .. } => { // Only value is evaluated, var_name is a definition
                changed |= Self::randomly_substitute_constants_in_instruction(value, known_values, rng);
            }
            Instruction::Branch { condition, .. } => {
                changed |= Self::randomly_substitute_constants_in_instruction(condition, known_values, rng);
            }
            Instruction::Return { value: Some(value_box) } => {
                changed |= Self::randomly_substitute_constants_in_instruction(value_box, known_values, rng);
            }
            _ => {}
        }
        changed
    }
}