use std::collections::HashMap;
use crate::ir::module::Module;
use crate::ir::value::{Instruction, BinaryOp, CompareCondition};
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor, ParamRange};

pub struct ConstantPropagationPass {
    params: HashMap<String, ParamRange>,
}

impl ConstantPropagationPass {
    pub fn new() -> Self {
        let mut params = HashMap::new();
        params.insert(
            "max_iterations".to_string(),
            ParamRange::new("max_iterations", 10, 1, 50, 1),
        );
        Self { params }
    }


    fn replace_vars_with_constants(
        inst: &mut Instruction,
        const_map: &HashMap<String, i64>,
    ) -> bool {
        let mut changed = false;
        match inst {
            Instruction::BinaryOp { lhs, rhs, .. } => {
                changed |= Self::replace_vars_with_constants(lhs, const_map);
                changed |= Self::replace_vars_with_constants(rhs, const_map);
            }
            Instruction::Compare { lhs, rhs, .. } => {
                changed |= Self::replace_vars_with_constants(lhs, const_map);
                changed |= Self::replace_vars_with_constants(rhs, const_map);
            }
            Instruction::Store { value, .. } => {
                changed |= Self::replace_vars_with_constants(value, const_map);
            }
            Instruction::Branch { condition, .. } => {
                changed |= Self::replace_vars_with_constants(condition, const_map);
            }
            Instruction::Return { value: Some(ret_val_box) } => {
                changed |= Self::replace_vars_with_constants(ret_val_box, const_map);
            }
            Instruction::Variable { name } => {
                if let Some(val) = const_map.get(name) {
                    *inst = Instruction::Constant { value: *val };
                    changed = true;
                }
            }
            _ => {}
        }
        changed
    }
}

impl Pass for ConstantPropagationPass {
    fn id(&self) -> &'static str {
        "constant_propagation"
    }

    fn name(&self) -> &'static str {
        "Constant Propagation"
    }

    fn description(&self) -> &'static str {
        "Replaces variable references with known constant values"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Conservative
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut changed_any_iter = false;
        let max_iterations = self.get_param("max_iterations").unwrap_or(10);

        for _ in 0..max_iterations {
            let mut changed_this_iter = false;
            let mut var_to_const: HashMap<String, i64> = HashMap::new();

            // First pass: collect all stores that store constants directly
            for func in &module.functions {
                for bb in &func.basic_blocks {
                    for instr in &bb.instructions {
                        if let Instruction::Store { var_name, value } = instr {
                            if let Instruction::Constant { value: const_val } = &**value {
                                var_to_const.insert(var_name.clone(), *const_val);
                            }
                        }
                    }
                }
            }

            // Second pass: replace variable references with constants where known
            for func in &mut module.functions {
                for bb in &mut func.basic_blocks {
                    for instr in &mut bb.instructions {
                        // We need to clone the instruction to replace it if it's a Variable
                        // or to modify its sub-expressions
                        let mut current_instr_cloned = instr.clone();
                        if Self::replace_vars_with_constants(&mut current_instr_cloned, &var_to_const) {
                            *instr = current_instr_cloned;
                            changed_this_iter = true;
                        }
                    }
                }
            }

            if !changed_this_iter {
                break; // No changes in this iteration
            }
            changed_any_iter = true;
        }

        Ok(changed_any_iter)
    }

    fn get_param(&self, name: &str) -> Option<i32> {
        self.params.get(name).map(|p| p.current)
    }

    fn set_param(&mut self, name: &str, value: i32) -> bool {
        if let Some(param) = self.params.get_mut(name) {
            param.set_current(value);
            true
        } else {
            false
        }
    }

    fn descriptor(&self) -> PassDescriptor {
        PassDescriptor::new(
            self.id(),
            self.name(),
            self.description(),
            self.safety(),
            self.params.values().cloned().collect(),
        )
    }
}