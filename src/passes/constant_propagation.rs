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

            // Count every store to each variable, anywhere in the function,
            // regardless of value. A variable is only a safe global constant
            // if it is written EXACTLY ONCE in the whole function — anything
            // written more than once (the overwhelmingly common case being a
            // loop induction variable or accumulator: initialized once,
            // reassigned every iteration) must never be globally substituted,
            // because the substitution doesn't know about control flow.
            //
            // The previous version recorded a var as constant the moment it
            // saw ANY `Store{var, Constant}` instruction anywhere — so
            // `i = 0` in the entry block got treated as "i is always 0" and
            // substituted into the loop condition `i < n` as well, turning
            // it into a control-flow-blind `0 < n` that never reflects the
            // real runtime value of i. For a loop guarded on i, that's an
            // infinite loop, not a missed optimization — caught by the
            // engine's correctness validator the first time it was wired
            // into the evolutionary scoring path.
            let mut store_count: HashMap<String, u32> = HashMap::new();
            for func in &module.functions {
                for bb in &func.basic_blocks {
                    for instr in &bb.instructions {
                        if let Instruction::Store { var_name, .. } = instr {
                            *store_count.entry(var_name.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }

            let mut var_to_const: HashMap<String, i64> = HashMap::new();
            for func in &module.functions {
                for bb in &func.basic_blocks {
                    for instr in &bb.instructions {
                        if let Instruction::Store { var_name, value } = instr {
                            if store_count.get(var_name) == Some(&1) {
                                if let Instruction::Constant { value: const_val } = &**value {
                                    var_to_const.insert(var_name.clone(), *const_val);
                                }
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