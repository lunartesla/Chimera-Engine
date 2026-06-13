use crate::ir::module::Module;
use crate::Function; // Re-export from crate root
use crate::BasicBlock; // Re-export from crate root
use crate::ir::value::{Instruction, BinaryOp, CompareCondition, ValueType};
use std::collections::HashMap;
use std::fs;
use serde::{Serialize, Deserialize};
use serde_json;
use log::{warn, info}; // Added info for load_uroboros_library
use std::path::Path; // Add Path for load_uroboros_library

pub fn build_sum_example(n: i32) -> Module {
    let mut module = Module::new("demo".to_string());
    let mut func = Function::new("compute_sum".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store {
            var_name: "i".to_string(),
            value: Box::new(Instruction::Constant { value: 0 }),
        },
        Instruction::Store {
            var_name: "sum".to_string(),
            value: Box::new(Instruction::Constant { value: 0 }),
        },
        Instruction::Store {
            var_name: "n".to_string(),
            value: Box::new(Instruction::Constant { value: n as i64 }),
        },
        Instruction::Jump {
            label: "loop_cond".to_string(),
        },
    ]);

    let mut loop_cond = BasicBlock::new("loop_cond".to_string());
    loop_cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
            then_label: "loop_body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);

    let mut loop_body = BasicBlock::new("loop_body".to_string());
    loop_body.instructions.extend(vec![
        Instruction::Store {
            var_name: "sum".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "sum".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "i".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Jump {
            label: "loop_cond".to_string(),
        },
    ]);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return {
            value: Some(Box::new(Instruction::Variable { name: "sum".to_string() })),
        },
    ]);

    func.basic_blocks.extend(vec![entry, loop_cond, loop_body, exit_bb]);
    module.functions.push(func);
    module
}

pub fn build_simple_sum(n: i32) -> Module {
    let mut mod_ = Module::new("simple_sum".to_string());
    let mut func = Function::new("sum".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "acc".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
            then_label: "body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(cond);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        Instruction::Store {
            var_name: "acc".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "acc".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "i".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "acc".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

pub fn build_fib_loop(n: i32) -> Module {
    let mut mod_ = Module::new("fib_loop".to_string());
    let mut func = Function::new("fib".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "a".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "b".to_string(), value: Box::new(Instruction::Constant { value: 1 }) },
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
            then_label: "body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(cond);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        Instruction::Store {
            var_name: "tmp".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "a".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "b".to_string() }),
            }),
        },
        Instruction::Store { var_name: "a".to_string(), value: Box::new(Instruction::Variable { name: "b".to_string() }) },
        Instruction::Store { var_name: "b".to_string(), value: Box::new(Instruction::Variable { name: "tmp".to_string() }) },
        Instruction::Store {
            var_name: "i".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "b".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

pub fn build_nested_loop(n: i32) -> Module {
    let mut mod_ = Module::new("nested_loop".to_string());
    let mut func = Function::new("nested".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "acc".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
            then_label: "body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(cond);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        Instruction::Store {
            var_name: "sq".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "acc".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "acc".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "sq".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "i".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "acc".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

pub fn build_branch_heavy(n: i32) -> Module {
    let mut mod_ = Module::new("branch_heavy".to_string());
    let mut func = Function::new("branches".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "acc".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
            then_label: "check".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(cond);

    let mut check = BasicBlock::new("check".to_string());
    check.instructions.extend(vec![
        Instruction::Store {
            var_name: "h".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Div,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 2 }),
            }),
        },
        Instruction::Store {
            var_name: "e2".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::Variable { name: "h".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 2 }),
            }),
        },
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Eq,
                lhs: Box::new(Instruction::Variable { name: "e2".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
            then_label: "even".to_string(),
            else_label: "odd".to_string(),
        },
    ]);
    func.basic_blocks.push(check);

    let mut even = BasicBlock::new("even".to_string());
    even.instructions.extend(vec![
        Instruction::Store {
            var_name: "acc".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "acc".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
        },
        Instruction::Jump { label: "lend".to_string() },
    ]);
    func.basic_blocks.push(even);

    let mut odd = BasicBlock::new("odd".to_string());
    odd.instructions.extend(vec![
        Instruction::Store {
            var_name: "acc".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Sub,
                lhs: Box::new(Instruction::Variable { name: "acc".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
        },
        Instruction::Jump { label: "lend".to_string() },
    ]);
    func.basic_blocks.push(odd);

    let mut lend = BasicBlock::new("lend".to_string());
    lend.instructions.extend(vec![
        Instruction::Store {
            var_name: "i".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(lend);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "acc".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

pub fn build_entropy_loop(n: i32) -> Module {
    let mut mod_ = Module::new("entropy_loop".to_string());
    let mut func = Function::new("entropy".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "total".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "freq".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
            then_label: "body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(cond);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        Instruction::Store {
            var_name: "sc".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 3 }),
            }),
        },
        Instruction::Store {
            var_name: "freq".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "freq".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "sc".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "d".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Store {
            var_name: "term".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Div,
                lhs: Box::new(Instruction::Variable { name: "freq".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "d".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "total".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "total".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "term".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "i".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "total".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

#[derive(Debug, Deserialize)]
struct UroborosEntry {
    module_name: String,
    function_name: String,
    loop_count: Option<i32>,
}

pub fn load_uroboros_library() -> Vec<Module> {
    let mut modules = Vec::new();
    let paths = [
        "uroboros_ir_library.json",
        "scripts/uroboros_ir_library.json",
    ];

    for path_str in &paths {
        let path = Path::new(path_str);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                match serde_json::from_str::<Vec<UroborosEntry>>(&content) {
                    Ok(entries) => {
                        for entry in entries {
                            let mn = entry.module_name;
                            let fn_ = entry.function_name;
                            let loop_n = entry.loop_count.unwrap_or(4) * 8; // C++ default
                            let loop_n = loop_n.max(4);

                            let mut mod_ = Module::new(mn.clone());
                            let mut func = Function::new(fn_.clone(), ValueType::Int);

                            let mut entry_bb = BasicBlock::new("entry".to_string());
                            entry_bb.instructions.extend(vec![
                                Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
                                Instruction::Store { var_name: "acc".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
                                Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: loop_n as i64 }) },
                                Instruction::Jump { label: "cond".to_string() },
                            ]);
                            func.basic_blocks.push(entry_bb);

                            let mut cond_bb = BasicBlock::new("cond".to_string());
                            cond_bb.instructions.extend(vec![
                                Instruction::Branch {
                                    condition: Box::new(Instruction::Compare {
                                        condition: CompareCondition::Lt,
                                        lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                                        rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
                                    }),
                                    then_label: "body".to_string(),
                                    else_label: "exit".to_string(),
                                },
                            ]);
                            func.basic_blocks.push(cond_bb);

                            let mut body_bb = BasicBlock::new("body".to_string());
                            body_bb.instructions.extend(vec![
                                Instruction::Store {
                                    var_name: "acc".to_string(),
                                    value: Box::new(Instruction::BinaryOp {
                                        op: BinaryOp::Add,
                                        lhs: Box::new(Instruction::Variable { name: "acc".to_string() }),
                                        rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                                    }),
                                },
                                Instruction::Store {
                                    var_name: "i".to_string(),
                                    value: Box::new(Instruction::BinaryOp {
                                        op: BinaryOp::Add,
                                        lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                                        rhs: Box::new(Instruction::Constant { value: 1 }),
                                    }),
                                },
                                Instruction::Jump { label: "cond".to_string() },
                            ]);
                            func.basic_blocks.push(body_bb);

                            let mut exit_bb = BasicBlock::new("exit".to_string());
                            exit_bb.instructions.extend(vec![
                                Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "acc".to_string() })) },
                            ]);
                            func.basic_blocks.push(exit_bb);

                            mod_.functions.push(func);
                            modules.push(mod_);
                        }
                        info!("[daemon] Loaded {} UROBOROS modules from {}", modules.len(), path.display());
                        return modules; // C++ breaks after first successful load
                    }
                    Err(e) => warn!("[daemon] Failed to parse uroboros_ir_library.json from {}: {}", path.display(), e),
                }
            }
        }
    }
    modules
}