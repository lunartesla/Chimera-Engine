use std::collections::HashMap;
use crate::ir::module::Module;
use crate::ir::value::{BinaryOp, Instruction};
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor, ParamRange};

pub struct ConstantFoldingPass {
    params: HashMap<String, ParamRange>,
}

impl ConstantFoldingPass {
    pub fn new() -> Self {
        let mut params = HashMap::new();
        params.insert(
            "iterations".to_string(),
            ParamRange::new("iterations", 5, 1, 50, 1),
        );
        Self { params }
    }

    fn fold_instruction_recursive(inst: &mut Instruction) -> bool {
        let mut changed = false;
        match inst {
            Instruction::BinaryOp { op, lhs, rhs } => {
                changed |= Self::fold_instruction_recursive(lhs);
                changed |= Self::fold_instruction_recursive(rhs);
                if let (Instruction::Constant { value: l_val }, Instruction::Constant { value: r_val }) = (&**lhs, &**rhs) {
                    let result = match op {
                        BinaryOp::Add => l_val + r_val,
                        BinaryOp::Sub => l_val - r_val,
                        BinaryOp::Mul => l_val * r_val,
                        BinaryOp::Div => {
                            if *r_val == 0 {
                                return changed; // Avoid division by zero
                            }
                            l_val / r_val
                        }
                    };
                    *inst = Instruction::Constant { value: result };
                    changed = true;
                }
            }
            Instruction::Store { value, .. } => {
                changed |= Self::fold_instruction_recursive(value);
            }
            Instruction::Return { value: Some(ret_val_box) } => {
                changed |= Self::fold_instruction_recursive(ret_val_box);
            }
            Instruction::Compare { lhs, rhs, .. } => {
                changed |= Self::fold_instruction_recursive(lhs);
                changed |= Self::fold_instruction_recursive(rhs);
            }
            Instruction::Branch { condition, .. } => {
                changed |= Self::fold_instruction_recursive(condition);
            }
            _ => {}
        }
        changed
    }
}

impl Pass for ConstantFoldingPass {
    fn id(&self) -> &'static str {
        "constant_folding"
    }

    fn name(&self) -> &'static str {
        "Constant Folding"
    }

    fn description(&self) -> &'static str {
        "Evaluates constant expressions at compile time"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Safe
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut changed_any_iter = false;
        let iterations = self.get_param("iterations").unwrap_or(5);

        for _ in 0..iterations {
            let mut changed_this_iter = false;
            for func in &mut module.functions {
                for bb in &mut func.basic_blocks {
                    // Iterate with an index to allow replacement of instruction
                    let mut i = 0;
                    while i < bb.instructions.len() {
                        let mut inst = bb.instructions.remove(i);
                        if Self::fold_instruction_recursive(&mut inst) {
                            changed_this_iter = true;
                        }
                        bb.instructions.insert(i, inst);
                        i += 1;
                    }
                }
            }
            if !changed_this_iter {
                break; // No changes in this iteration, we're done
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