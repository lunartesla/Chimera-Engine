#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ValueType {
    Int,
    Float,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl BinaryOp {
    pub fn display(&self) -> &str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompareCondition {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Instruction {
    Constant { value: i64 },
    Variable { name: String },
    BinaryOp {
        op: BinaryOp,
        lhs: Box<Instruction>,
        rhs: Box<Instruction>,
    },
    Compare {
        condition: CompareCondition,
        lhs: Box<Instruction>,
        rhs: Box<Instruction>,
    },
    Store {
        var_name: String,
        value: Box<Instruction>,
    },
    Branch {
        condition: Box<Instruction>,
        then_label: String,
        else_label: String,
    },
    Jump { label: String },
    Return { value: Option<Box<Instruction>> },
    /// Phase 1 of LLVM-format adoption: intra-module function calls,
    /// scalar args/return only. No pointers — args are evaluated as
    /// ordinary scalar Instruction trees, matched positionally against the
    /// callee's parameter list (see llvm_frontend.rs's existing param-baking
    /// convention, just resolved per-call now instead of baked once at
    /// parse time). Deliberately does NOT support calls to functions outside
    /// this module (libc, allocator, etc.) — those still get rejected by the
    /// parser rather than silently treated as no-ops.
    Call {
        function_name: String,
        args: Vec<Box<Instruction>>,
    },
}

impl Instruction {
    pub fn display(&self) -> String {
        match self {
            Instruction::Constant { value } => format!("const {}", value),
            Instruction::Variable { name } => format!("var {}", name),
            Instruction::BinaryOp { op, lhs, rhs } => {
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                };
                format!("{} {} {}", lhs.display(), op_str, rhs.display())
            }
            Instruction::Compare { condition, lhs, rhs } => {
                let cond_str = match condition {
                    CompareCondition::Eq => "==",
                    CompareCondition::Ne => "!=",
                    CompareCondition::Lt => "<",
                    CompareCondition::Le => "<=",
                    CompareCondition::Gt => ">",
                    CompareCondition::Ge => ">=",
                };
                format!("{} {} {}", lhs.display(), cond_str, rhs.display())
            }
            Instruction::Store { var_name, value } => {
                format!("store {} -> {}", value.display(), var_name)
            }
            Instruction::Branch { condition, then_label, else_label } => {
                format!("br {} -> {}, {}", condition.display(), then_label, else_label)
            }
            Instruction::Jump { label } => format!("jmp {}", label),
            Instruction::Return { value } => {
                if let Some(v) = value {
                    format!("ret {}", v.display())
                } else {
                    "ret void".to_string()
                }
            }
            Instruction::Call { function_name, args } => {
                let args_str = args.iter().map(|a| a.display()).collect::<Vec<_>>().join(", ");
                format!("call {}({})", function_name, args_str)
            }
        }
    }

    pub fn is_terminator(&self) -> bool {
        matches!(self,
            Instruction::Branch { .. } |
            Instruction::Jump { .. } |
            Instruction::Return { .. }
        )
    }
}