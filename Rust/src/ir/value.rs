use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Int,
    Float,
    Void,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Constant(i32),
    Variable(String),
    BinaryOp {
        op: BinaryOp,
        lhs: Rc<RefCell<Instruction>>,
        rhs: Rc<RefCell<Instruction>>,
    },
    Compare {
        cond: CompareCondition,
        lhs: Rc<RefCell<Instruction>>,
        rhs: Rc<RefCell<Instruction>>,
    },
    Store {
        var_name: String,
        value: Rc<RefCell<Instruction>>,
    },
    Branch {
        condition: Rc<RefCell<Instruction>>,
        true_target: String,
        false_target: String,
    },
    Jump {
        target: String,
    },
    Return {
        value: Option<Rc<RefCell<Instruction>>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareCondition {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl Instruction {
    pub fn clone(&self) -> Self {
        match self {
            Instruction::Constant(v) => Instruction::Constant(*v),
            Instruction::Variable(name) => Instruction::Variable(name.clone()),
            Instruction::BinaryOp { op, lhs, rhs } => Instruction::BinaryOp {
                op: *op,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            Instruction::Compare { cond, lhs, rhs } => Instruction::Compare {
                cond: *cond,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            Instruction::Store { var_name, value } => Instruction::Store {
                var_name: var_name.clone(),
                value: value.clone(),
            },
            Instruction::Branch { condition, true_target, false_target } => Instruction::Branch {
                condition: condition.clone(),
                true_target: true_target.clone(),
                false_target: false_target.clone(),
            },
            Instruction::Jump { target } => Instruction::Jump {
                target: target.clone(),
            },
            Instruction::Return { value } => Instruction::Return {
                value: value.as_ref().map(|v| v.clone()),
            },
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Instruction::Constant(v) => v.to_string(),
            Instruction::Variable(name) => format!("%{}", name),
            Instruction::BinaryOp { op, lhs, rhs } => {
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                };
                format!(
                    "({} {} {})",
                    lhs.borrow().to_string(),
                    op_str,
                    rhs.borrow().to_string()
                )
            }
            Instruction::Compare { cond, lhs, rhs } => {
                let op_str = match cond {
                    CompareCondition::Eq => "==",
                    CompareCondition::Ne => "!=",
                    CompareCondition::Lt => "<",
                    CompareCondition::Gt => ">",
                    CompareCondition::Le => "<=",
                    CompareCondition::Ge => ">=",
                };
                format!(
                    "({} {} {})",
                    lhs.borrow().to_string(),
                    op_str,
                    rhs.borrow().to_string()
                )
            }
            Instruction::Store { var_name, value } => {
                format!("store {} = {}", var_name, value.borrow().to_string())
            }
            Instruction::Branch { condition, true_target, false_target } => {
                format!(
                    "br {}, {}, {}",
                    condition.borrow().to_string(),
                    true_target,
                    false_target
                )
            }
            Instruction::Jump { target } => format!("jmp {}", target),
            Instruction::Return { value } => match value {
                Some(v) => format!("return {}", v.borrow().to_string()),
                None => "return void".to_string(),
            },
        }
    }

    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Instruction::Branch { .. } | Instruction::Jump { .. } | Instruction::Return { .. }
        )
    }
}
