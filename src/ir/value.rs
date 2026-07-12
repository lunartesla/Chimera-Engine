#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ValueType {
    Int,
    Float,
    Void,
    /// Phase 2: pointee type tracked mainly for documentation/debugging —
    /// our memory model is uniformly i64-per-slot (see GetElementPtr), so
    /// nothing currently uses this for size computation. Kept as a real
    /// field rather than erased so a future struct/byte-addressed memory
    /// model doesn't require yet another type-system rework.
    Pointer(Box<ValueType>),
    Array(Box<ValueType>, usize),
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
    /// Phase 2 of LLVM-format adoption: a real addressable memory model.
    /// Reserves `count` contiguous i64 slots in the interpreter's heap
    /// (zero-initialized) and binds `name` to the base address — same named
    /// HashMap<String,i64> used for plain scalars holds this too, since an
    /// address is just an i64; the *instruction type* used to access it
    /// (Variable for scalars vs GetElementPtr/LoadPtr/StorePtr for this)
    /// is what determines interpretation, not a tagged value type.
    AllocaArray {
        name: String,
        count: usize,
    },
    /// Computes `base_address + index` into the heap. Deliberately the
    /// simplified single-dimension form — real LLVM getelementptr supports
    /// multi-index struct+array addressing with per-type byte sizes; we
    /// collapse everything to uniform i64 slots and one index, which covers
    /// the common `arr[i]` pattern and nothing fancier (struct field access,
    /// multi-dimensional arrays) by design, not by oversight.
    GetElementPtr {
        base: Box<Instruction>,
        index: Box<Instruction>,
    },
    LoadPtr {
        ptr: Box<Instruction>,
    },
    StorePtr {
        ptr: Box<Instruction>,
        value: Box<Instruction>,
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
            Instruction::AllocaArray { name, count } => format!("alloca [{} x i64] -> {}", count, name),
            Instruction::GetElementPtr { base, index } => {
                format!("gep {}, {}", base.display(), index.display())
            }
            Instruction::LoadPtr { ptr } => format!("load *({})", ptr.display()),
            Instruction::StorePtr { ptr, value } => {
                format!("store {} -> *({})", value.display(), ptr.display())
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