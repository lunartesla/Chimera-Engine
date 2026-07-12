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
        // "trace" reads like a secondary running metric (as if someone
        // added it for diagnostics), but it's never read anywhere after
        // being written — a genuinely unused variable buried among real
        // accumulation logic, real dead_code_elimination bait rather than
        // a flagged, obvious "unused_junk" var.
        Instruction::Store { var_name: "trace".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
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
            var_name: "t".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 3 }),
            }),
        },
        Instruction::Store {
            var_name: "acc".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "acc".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "t".to_string() }),
            }),
        },
        // trace mirrors the real accumulation (uses the same "t" that acc
        // legitimately needs), which is exactly why it's easy to miss on a
        // skim — it looks like it's doing real work, not like dead code.
        // Deliberately does NOT read its own prior value (no `trace + t`) —
        // dead_code's real check is "is this name ever referenced by
        // anything," and a self-accumulating var references itself, which
        // makes it genuinely (if crudely) "used" by that exact check. A
        // truly dead variable can't read itself.
        Instruction::Store {
            var_name: "trace".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::Variable { name: "t".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 2 }),
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
        // "k" reads as a real config constant (a step size, say). It's
        // single-store so constant_propagation is safe to substitute it —
        // then "salt" is genuinely foldable to a fixed 8 once k's value is
        // known, but only after BOTH propagation and folding run in the
        // right combination; neither pass alone collapses it. "salt" then
        // feeds "parity_track" every iteration, which is never read again —
        // a dead accumulator whose per-term value happens to be a hidden
        // constant, not an obvious literal.
        Instruction::Store { var_name: "k".to_string(), value: Box::new(Instruction::Constant { value: 5 }) },
        Instruction::Store {
            var_name: "salt".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "k".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 3 }),
            }),
        },
        Instruction::Store { var_name: "parity_track".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
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
            var_name: "parity_track".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "parity_track".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "salt".to_string() }),
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
        // "bump" looks like an intentional scale factor (as if this branch
        // is meant to weight even terms differently from odd ones) — but
        // *1 is a genuine algebraic identity strength_reduction really does
        // fold (x*1 -> x, verified in strength_reduction.rs), it's just
        // buried behind a plausible-looking named variable rather than a
        // bare "i * 1" sitting in the open.
        Instruction::Store {
            var_name: "bump".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Store {
            var_name: "acc".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "acc".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "bump".to_string() }),
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

/// Exercises Phase 2 (AllocaArray/GetElementPtr/LoadPtr/StorePtr) with a
/// dead accumulator buried alongside the real one — same obscuring pattern
/// as build_simple_sum's "trace", just over array elements instead of a
/// plain scalar loop, so the module still has to sequence AllocaArray fill
/// + GEP-indexed load correctly around the dead-code opportunity.
pub fn build_array_reduce(n: i32) -> Module {
    let mut mod_ = Module::new("array_reduce".to_string());
    let mut func = Function::new("array_reduce".to_string(), ValueType::Int);
    let size: i64 = 8;

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.push(Instruction::AllocaArray { name: "arr".to_string(), count: size as usize });
    for k in 0..size {
        entry.instructions.push(Instruction::StorePtr {
            ptr: Box::new(Instruction::GetElementPtr {
                base: Box::new(Instruction::Variable { name: "arr".to_string() }),
                index: Box::new(Instruction::Constant { value: k }),
            }),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::Constant { value: k + 1 }),
                rhs: Box::new(Instruction::Constant { value: n as i64 }),
            }),
        });
    }
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "sum".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        // "checksum" reads as if it exists to validate the reduction (a
        // plausible reason to duplicate the accumulation), but nothing
        // ever reads it back — real dead-code bait over array-derived
        // values rather than a plain scalar loop variable.
        Instruction::Store { var_name: "checksum".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.push(Instruction::Branch {
        condition: Box::new(Instruction::Compare {
            condition: CompareCondition::Lt,
            lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            rhs: Box::new(Instruction::Constant { value: size }),
        }),
        then_label: "body".to_string(),
        else_label: "exit".to_string(),
    });
    func.basic_blocks.push(cond);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        Instruction::Store {
            var_name: "elem".to_string(),
            value: Box::new(Instruction::LoadPtr {
                ptr: Box::new(Instruction::GetElementPtr {
                    base: Box::new(Instruction::Variable { name: "arr".to_string() }),
                    index: Box::new(Instruction::Variable { name: "i".to_string() }),
                }),
            }),
        },
        Instruction::Store {
            var_name: "sum".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "sum".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "elem".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "checksum".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "elem".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
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
    exit_bb.instructions.push(Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "sum".to_string() })) });
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

/// Exercises Phase 1 (Instruction::Call, real per-call argument binding)
/// with the same dead-accumulator pattern, this time fed by a real function
/// call rather than a plain arithmetic expression — checks that dead_code
/// correctly identifies a Store-wrapping-a-Call as removable when its
/// target is unused (see the code-review note on this in the module doc:
/// this is sound for this specific engine only because it models no
/// observable call side effects, not a general DCE-of-calls guarantee).
pub fn build_recursive_calls(n: i32) -> Module {
    let mut mod_ = Module::new("recursive_calls".to_string());

    let mut step_func = Function::new("step".to_string(), ValueType::Int);
    step_func.set_params(vec!["x".to_string()]);
    let mut step_body = BasicBlock::new("entry".to_string());
    step_body.instructions.push(Instruction::Return {
        value: Some(Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })),
    });
    step_func.basic_blocks.push(step_body);

    let mut main_func = Function::new("accumulate_calls".to_string(), ValueType::Int);
    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "total".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        // "shadow" looks like a parallel running total (as if computing
        // the same reduction two ways to cross-check it — a pattern that
        // shows up for real in defensively-written code), but it's never
        // read back; the call feeding it (step(i)) is genuinely dead too.
        Instruction::Store { var_name: "shadow".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    main_func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.push(Instruction::Branch {
        condition: Box::new(Instruction::Compare {
            condition: CompareCondition::Lt,
            lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
        }),
        then_label: "body".to_string(),
        else_label: "exit".to_string(),
    });
    main_func.basic_blocks.push(cond);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        Instruction::Store {
            var_name: "c".to_string(),
            value: Box::new(Instruction::Call {
                function_name: "step".to_string(),
                args: vec![Box::new(Instruction::Variable { name: "i".to_string() })],
            }),
        },
        Instruction::Store {
            var_name: "total".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "total".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "c".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "shadow".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "c".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
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
    main_func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.push(Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "total".to_string() })) });
    main_func.basic_blocks.push(exit_bb);

    mod_.functions.push(main_func);
    mod_.functions.push(step_func);
    mod_
}

#[derive(Debug, Deserialize)]
struct UroborosEntry {
    module_name: String,
    function_name: String,
    loop_count: Option<i32>,
}

/// Computes digital root via iterative sum-of-digits with a dead
/// accumulator tracking total iterations. Tests iterative constant collapse:
/// each loop iteration computes `sum + digit`, but the iteration counter
/// "iterations" is written yet never read - genuine DCE bait.
pub fn build_digital_root(n: i32) -> Module {
    let mut mod_ = Module::new("digital_root".to_string());
    let mut func = Function::new("digital_root".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "x".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Store { var_name: "sum".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        // "iterations" looks like debugging/profiling data, but nothing reads it
        Instruction::Store { var_name: "iterations".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Jump { label: "loop".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut loop_bb = BasicBlock::new("loop".to_string());
    loop_bb.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Gt,
                lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 0 }),
            }),
            then_label: "body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(loop_bb);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        // Extract last digit: x % 10
        Instruction::Store { var_name: "digit".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Sub,
            lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
            rhs: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::BinaryOp {
                    op: BinaryOp::Div,
                    lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
                    rhs: Box::new(Instruction::Constant { value: 10 }),
                }),
                rhs: Box::new(Instruction::Constant { value: 10 }),
            }),
        })},
        // sum += digit
        Instruction::Store { var_name: "sum".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "sum".to_string() }),
            rhs: Box::new(Instruction::Variable { name: "digit".to_string() }),
        })},
        // x = x / 10  (integer division for next iteration)
        Instruction::Store { var_name: "x".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Div,
            lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 10 }),
        })},
        // iterations += 1 (dead)
        Instruction::Store { var_name: "iterations".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "iterations".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Jump { label: "loop".to_string() },
    ]);
    func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "sum".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

/// Two-phase accumulation with dead intermediate: computes a*b + c*d where
/// intermediates "ab" and "cd" are written but "intermed_sum" (their sum) is
/// never read. Tests CSE on separate subexpressions and DCE on the unused sum.
pub fn build_two_phase_product(n: i32) -> Module {
    let mut mod_ = Module::new("two_phase_product".to_string());
    let mut func = Function::new("two_phase".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "a".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Store { var_name: "b".to_string(), value: Box::new(Instruction::Constant { value: 3 }) },
        Instruction::Store { var_name: "c".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Store { var_name: "d".to_string(), value: Box::new(Instruction::Constant { value: 7 }) },
        Instruction::Store { var_name: "ab".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "cd".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        // "intermed_sum" looks real but is dead
        Instruction::Store { var_name: "intermed_sum".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Jump { label: "compute".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut compute = BasicBlock::new("compute".to_string());
    compute.instructions.extend(vec![
        // ab = a * b && cd = c * d - identical structure, could share CSE
        Instruction::Store { var_name: "ab".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Mul,
            lhs: Box::new(Instruction::Variable { name: "a".to_string() }),
            rhs: Box::new(Instruction::Variable { name: "b".to_string() }),
        })},
        Instruction::Store { var_name: "cd".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Mul,
            lhs: Box::new(Instruction::Variable { name: "c".to_string() }),
            rhs: Box::new(Instruction::Variable { name: "d".to_string() }),
        })},
        // intermediate sum (dead - never read)
        Instruction::Store { var_name: "intermed_sum".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "ab".to_string() }),
            rhs: Box::new(Instruction::Variable { name: "cd".to_string() }),
        })},
        Instruction::Return { value: Some(Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "ab".to_string() }),
            rhs: Box::new(Instruction::Variable { name: "cd".to_string() }),
        })) },
    ]);
    func.basic_blocks.push(compute);

    mod_.functions.push(func);
    mod_
}

/// Triangle number via nested loop with dead tracking variable.
/// Outer loop: i=0..n, inner: j=0..i, accumulating sum += j
/// Tests loop unrolling synergy with strength reduction on j+0 identities.
pub fn build_triangle_numbers(n: i32) -> Module {
    let mut mod_ = Module::new("triangle".to_string());
    let mut func = Function::new("triangle".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "sum".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        // "inner_iters" counts inner loop iterations but is never read
        Instruction::Store { var_name: "inner_iters".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Jump { label: "outer_cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut outer_cond = BasicBlock::new("outer_cond".to_string());
    outer_cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
            then_label: "outer_body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(outer_cond);

    let mut outer_body = BasicBlock::new("outer_body".to_string());
    outer_body.instructions.extend(vec![
        Instruction::Store { var_name: "j".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Jump { label: "inner_cond".to_string() },
    ]);
    func.basic_blocks.push(outer_body);

    let mut inner_cond = BasicBlock::new("inner_cond".to_string());
    inner_cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "j".to_string() }),
                rhs: Box::new(Instruction::BinaryOp {
                    op: BinaryOp::Add,
                    lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                    rhs: Box::new(Instruction::Constant { value: 1 }),
                }),
            }),
            then_label: "inner_body".to_string(),
            else_label: "outer_next".to_string(),
        },
    ]);
    func.basic_blocks.push(inner_cond);

    let mut inner_body = BasicBlock::new("inner_body".to_string());
    inner_body.instructions.extend(vec![
        Instruction::Store { var_name: "sum".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "sum".to_string() }),
            rhs: Box::new(Instruction::Variable { name: "j".to_string() }),
        })},
        // inner_iters looks real but dead
        Instruction::Store { var_name: "inner_iters".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "inner_iters".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Store { var_name: "j".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "j".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Jump { label: "inner_cond".to_string() },
    ]);
    func.basic_blocks.push(inner_body);

    let mut outer_next = BasicBlock::new("outer_next".to_string());
    outer_next.instructions.extend(vec![
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Jump { label: "outer_cond".to_string() },
    ]);
    func.basic_blocks.push(outer_next);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "sum".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

/// Computes factorial with memoization table (array). Tests Phase 2 memory
/// model with a twist: memo entries are computed but the table itself is
/// never fully read: idx 0..n gets written, but only idx n is read.
pub fn build_factorial_memo(n: i32) -> Module {
    let mut mod_ = Module::new("factorial_memo".to_string());
    let mut func = Function::new("factorial".to_string(), ValueType::Int);
    let mut step_func = Function::new("step".to_string(), ValueType::Int);
    step_func.set_params(vec!["x".to_string()]);

    // step(x) = x + 1 (simple function for testing calls)
    let mut step_body = BasicBlock::new("entry".to_string());
    step_body.instructions.push(Instruction::Return {
        value: Some(Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })),
    });
    step_func.basic_blocks.push(step_body);

    let mut entry = BasicBlock::new("entry".to_string());
    // memo array of size n+1
    entry.instructions.push(Instruction::AllocaArray { name: "memo".to_string(), count: (n as usize).max(5) + 1 });
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "memo_base".to_string(), value: Box::new(Instruction::Variable { name: "memo".to_string() }) },
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 1 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n.max(1) as i64 }) },
        // memo[0] = 1 is dead (0 index never read back)
        Instruction::StorePtr {
            ptr: Box::new(Instruction::GetElementPtr {
                base: Box::new(Instruction::Variable { name: "memo".to_string() }),
                index: Box::new(Instruction::Constant { value: 0 }),
            }),
            value: Box::new(Instruction::Constant { value: 1 }),
        },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::BinaryOp {
                    op: BinaryOp::Add,
                    lhs: Box::new(Instruction::Variable { name: "n".to_string() }),
                    rhs: Box::new(Instruction::Constant { value: 1 }),
                }),
            }),
            then_label: "body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(cond);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        // fact = memo[i-1] * i
        Instruction::Store { var_name: "idx".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Sub,
            lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Store { var_name: "fact".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Mul,
            lhs: Box::new(Instruction::LoadPtr {
                ptr: Box::new(Instruction::BinaryOp {
                    op: BinaryOp::Add,
                    lhs: Box::new(Instruction::Variable { name: "memo".to_string() }),
                    rhs: Box::new(Instruction::Variable { name: "idx".to_string() }),
                }),
            }),
            rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
        })},
        // memo[i] = fact
        Instruction::StorePtr {
            ptr: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "memo".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
            value: Box::new(Instruction::Variable { name: "fact".to_string() }),
        },
        // "checksum" is dead - same value as fact but never used
        Instruction::Store { var_name: "checksum".to_string(), value: Box::new(Instruction::Variable { name: "fact".to_string() }) },
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::LoadPtr {
            ptr: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "memo".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
        })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_.functions.push(step_func);
    mod_
}

/// Computes gcd via Euclidean algorithm with dead iteration counter and
/// redundant comparison. Tests strength reduction on division/modulo identities.
pub fn build_gcd_euclidean(n: i32) -> Module {
    let mut mod_ = Module::new("gcd_euclidean".to_string());
    let mut func = Function::new("gcd".to_string(), ValueType::Int);
    let m_val = (n as i64).max(10); // ensure m > 0

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "a".to_string(), value: Box::new(Instruction::Constant { value: m_val * 2 }) },
        Instruction::Store { var_name: "b".to_string(), value: Box::new(Instruction::Constant { value: m_val }) },
        // "iters" is dead but looks like debugging info
        Instruction::Store { var_name: "iters".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Jump { label: "loop".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut loop_bb = BasicBlock::new("loop".to_string());
    loop_bb.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Ne,
                lhs: Box::new(Instruction::Variable { name: "b".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 0 }),
            }),
            then_label: "body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(loop_bb);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        // temp = a % b, then a = b, b = temp
        Instruction::Store { var_name: "temp".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Sub,
            lhs: Box::new(Instruction::Variable { name: "a".to_string() }),
            rhs: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Instruction::BinaryOp {
                    op: BinaryOp::Div,
                    lhs: Box::new(Instruction::Variable { name: "a".to_string() }),
                    rhs: Box::new(Instruction::Variable { name: "b".to_string() }),
                }),
                rhs: Box::new(Instruction::Variable { name: "b".to_string() }),
            }),
        })},
        // Redundant comparison: (a != 0) uses temp instead of b - tests dead code where temp IS used
        // but the comparison result isn't
        Instruction::Store { var_name: "a".to_string(), value: Box::new(Instruction::Variable { name: "b".to_string() }) },
        Instruction::Store { var_name: "b".to_string(), value: Box::new(Instruction::Variable { name: "temp".to_string() }) },
        // iters += 1 but never read
        Instruction::Store { var_name: "iters".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "iters".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Jump { label: "loop".to_string() },
    ]);
    func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "a".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

/// Multiple function calls with shared subexpression patterns. Tests CSE
/// across function boundaries and dead code in call chains.
pub fn build_call_chain(n: i32) -> Module {
    let mut mod_ = Module::new("call_chain".to_string());

    // Helper: add(x) = x + 1
    let mut add_func = Function::new("add".to_string(), ValueType::Int);
    add_func.set_params(vec!["x".to_string()]);
    let mut add_body = BasicBlock::new("entry".to_string());
    add_body.instructions.push(Instruction::Return {
        value: Some(Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })),
    });
    add_func.basic_blocks.push(add_body);

    // Helper: mul(x) = x * 2
    let mut mul_func = Function::new("mul".to_string(), ValueType::Int);
    mul_func.set_params(vec!["x".to_string()]);
    let mut mul_body = BasicBlock::new("entry".to_string());
    mul_body.instructions.push(Instruction::Return {
        value: Some(Box::new(Instruction::BinaryOp {
            op: BinaryOp::Mul,
            lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 2 }),
        })),
    });
    mul_func.basic_blocks.push(mul_body);

    // Main: chained calls with dead accumulation
    let mut main_func = Function::new("main".to_string(), ValueType::Int);
    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "v".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        // result = add(mul(v)) - tests call result substitution
        Instruction::Store { var_name: "t1".to_string(), value: Box::new(Instruction::Call {
            function_name: "mul".to_string(),
            args: vec![Box::new(Instruction::Variable { name: "v".to_string() })],
        })},
    ]);
    main_func.basic_blocks.push(entry);

    let mut call2 = BasicBlock::new("call2".to_string());
    call2.instructions.extend(vec![
        Instruction::Store { var_name: "result".to_string(), value: Box::new(Instruction::Call {
            function_name: "add".to_string(),
            args: vec![Box::new(Instruction::Variable { name: "t1".to_string() })],
        })},
        // "shadow_result" is dead but looks like a parallel computation
        Instruction::Store { var_name: "shadow_result".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "t1".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "result".to_string() })) },
    ]);
    main_func.basic_blocks.push(call2);

    mod_.functions.push(main_func);
    mod_.functions.push(add_func);
    mod_.functions.push(mul_func);
    mod_
}

/// Tests complex nested expressions with multiple dead paths.
/// Computes (a*b + c*d) - (e*f + g*h) where some intermediate products
/// are computed but their sums are never consolidated.
pub fn build_complex_expr(n: i32) -> Module {
    let mut mod_ = Module::new("complex_expr".to_string());
    let mut func = Function::new("complex".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "a".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Store { var_name: "b".to_string(), value: Box::new(Instruction::Constant { value: 2 }) },
        Instruction::Store { var_name: "c".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Store { var_name: "d".to_string(), value: Box::new(Instruction::Constant { value: 3 }) },
        Instruction::Store { var_name: "e".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Store { var_name: "f".to_string(), value: Box::new(Instruction::Constant { value: 1 }) },
        Instruction::Store { var_name: "g".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Store { var_name: "h".to_string(), value: Box::new(Instruction::Constant { value: 4 }) },
        Instruction::Store { var_name: "ab".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Mul, lhs: Box::new(Instruction::Variable { name: "a".to_string() }), rhs: Box::new(Instruction::Variable { name: "b".to_string() }),
        })},
        Instruction::Store { var_name: "cd".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Mul, lhs: Box::new(Instruction::Variable { name: "c".to_string() }), rhs: Box::new(Instruction::Variable { name: "d".to_string() }),
        })},
        Instruction::Store { var_name: "ef".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Mul, lhs: Box::new(Instruction::Variable { name: "e".to_string() }), rhs: Box::new(Instruction::Variable { name: "f".to_string() }),
        })},
        Instruction::Store { var_name: "gh".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Mul, lhs: Box::new(Instruction::Variable { name: "g".to_string() }), rhs: Box::new(Instruction::Variable { name: "h".to_string() }),
        })},
        // "left_sum" is dead - ab + cd computed but never used
        Instruction::Store { var_name: "left_sum".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add, lhs: Box::new(Instruction::Variable { name: "ab".to_string() }), rhs: Box::new(Instruction::Variable { name: "cd".to_string() }),
        })},
        // "right_sum" is dead
        Instruction::Store { var_name: "right_sum".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add, lhs: Box::new(Instruction::Variable { name: "ef".to_string() }), rhs: Box::new(Instruction::Variable { name: "gh".to_string() }),
        })},
        Instruction::Return { value: Some(Box::new(Instruction::BinaryOp {
            op: BinaryOp::Sub,
            lhs: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "ab".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "cd".to_string() }),
            }),
            rhs: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "ef".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "gh".to_string() }),
            }),
        })) },
    ]);
    func.basic_blocks.push(entry);

    mod_.functions.push(func);
    mod_
}

/// Array-based prefix sum computation. Tests memory operations with
/// dead tracking of read/write counts.
pub fn build_prefix_sum(n: i32) -> Module {
    let mut mod_ = Module::new("prefix_sum".to_string());
    let mut func = Function::new("prefix".to_string(), ValueType::Int);
    let size = (n as usize).max(5);

    let mut entry = BasicBlock::new("entry".to_string());
    // arr[0..size] = {1,2,3,...size}
    entry.instructions.push(Instruction::AllocaArray { name: "arr".to_string(), count: size });
    for k in 0..(size as i64) {
        entry.instructions.push(Instruction::StorePtr {
            ptr: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "arr".to_string() }),
                rhs: Box::new(Instruction::Constant { value: k }),
            }),
            value: Box::new(Instruction::Constant { value: k + 1 }),
        });
    }
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "writes".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "reads".to_string(), value: Box::new(Instruction::Constant { value: 0 }) }, // dead
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 1 }) },
        Instruction::Store { var_name: "sum".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Jump { label: "loop".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut loop_bb = BasicBlock::new("loop".to_string());
    loop_bb.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: size as i64 }),
            }),
            then_label: "body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(loop_bb);

    let mut body = BasicBlock::new("body".to_string());
    body.instructions.extend(vec![
        Instruction::Store { var_name: "elem".to_string(), value: Box::new(Instruction::LoadPtr {
            ptr: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "arr".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
        })},
        Instruction::Store { var_name: "sum".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "sum".to_string() }),
            rhs: Box::new(Instruction::Variable { name: "elem".to_string() }),
        })},
        // writes += 1 (dead, since we're writing but this particular counter isn't necessary)
        Instruction::Store { var_name: "writes".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "writes".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            rhs: Box::new(Instruction::Constant { value: 1 }),
        })},
        Instruction::Jump { label: "loop".to_string() },
    ]);
    func.basic_blocks.push(body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "sum".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    mod_.functions.push(func);
    mod_
}

/// Multiple exit paths with different dead computations per branch.
/// Tests dead code elimination on branch-specific unused stores.
pub fn build_multi_exit(n: i32) -> Module {
    let mut mod_ = Module::new("multi_exit".to_string());
    let mut func = Function::new("multi".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "x".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Store { var_name: "track_a".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "track_b".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Jump { label: "cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: CompareCondition::Gt,
                lhs: Box::new(Instruction::Variable { name: "x".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 0 }),
            }),
            then_label: "path_a".to_string(),
            else_label: "path_b".to_string(),
        },
    ]);
    func.basic_blocks.push(cond);

    let mut path_a = BasicBlock::new("path_a".to_string());
    path_a.instructions.extend(vec![
        // track_a gets updated but never read
        Instruction::Store { var_name: "track_a".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Add, lhs: Box::new(Instruction::Variable { name: "x".to_string() }), rhs: Box::new(Instruction::Constant { value: 10 }),
        })},
        // "a_temp" is dead
        Instruction::Store { var_name: "a_temp".to_string(), value: Box::new(Instruction::Constant { value: 999 }) },
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "x".to_string() })) },
    ]);
    func.basic_blocks.push(path_a);

    let mut path_b = BasicBlock::new("path_b".to_string());
    path_b.instructions.extend(vec![
        // track_b gets updated but never read
        Instruction::Store { var_name: "track_b".to_string(), value: Box::new(Instruction::BinaryOp {
            op: BinaryOp::Sub, lhs: Box::new(Instruction::Variable { name: "x".to_string() }), rhs: Box::new(Instruction::Constant { value: 5 }),
        })},
        // "b_temp" is dead
        Instruction::Store { var_name: "b_temp".to_string(), value: Box::new(Instruction::Constant { value: 999 }) },
        Instruction::Return { value: Some(Box::new(Instruction::Constant { value: (-n) as i64 })) },
    ]);
    func.basic_blocks.push(path_b);

    mod_.functions.push(func);
    mod_
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