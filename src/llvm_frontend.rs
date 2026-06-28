// LLVM IR frontend: ingest real .c/.cpp/.rs/.ll programs into the engine's IR.
//
// Scope (deliberately limited to what the engine's Instruction enum can express):
//   - scalar int/float arithmetic, comparisons, branches, loops, return
//   - NO function calls, pointers/GEP, arrays, structs, phi nodes, switch, vectors
// This matches clang/rustc output at -O0 (no mem2reg => every local is an
// alloca+load+store, no phi nodes at all) which is exactly what makes this
// tractable without a real LLVM dependency (no llvm-sys/inkwell needed — just
// a constrained textual .ll parser).
//
// Rust note: plain `rustc --emit=llvm-ir` inserts overflow-check intrinsics
// (`call ... @llvm.sadd.with.overflow...`, `extractvalue`, panic branches) that
// fall outside this subset. We compile with `-C overflow-checks=off -C panic=abort`
// to get the same clean alloca/load/store/icmp/br shape clang produces.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;

use crate::ir::module::Module;
use crate::ir::function::Function;
use crate::ir::basic_block::BasicBlock;
use crate::ir::value::{Instruction, BinaryOp, CompareCondition, ValueType};

/// Compile a source file to a temporary .ll file (or pass through if it's
/// already .ll). Returns the path to the textual LLVM IR file.
pub fn compile_source_to_ll(source_path: &Path) -> Result<PathBuf> {
    let ext = source_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "ll" {
        return Ok(source_path.to_path_buf());
    }

    let out_path = source_path.with_extension("generated.ll");

    match ext.as_str() {
        "c" | "cpp" | "cc" | "cxx" => {
            let status = Command::new("clang")
                .args([
                    "-S", "-emit-llvm", "-O0",
                    "-Xclang", "-disable-O0-optnone",
                ])
                .arg(source_path)
                .arg("-o")
                .arg(&out_path)
                .status()
                .context("Failed to invoke clang. Is LLVM/clang installed and on PATH?")?;
            if !status.success() {
                bail!("clang exited with status {} while compiling {}", status, source_path.display());
            }
        }
        "rs" => {
            let status = Command::new("rustc")
                .args([
                    "--emit=llvm-ir",
                    "-C", "opt-level=0",
                    "-C", "overflow-checks=off",
                    "-C", "panic=abort",
                    "--crate-type=lib",
                ])
                .arg(source_path)
                .arg("-o")
                .arg(&out_path)
                .status()
                .context("Failed to invoke rustc. Is it on PATH?")?;
            if !status.success() {
                bail!("rustc exited with status {} while compiling {}", status, source_path.display());
            }
        }
        other => bail!(
            "Unsupported target source extension '.{}'. Supported: .c, .cpp, .cc, .cxx, .rs, .ll",
            other
        ),
    }

    Ok(out_path)
}

/// A resolved value: either a concrete leaf the engine's IR understands, or
/// a constructed expression tree carried forward from a prior SSA register.
type SsaMap = HashMap<String, Instruction>;

struct FunctionParse {
    function: Function,
}

fn strip_percent(tok: &str) -> String {
    tok.trim_start_matches('%').to_string()
}

/// Parse an integer/float literal or a `%reg` reference into an Instruction
/// expression tree, using the ssa_map for register resolution.
fn resolve_operand(tok: &str, ssa: &SsaMap) -> Result<Instruction> {
    let tok = tok.trim().trim_end_matches(',');
    if tok.starts_with('%') {
        let reg = strip_percent(tok);
        ssa.get(&reg)
            .cloned()
            .ok_or_else(|| anyhow!("reference to undefined SSA register %{}", reg))
    } else if let Ok(i) = tok.parse::<i64>() {
        Ok(Instruction::Constant { value: i })
    } else if let Ok(f) = tok.parse::<f64>() {
        // This IR's Constant is integer-only; truncate floats toward zero
        // rather than silently losing precision in a way that's hard to spot.
        Ok(Instruction::Constant { value: f as i64 })
    } else if tok == "true" {
        Ok(Instruction::Constant { value: 1 })
    } else if tok == "false" {
        Ok(Instruction::Constant { value: 0 })
    } else {
        bail!("unrecognized operand '{}'", tok)
    }
}

fn map_binop(opcode: &str) -> Option<BinaryOp> {
    match opcode {
        "add" | "fadd" => Some(BinaryOp::Add),
        "sub" | "fsub" => Some(BinaryOp::Sub),
        "mul" | "fmul" => Some(BinaryOp::Mul),
        "sdiv" | "udiv" | "fdiv" => Some(BinaryOp::Div),
        _ => None,
    }
}

fn map_cmp_cond(cond: &str) -> Option<CompareCondition> {
    match cond {
        "eq" | "oeq" => Some(CompareCondition::Eq),
        "ne" | "one" => Some(CompareCondition::Ne),
        "slt" | "ult" | "olt" => Some(CompareCondition::Lt),
        "sle" | "ule" | "ole" => Some(CompareCondition::Le),
        "sgt" | "ugt" | "ogt" => Some(CompareCondition::Gt),
        "sge" | "uge" | "oge" => Some(CompareCondition::Ge),
        _ => None,
    }
}

/// Parse one function body (the lines strictly between the `{` that opens
/// `define ...` and its matching `}`), given the function's name, return
/// type, and an ordered list of (register_name) for its parameters.
///
/// `param_value`: `Some(v)` bakes every parameter as a Constant(v) — used
/// for the top-level target function, which nothing in this module calls,
/// matching module_builders.rs's existing convention. `None` means this
/// function is itself a callee reached via Instruction::Call from another
/// function in this module: its parameters become real Variables bound at
/// call time instead, and `Function.params` gets set so the interpreter
/// knows what to bind (see interpreter.rs's execute_function).
fn parse_function_body(
    fn_name: &str,
    return_type: ValueType,
    params: &[String],
    body_lines: &[String],
    param_value: Option<i64>,
) -> Result<FunctionParse> {
    let label_re = Regex::new(r"^([A-Za-z0-9_.$]+):\s*(;.*)?$").unwrap();
    let alloca_re = Regex::new(r"^%([A-Za-z0-9_.$]+)\s*=\s*alloca\b").unwrap();
    let load_re = Regex::new(r"^%([A-Za-z0-9_.$]+)\s*=\s*load\s+[^,]+,\s*ptr\s+%([A-Za-z0-9_.$]+)").unwrap();
    let store_re = Regex::new(r"^store\s+[A-Za-z0-9_]+\s+(.+?),\s*ptr\s+%([A-Za-z0-9_.$]+)").unwrap();
    let binop_re = Regex::new(r"^%([A-Za-z0-9_.$]+)\s*=\s*(add|sub|mul|sdiv|udiv|fadd|fsub|fmul|fdiv)\b(?:\s+\w+)*\s+[A-Za-z0-9_]+\s+(.+?),\s*(.+)$").unwrap();
    let icmp_re = Regex::new(r"^%([A-Za-z0-9_.$]+)\s*=\s*[fi]cmp\s+(\w+)\s+[A-Za-z0-9_]+\s+(.+?),\s*(.+)$").unwrap();
    let passthrough_re = Regex::new(r"^%([A-Za-z0-9_.$]+)\s*=\s*(sext|zext|trunc|bitcast|fptosi|sitofp|fpext|fptrunc)\b.*\s%([A-Za-z0-9_.$]+)\s+to\b").unwrap();
    let br_cond_re = Regex::new(r"^br\s+i1\s+%([A-Za-z0-9_.$]+),\s*label\s+%([A-Za-z0-9_.$]+),\s*label\s+%([A-Za-z0-9_.$]+)").unwrap();
    let br_uncond_re = Regex::new(r"^br\s+label\s+%([A-Za-z0-9_.$]+)").unwrap();
    let ret_val_re = Regex::new(r"^ret\s+[A-Za-z0-9_]+\s+(.+)$").unwrap();
    let ret_void_re = Regex::new(r"^ret\s+void\b").unwrap();
    // Call, scalar-args-only (Phase 1 of LLVM-format adoption — see
    // llvm_frontend.rs module doc). Handles an optional destination
    // register, optional tail/musttail/notail keyword, optional return-value
    // attributes (noundef, nonnull, ...) between "call" and the actual
    // type, an explicit known-type token (avoids ambiguity with those
    // attribute words), the callee name, and its argument list.
    let call_re = Regex::new(r"^(?:%([A-Za-z0-9_.$]+)\s*=\s*)?(?:tail\s+|musttail\s+|notail\s+)?call\s+(?:\w+\s+)*?(void|i1|i8|i16|i32|i64|i128|float|double|ptr)\s+@([A-Za-z0-9_.$]+)\s*\(([^)]*)\)").unwrap();

    let mut ssa: SsaMap = HashMap::new();
    let mut alloca_vars: HashMap<String, String> = HashMap::new(); // ptr reg -> var name

    match param_value {
        Some(v) => {
            // Top-level target: nothing calls it, so bake a representative
            // value the same way module_builders.rs bakes "n" — this IR has
            // no caller to supply a real argument for the entry point.
            for p in params {
                ssa.insert(p.clone(), Instruction::Constant { value: v });
            }
        }
        None => {
            // Callee reached via a real Call: parameters are genuine
            // runtime values bound by the interpreter at call time, not
            // baked constants — see Function.params / execute_function.
            for p in params {
                ssa.insert(p.clone(), Instruction::Variable { name: p.clone() });
            }
        }
    }

    let mut function = Function::new(fn_name.to_string(), return_type);
    function.set_params(params.to_vec());
    let mut current_block = BasicBlock::new("entry".to_string());

    for raw_line in body_lines {
        let line_full = raw_line.trim();
        if line_full.is_empty() {
            continue;
        }
        // Strip trailing `; comment` (but only the metadata-comment suffix,
        // not anything inside the instruction itself — at this IR subset
        // there are no string literals to worry about colliding with `;`).
        let line = match line_full.find(" ; ") {
            Some(idx) => line_full[..idx].trim(),
            None => line_full,
        };
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        if let Some(m) = label_re.captures(line) {
            // New basic block. Close out the previous one (push it) and
            // start fresh, unless the previous block is empty (defensive).
            if !current_block.instructions.is_empty() || current_block.name == "entry" {
                function.basic_blocks.push(current_block);
            }
            current_block = BasicBlock::new(m.get(1).unwrap().as_str().to_string());
            continue;
        }

        if let Some(m) = alloca_re.captures(line) {
            let reg = m.get(1).unwrap().as_str().to_string();
            // The alloca register name IS the variable name — clean and
            // matches module_builders.rs's hand-built style directly.
            alloca_vars.insert(reg.clone(), reg);
            continue;
        }

        if let Some(m) = load_re.captures(line) {
            let dest = m.get(1).unwrap().as_str().to_string();
            let ptr_reg = m.get(2).unwrap().as_str();
            let var_name = alloca_vars.get(ptr_reg).ok_or_else(|| {
                anyhow!("load from unknown pointer register %{} (no matching alloca)", ptr_reg)
            })?;
            ssa.insert(dest, Instruction::Variable { name: var_name.clone() });
            continue;
        }

        if let Some(m) = store_re.captures(line) {
            let value_tok = m.get(1).unwrap().as_str();
            let ptr_reg = m.get(2).unwrap().as_str();
            let var_name = alloca_vars.get(ptr_reg).ok_or_else(|| {
                anyhow!("store to unknown pointer register %{} (no matching alloca)", ptr_reg)
            })?.clone();
            let value = resolve_operand(value_tok, &ssa)?;
            current_block.append(Instruction::Store { var_name, value: Box::new(value) });
            continue;
        }

        if let Some(m) = binop_re.captures(line) {
            let dest = m.get(1).unwrap().as_str().to_string();
            let opcode = m.get(2).unwrap().as_str();
            let lhs_tok = m.get(3).unwrap().as_str();
            let rhs_tok = m.get(4).unwrap().as_str();
            let op = map_binop(opcode).ok_or_else(|| anyhow!("unsupported binary opcode '{}'", opcode))?;
            let lhs = resolve_operand(lhs_tok, &ssa)?;
            let rhs = resolve_operand(rhs_tok, &ssa)?;
            ssa.insert(dest, Instruction::BinaryOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) });
            continue;
        }

        if let Some(m) = icmp_re.captures(line) {
            let dest = m.get(1).unwrap().as_str().to_string();
            let cond_str = m.get(2).unwrap().as_str();
            let lhs_tok = m.get(3).unwrap().as_str();
            let rhs_tok = m.get(4).unwrap().as_str();
            let cond = map_cmp_cond(cond_str).ok_or_else(|| anyhow!("unsupported icmp/fcmp condition '{}'", cond_str))?;
            let lhs = resolve_operand(lhs_tok, &ssa)?;
            let rhs = resolve_operand(rhs_tok, &ssa)?;
            ssa.insert(dest, Instruction::Compare { condition: cond, lhs: Box::new(lhs), rhs: Box::new(rhs) });
            continue;
        }

        if let Some(m) = passthrough_re.captures(line) {
            let dest = m.get(1).unwrap().as_str().to_string();
            let src_reg = m.get(3).unwrap().as_str();
            let src = ssa.get(src_reg).cloned().ok_or_else(|| {
                anyhow!("type-cast from undefined SSA register %{}", src_reg)
            })?;
            ssa.insert(dest, src);
            continue;
        }

        if let Some(m) = br_cond_re.captures(line) {
            let cond_reg = m.get(1).unwrap().as_str();
            let then_label = m.get(2).unwrap().as_str().to_string();
            let else_label = m.get(3).unwrap().as_str().to_string();
            let condition = ssa.get(cond_reg).cloned().ok_or_else(|| {
                anyhow!("conditional branch on undefined SSA register %{}", cond_reg)
            })?;
            current_block.append(Instruction::Branch {
                condition: Box::new(condition),
                then_label,
                else_label,
            });
            continue;
        }

        if let Some(m) = br_uncond_re.captures(line) {
            let label = m.get(1).unwrap().as_str().to_string();
            current_block.append(Instruction::Jump { label });
            continue;
        }

        if let Some(m) = ret_val_re.captures(line) {
            let value = resolve_operand(m.get(1).unwrap().as_str(), &ssa)?;
            current_block.append(Instruction::Return { value: Some(Box::new(value)) });
            continue;
        }

        if ret_void_re.is_match(line) {
            current_block.append(Instruction::Return { value: None });
            continue;
        }

        if let Some(m) = call_re.captures(line) {
            let dest = m.get(1).map(|g| g.as_str().to_string());
            let function_name = m.get(3).unwrap().as_str().to_string();
            let arg_list_str = m.get(4).unwrap().as_str();

            let mut args = Vec::new();
            if !arg_list_str.trim().is_empty() {
                for raw_arg in arg_list_str.split(',') {
                    // Each arg is "TYPE [attrs...] VALUE" — the value is
                    // always the last whitespace-separated token.
                    let value_tok = raw_arg.trim().rsplit(char::is_whitespace).next()
                        .ok_or_else(|| anyhow!("malformed call argument '{}'", raw_arg))?;
                    args.push(Box::new(resolve_operand(value_tok, &ssa)?));
                }
            }

            // A call with no destination register has no value anything
            // can reference — under this IR's tree-substitution model
            // (every value-producing op gets embedded wherever it's used,
            // same as BinaryOp/Compare), a void call would just silently
            // vanish since nothing ever points at it. Rather than risk
            // dropping a real side-effecting call unnoticed, require a
            // destination for Phase 1 and reject void calls explicitly.
            let Some(dest) = dest else {
                bail!(
                    "void call to '{}' in function '{}' — Phase 1 (scalar-only calls) requires \
                     a used return value; side-effecting void calls aren't supported yet",
                    function_name, fn_name
                );
            };

            ssa.insert(dest, Instruction::Call { function_name, args });
            continue;
        }

        // Anything else (getelementptr, extractvalue, phi, switch,
        // unreachable, vector ops, invoke, ...) is out of scope for this IR
        // — fail loudly with the offending line rather than silently
        // dropping it and producing a module that looks fine but behaves
        // wrong.
        bail!(
            "unsupported LLVM IR construct in function '{}': {}",
            fn_name, line
        );
    }

    if !current_block.instructions.is_empty() || function.basic_blocks.is_empty() {
        function.basic_blocks.push(current_block);
    }

    Ok(FunctionParse { function })
}

fn map_llvm_type(ty: &str) -> ValueType {
    match ty {
        "void" => ValueType::Void,
        "float" | "double" => ValueType::Float,
        _ => ValueType::Int, // i1/i8/i16/i32/i64/i128 all collapse to Int — this IR has no width tracking
    }
}

/// Recursively walk an instruction tree collecting every callee name
/// referenced via Instruction::Call, so parse_ll_file can pull those
/// function bodies in too (Phase 1 only supports intra-module calls — any
/// callee not defined in this same .ll file is a hard parse error, not a
/// silently-ignored gap).
fn collect_called_functions(inst: &Instruction, out: &mut std::collections::HashSet<String>) {
    match inst {
        Instruction::Call { function_name, args } => {
            out.insert(function_name.clone());
            for a in args {
                collect_called_functions(a, out);
            }
        }
        Instruction::BinaryOp { lhs, rhs, .. } | Instruction::Compare { lhs, rhs, .. } => {
            collect_called_functions(lhs, out);
            collect_called_functions(rhs, out);
        }
        Instruction::Store { value, .. } => collect_called_functions(value, out),
        Instruction::Branch { condition, .. } => collect_called_functions(condition, out),
        Instruction::Return { value: Some(v) } => collect_called_functions(v, out),
        Instruction::Constant { .. } | Instruction::Variable { .. }
        | Instruction::Jump { .. } | Instruction::Return { value: None } => {}
    }
}

/// Locate a `define ... @name(...) { ... }` block for a specific function
/// name in the full .ll text. Returns (return_type, param_registers, body_lines).
fn find_and_parse_definition(text: &str, name: &str, bake_param_value: Option<i64>) -> Result<Function> {
    let define_re = Regex::new(&format!(
        r"(?m)^define[^@]*@{}\s*\(([^)]*)\)[^\{{]*\{{",
        regex::escape(name)
    )).unwrap();
    let ret_type_re = Regex::new(r"^define\s+\S*\s*([A-Za-z][A-Za-z0-9]*)\b").unwrap();

    let caps = define_re.captures(text).ok_or_else(|| {
        anyhow!(
            "call to '{}' but no definition found in this file — Phase 1 only supports \
             calls to other functions defined in the same module (no external/libc/std calls)",
            name
        )
    })?;

    let whole_match = caps.get(0).unwrap();
    let header = whole_match.as_str();
    let body_start = whole_match.end();

    let return_type = ret_type_re
        .captures(header)
        .and_then(|c| c.get(1).map(|g| map_llvm_type(g.as_str())))
        .unwrap_or(ValueType::Int);

    let param_list_str = caps.get(1).unwrap().as_str();
    let param_re = Regex::new(r"%([A-Za-z0-9_.$]+)").unwrap();
    let params: Vec<String> = param_re
        .captures_iter(param_list_str)
        .map(|c| c.get(1).unwrap().as_str().to_string())
        .collect();

    let rest = &text[body_start..];
    let mut depth = 1i32;
    let mut end_idx = None;
    for (i, ch) in rest.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end_idx = end_idx.ok_or_else(|| anyhow!("unterminated function body for '{}'", name))?;
    let body_text = &rest[..end_idx];
    let body_lines: Vec<String> = body_text.lines().map(|l| l.to_string()).collect();

    let parsed = parse_function_body(name, return_type, &params, &body_lines, bake_param_value)
        .with_context(|| format!("while parsing function '{}'", name))?;

    Ok(parsed.function)
}

/// Parse a textual .ll file into a Module. `param_value` is baked in for the
/// TOP-LEVEL target function's parameters only (nothing calls it, so there's
/// no real caller-supplied argument — same convention module_builders.rs
/// uses for its synthetic modules' "n" parameter). Any function the target
/// calls (transitively) gets pulled in too, with REAL params bound at call
/// time via Instruction::Call — see parse_function_body's doc comment.
///
/// If `fn_filter` is given, only that function is used as the entry point;
/// otherwise the first function definition found is used.
pub fn parse_ll_file(ll_path: &Path, module_name: &str, param_value: i64, fn_filter: Option<&str>) -> Result<Module> {
    let text = fs::read_to_string(ll_path)
        .with_context(|| format!("failed to read {}", ll_path.display()))?;

    let define_re = Regex::new(r"(?m)^define[^@]*@([A-Za-z0-9_.$]+)\s*\(").unwrap();

    let entry_name = match fn_filter {
        Some(f) => f.to_string(),
        None => define_re.captures(&text)
            .map(|c| c.get(1).unwrap().as_str().to_string())
            .ok_or_else(|| anyhow!("no function definitions found in {}", ll_path.display()))?,
    };

    let mut module = Module::new(module_name.to_string());
    let entry_fn = find_and_parse_definition(&text, &entry_name, Some(param_value))
        .with_context(|| format!("entry function '{}' in {}", entry_name, ll_path.display()))?;

    let mut pending: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    seen.insert(entry_fn.name.clone());
    for bb in &entry_fn.basic_blocks {
        for inst in &bb.instructions {
            let mut called = std::collections::HashSet::new();
            collect_called_functions(inst, &mut called);
            for name in called {
                if seen.insert(name.clone()) {
                    pending.push(name);
                }
            }
        }
    }
    module.functions.push(entry_fn);

    // BFS worklist: every callee can itself call further functions.
    while let Some(name) = pending.pop() {
        let callee_fn = find_and_parse_definition(&text, &name, None)
            .with_context(|| format!("callee function '{}' (reached from '{}') in {}", name, entry_name, ll_path.display()))?;
        for bb in &callee_fn.basic_blocks {
            for inst in &bb.instructions {
                let mut called = std::collections::HashSet::new();
                collect_called_functions(inst, &mut called);
                for n in called {
                    if seen.insert(n.clone()) {
                        pending.push(n);
                    }
                }
            }
        }
        module.functions.push(callee_fn);
    }

    Ok(module)
}

/// Top-level entry point used by main.rs: compile (if needed) + parse a
/// target source file into a Module ready to hand to EvolutionDaemon/
/// SelfEvolvingEngine. Returns the module plus the theoretical fitness
/// ceiling for score_pipeline()'s formula (baseline_instrs - current_instrs):
/// 0 = no improvement over the unmodified module, POSITIVE = real
/// improvement, ceiling (= baseline) reached only if every instruction is
/// eliminated.
pub fn load_target_module(
    source_path: &Path,
    param_value: i64,
    fn_filter: Option<&str>,
) -> Result<(Module, f64)> {
    let ext = source_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let module = if ext == "json" {
        // Module already derives Serialize/Deserialize end-to-end (Module,
        // Function, BasicBlock, Instruction all have the derive) — this
        // previously had a comment claiming "full deserialization isn't
        // done yet" and fell back to a single dummy empty function, which
        // wasn't true; real round-trip deserialization just works.
        let text = fs::read_to_string(source_path)
            .with_context(|| format!("failed to read {}", source_path.display()))?;
        serde_json::from_str::<Module>(&text)
            .with_context(|| format!("failed to deserialize Module JSON from {}", source_path.display()))?
    } else {
        let ll_path = compile_source_to_ll(source_path)
            .with_context(|| format!("compiling target {}", source_path.display()))?;

        let module_name = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("target")
            .to_string();

        parse_ll_file(&ll_path, &format!("target_{}", module_name), param_value, fn_filter)
            .with_context(|| format!("parsing {}", ll_path.display()))?
    };

    let baseline = module.instruction_count() as f64;
    let ceiling = baseline; // fitness = baseline - current; ceiling reached as current -> 0

    Ok((module, ceiling))
}
