use crate::ir::module::Module;
use crate::ir::value::{BinaryOp, Instruction};
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor};

pub struct StrengthReductionPass;

impl StrengthReductionPass {
    pub fn new() -> Self {
        Self
    }

    fn reduce_binary_op(op: BinaryOp, lhs: &Instruction, rhs: &Instruction) -> Option<Instruction> {
        match op {
            BinaryOp::Mul => {
                if let Instruction::Constant { value: l_val } = lhs {
                    if *l_val == 1 { return Some(rhs.clone()); } // 1 * x -> x
                    if *l_val == 0 { return Some(Instruction::Constant { value: 0 }); } // 0 * x -> 0
                }
                if let Instruction::Constant { value: r_val } = rhs {
                    if *r_val == 1 { return Some(lhs.clone()); } // x * 1 -> x
                    if *r_val == 0 { return Some(Instruction::Constant { value: 0 }); } // x * 0 -> 0
                }
                // NOTE: x * <power of two> -> shift is NOT implemented. This
                // IR has no shift instruction at all (see ir/value.rs), so
                // there is nothing to rewrite it to. The previous version of
                // this pass detected the power-of-two case and reported
                // changed=true anyway with zero actual mutation — a pass
                // claiming a change it didn't make. Silently doing nothing
                // is honest; claiming success while doing nothing is not.
                // Real shift support would need a new Instruction variant,
                // interpreter execution for it, and parser emission — a
                // separate, deliberate piece of work, not something to
                // smuggle in here.
            }
            BinaryOp::Add => {
                if let Instruction::Constant { value: l_val } = lhs {
                    if *l_val == 0 { return Some(rhs.clone()); } // 0 + x -> x
                }
                if let Instruction::Constant { value: r_val } = rhs {
                    if *r_val == 0 { return Some(lhs.clone()); } // x + 0 -> x
                }
            }
            _ => {}
        }
        None
    }

    /// Recursively walks and simplifies an instruction tree, bottom-up
    /// (children simplified before the parent is checked, so a nested
    /// identity that only becomes visible after an inner simplification —
    /// e.g. `(x*1) + 0` — still gets fully collapsed in one pass).
    ///
    /// The previous version only matched a BinaryOp sitting directly as a
    /// top-level entry in bb.instructions — but this IR's actual convention
    /// never produces that shape; every computed value is always nested
    /// inside something else (Store's .value, Branch's .condition, Return's
    /// .value, another BinaryOp's lhs/rhs, ...). That meant this pass could
    /// structurally never fire on any module this engine has ever built or
    /// parsed. Recursing into every instruction kind's sub-expressions is
    /// the actual fix, not a nice-to-have.
    fn simplify_tree(inst: &mut Instruction) -> bool {
        let mut changed = false;
        match inst {
            Instruction::BinaryOp { op, lhs, rhs } => {
                changed |= Self::simplify_tree(lhs);
                changed |= Self::simplify_tree(rhs);
                if let Some(reduced) = Self::reduce_binary_op(*op, lhs, rhs) {
                    *inst = reduced;
                    changed = true;
                }
            }
            Instruction::Store { value, .. } => {
                changed |= Self::simplify_tree(value);
            }
            Instruction::Compare { lhs, rhs, .. } => {
                changed |= Self::simplify_tree(lhs);
                changed |= Self::simplify_tree(rhs);
            }
            Instruction::Branch { condition, .. } => {
                changed |= Self::simplify_tree(condition);
            }
            Instruction::Return { value: Some(v) } => {
                changed |= Self::simplify_tree(v);
            }
            Instruction::Return { value: None } | Instruction::Jump { .. } => {}
            Instruction::Call { args, .. } => {
                for a in args {
                    changed |= Self::simplify_tree(a);
                }
            }
            Instruction::AllocaArray { .. } => {}
            Instruction::GetElementPtr { base, index } => {
                changed |= Self::simplify_tree(base);
                changed |= Self::simplify_tree(index);
            }
            Instruction::LoadPtr { ptr } => {
                changed |= Self::simplify_tree(ptr);
            }
            Instruction::StorePtr { ptr, value } => {
                changed |= Self::simplify_tree(ptr);
                changed |= Self::simplify_tree(value);
            }
            Instruction::Constant { .. } | Instruction::Variable { .. } => {}
        }
        changed
    }
}

impl Pass for StrengthReductionPass {
    fn id(&self) -> &'static str {
        "strength_reduction"
    }

    fn name(&self) -> &'static str {
        "Strength Reduction"
    }

    fn description(&self) -> &'static str {
        "Simplifies algebraic identities (x*1, x*0, x+0) anywhere in an instruction tree"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Risky
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut changed = false;
        for func in &mut module.functions {
            for bb in &mut func.basic_blocks {
                for inst in &mut bb.instructions {
                    if Self::simplify_tree(inst) {
                        changed = true;
                    }
                }
            }
        }
        Ok(changed)
    }

    fn get_param(&self, _name: &str) -> Option<i32> {
        None
    }

    fn set_param(&mut self, _name: &str, _value: i32) -> bool {
        false
    }

    fn descriptor(&self) -> PassDescriptor {
        PassDescriptor::new(
            self.id(),
            self.name(),
            self.description(),
            self.safety(),
            vec![],
        )
    }
}
