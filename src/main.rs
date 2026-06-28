use std::env;
use std::fs;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;
use log::{info, warn, error, LevelFilter};
use env_logger::Builder;
use anyhow::{Result, Context, bail};
use serde::Serialize;

use metamorphic_engine::ir::module::Module;
use metamorphic_engine::ir::function::Function;
use metamorphic_engine::ir::value::ValueType;
use metamorphic_engine::engine::OptimizationEngine;
use metamorphic_engine::passes::OptimizationLevel;
use metamorphic_engine::interpreter::Interpreter;
use metamorphic_engine::profiler::RuntimeProfiler;
use metamorphic_engine::self_evolving_engine::SelfEvolvingEngine;
use metamorphic_engine::goal_definition::GoalDefinition;
use metamorphic_engine::module_builders;
use metamorphic_engine::evolution_daemon::EvolutionDaemon;
use metamorphic_engine::engine_server::EngineServer;
use metamorphic_engine::teacher::Teacher;
use metamorphic_engine::ir_generator::IRGenerator;
use metamorphic_engine::llvm_frontend;

// score_pipeline() in self_evolving_engine.rs is allocation-heavy (clones whole
// modules + builds PassManagers on every call, now happening in parallel across
// islands). mimalloc handles this kind of small/short-lived-allocation-heavy
// workload meaningfully faster than the system default allocator.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Constants from C++ main.cpp or defaults
const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

fn init_logger() {
    Builder::new()
        .filter_level(LevelFilter::Info)
        .filter_module("neuralneat", LevelFilter::Warn) // silence NEAT genome spam
        .parse_default_env()
        .init();
}

#[tokio::main] // Use tokio for async operations like EngineServer
async fn main() -> Result<()> {
    init_logger(); // Initialize logging

    // BlackWall: set HIGH process priority for maximum throughput
    // and use all available CPU cores.
    // This is OS-specific and requires platform-specific Rust crates or FFI.
    // For now, log a message.
    info!("[BlackWall] Process priority: HIGH | CPU cores: (Rust equivalent not directly implemented yet)");


    let args: Vec<String> = env::args().collect();
    let mut demo_mode = false;
    let mut daemon_mode = false;
    let mut generate_mode = false;
    let mut target_mode = false;
    let mut wildcard_mode = false;
    let mut goal_name = "minimize_instrs".to_string();
    let mut target_file = String::new();
    let mut target_arg: i64 = 10;
    let mut target_fn: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--demo" => demo_mode = true,
            "--daemon" => daemon_mode = true,
            "--generate" => generate_mode = true,
            "--wild" => wildcard_mode = true,
            "--goal" => {
                i += 1;
                if i < args.len() {
                    goal_name = args[i].clone();
                }
            }
            "--target" => {
                i += 1;
                if i < args.len() {
                    target_mode = true;
                    target_file = args[i].clone();
                }
            }
            "--target-arg" => {
                i += 1;
                if i < args.len() {
                    target_arg = args[i].parse().unwrap_or(10);
                }
            }
            "--target-fn" => {
                i += 1;
                if i < args.len() {
                    target_fn = Some(args[i].clone());
                }
            }
            "--help" => {
                print_help();
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    // Default to demo if no mode specified
    if !demo_mode && !daemon_mode && !generate_mode && !target_mode {
        demo_mode = true;
    }

    let target_spec = if target_mode {
        Some(TargetSpec { path: target_file.clone(), param_value: target_arg, fn_filter: target_fn.clone() })
    } else {
        None
    };

    if daemon_mode {
        run_daemon(wildcard_mode, &goal_name, target_spec).await?;
    } else if generate_mode {
        run_generate_mode().await?;
    } else if target_mode {
        // --target without --daemon: same real ingestion + same persistent
        // daemon/dashboard infrastructure, just defaulting goal/wildcard.
        // There's no reason to keep the old lighter-weight path around —
        // it never started the EngineServer/dashboard and had no Ctrl-C
        // handling, so it was strictly worse for actually watching a run.
        run_daemon(false, &goal_name, target_spec).await?;
    } else {
        run_demo();
    }

    Ok(())
}

struct TargetSpec {
    path: String,
    param_value: i64,
    fn_filter: Option<String>,
}

fn print_help() {
    println!("Metamorphic Engine");
    println!("Usage: metamorphic [options]");
    println!("\nOptions:");
    println!("  --demo            Run demo mode (default)");
    println!("  --daemon          Run in daemon mode (continuous evolution)");
    println!("  --generate        Generate new module via h3 (empty slate mode)");
    println!("  --wild            Enable wildcard IR mutation mode");
    println!("  --goal <name>     Set goal for daemon mode");
    println!("                     Options: minimize_instrs, minimize_time,");
    println!("                             token_comm, max_branch_elim");
    println!("  --target <path>   Ingest a real program as the evolution target");
    println!("                     (.c/.cpp/.cc/.cxx via clang, .rs via rustc, .ll direct,");
    println!("                      .json for a serialized Module). Combine with --daemon");
    println!("                     to run it through the full persistent daemon+dashboard.");
    println!("  --target-arg <N>  Value baked in for the target function's parameter(s)");
    println!("                     (default: 10). This IR has no Call instruction, so");
    println!("                     parameters are baked as constants, not passed at runtime.");
    println!("  --target-fn <name> Which function in the target file to ingest");
    println!("                     (default: first function definition found)");
    println!("  --help            Show this help message");
    println!("\nEnvironment Variables:");
    println!("  OPENROUTER_API_KEY  API key for Teacher (LLM integration)");
}

fn run_demo() {
    info!("Metamorphic Engine - Demo Mode");
    info!("===================================\n");

    let module = module_builders::build_sum_example(10);

    info!("=== Original IR ===\n{:?}", module);

    let mut engine = OptimizationEngine::new(OptimizationLevel::Conservative);
    engine.load_module(module.clone());
    engine.profile("compute_sum").expect("Failed to profile function");
    // C++ printed report directly
    engine.identify_hot_paths(5);

    info!("\n=== Pass Registry ===");
    for id in engine.get_registry().list_all() {
        if let Some(desc) = engine.get_registry().get_descriptor(&id) {
            info!("  [{:?}] {} ({})", desc.safety, desc.name, desc.id);
        }
    }

    // C++ prints pipeline info. PassManager needs method for this.
    // info!("\nPipeline: {}", engine.get_pass_manager().get_pipeline_info());

    info!("\n--- Optimization ---");
    let changed = engine.optimize_hot_paths().expect("Failed to optimize hot paths");
    info!("Optimization {}", if changed { "made changes" } else { "no changes" });

    if changed {
        info!("\n=== Optimized IR ===\n{:?}", engine.get_module().unwrap());
    }

    info!("\n--- Validation ---");
    engine.validate_optimization("compute_sum");

    info!("\n=== Functional Testing ===");
    for n in [5, 10, 20] {
        let test_mod = module_builders::build_sum_example(n);

        let mut p1 = RuntimeProfiler::new();
        let i1 = Interpreter::new();
        let orig = i1.execute_function(&test_mod, &test_mod.functions[0], &[], Some(&mut p1)).expect("Original execution failed");

        let mut test_eng = OptimizationEngine::new(OptimizationLevel::Conservative);
        test_eng.load_module(test_mod.clone());
        test_eng.profile("compute_sum").expect("Failed to profile for test_eng");
        test_eng.identify_hot_paths(1);
        test_eng.optimize_hot_paths().expect("Failed to optimize for test_eng");

        let mut p2 = RuntimeProfiler::new();
        let i2 = Interpreter::new();
        let opt_module = test_eng.get_module().unwrap();
        let opt = i2.execute_function(opt_module, &opt_module.functions[0], &[], Some(&mut p2)).expect("Optimized execution failed");

        let expected = n * (n - 1) / 2;
        info!("n={}: expected={} orig={} opt={} {}", n, expected, orig, opt, if orig == expected as i64 && opt == expected as i64 { "OK" } else { "FAIL" });
    }

    info!("\n=== IR Generation ===");
    {
        let orig = module.functions[0].clone();
        let mut consts = HashMap::new();
        consts.insert("n".to_string(), 5);
        let spec = IRGenerator::specialize_function(&orig, &consts);
        info!("Specialized (n=5):\n{:?}", spec);

        let mut known_values = HashMap::new();
        known_values.insert("i".to_string(), 0);
        known_values.insert("sum".to_string(), 0);
        known_values.insert("n".to_string(), 10);
        let variants = IRGenerator::generate_variants(&orig, &known_values, 4);
        info!("Generated {} variants", variants.len());
    }

    info!("\n--- Self-Evolving Engine ---");
    {
        let mut se = SelfEvolvingEngine::new(engine, OptimizationLevel::Conservative, "demo_se");
        // C++ printed registryInfo, which is PassRegistry.
        // info!("{}", se.registry_info());
        se.evolve(5, false);
    }

    info!("\n=== Demo Complete ===");
}

async fn run_daemon(wildcard: bool, goal_name: &str, target: Option<TargetSpec>) -> Result<()> {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║   Metamorphic Engine - Evolution Daemon          ║");
    if wildcard {
        println!("║   WILDCARD IR MUTATION MODE ENABLED              ║");
    }
    if target.is_some() {
        println!("║   REAL TARGET INGESTION MODE                     ║");
    }
    println!("╚══════════════════════════════════════════════════╝\n");

    let api_key = env::var(OPENROUTER_API_KEY_ENV).ok();
    let teacher = api_key.map(|_key| Teacher::new()); // Teacher needs to be constructed this way
    let teacher = Mutex::new(teacher);

    let all_modules: Vec<Module> = if let Some(spec) = &target {
        let target_path = PathBuf::from(&spec.path);
        if !target_path.exists() {
            error!("[Target] File not found: {}", target_path.display());
            bail!("target file not found: {}", target_path.display());
        }

        info!("[Target] Ingesting real program: {}", target_path.display());
        let (module, ceiling) = llvm_frontend::load_target_module(
            &target_path,
            spec.param_value,
            spec.fn_filter.as_deref(),
        ).with_context(|| format!("failed to ingest target {}", target_path.display()))?;

        let baseline = module.instruction_count();
        info!("[Target] Loaded module '{}': {} function(s), {} instruction(s) baseline",
            module.name, module.functions.len(), baseline);
        info!("[Target] Theoretical fitness ceiling for this module: {:.4}", ceiling);
        info!("[Target] (fitness = baseline_instrs - current_instrs: 0 = no improvement,");
        info!("[Target]  POSITIVE = real improvement, negative = pipeline made it worse.");
        info!("[Target]  Ceiling ({:.4}) = every instruction eliminated, current->0.)", ceiling);

        vec![module]
    } else {
        let modules = vec![
            module_builders::build_simple_sum(20),
            module_builders::build_fib_loop(15),
            module_builders::build_nested_loop(8),
            module_builders::build_branch_heavy(16),
            module_builders::build_entropy_loop(24),
        ];
        info!("Synthetic modules: {}", modules.len());

        let uro_mods = module_builders::load_uroboros_library();
        let mut all = modules;
        all.extend(uro_mods);
        info!("Total modules: {}", all.len());
        all
    };

    let daemon_teacher = teacher.into_inner().ok().flatten();

    // Create engine server with broadcast capability
    let tuning = std::sync::Arc::new(Mutex::new(metamorphic_engine::evolution_daemon::DaemonTuning::default()));
    let mut engine_server = EngineServer::new(9877);
    engine_server.set_tuning(std::sync::Arc::clone(&tuning));
    let server_clone = engine_server.clone_handle();

    // Start the server in background
    let server_handle = tokio::spawn(async move {
        let _ = server_clone.start().await;
    });

    let state_file = if target.is_some() { "target_best_pipelines.json" } else { "daemon_best_pipelines.json" };

    let mut daemon = EvolutionDaemon::new(
        all_modules,
        state_file,
        wildcard,
        daemon_teacher
    );
    daemon.set_tuning(std::sync::Arc::clone(&tuning));


    let goal = match goal_name {
        "minimize_instrs" => GoalDefinition::minimize_instructions(8),
        "minimize_time" => GoalDefinition::minimize_time(10.0),
        "token_comm" => GoalDefinition::token_communication(),
        "max_branch_elim" => GoalDefinition::maximize_branch_elimination(),
        _ => GoalDefinition::minimize_instructions(8), // Default
    };
    daemon.set_goal(goal);

    // Set the engine server reference for broadcasting
    daemon.set_engine_server(engine_server);

    // Run in a separate thread to not block the async runtime
    let handle = tokio::task::spawn_blocking(move || {
        daemon.run();
    });

    // Wait for daemon to complete or handle Ctrl-C
    tokio::select! {
        _ = handle => {}
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl-C received, shutting down daemon");
        }
    }

    Ok(())
}

async fn run_generate_mode() -> Result<()> {
    info!("Metamorphic Engine - Empty Slate Mode");
    info!("==========================================\n");

    let _api_key = env::var(OPENROUTER_API_KEY_ENV).ok();
    let mut teacher = Teacher::new(); // Teacher needs to be constructed this way

    if !teacher.is_available() {
        error!("Teacher not available for generate mode. Set OPENROUTER_API_KEY.");
        return Ok(());
    }

    info!("[Generate] Asking h3 to suggest a module description...");
    let goal_desc = "minimize instruction count for a computational task";

    let system_prompt = "You are h3, an AI assistant for the Metamorphic Engine. \
                         Generate a clear, concise description of a computational module that \
                         would be useful for IR optimization testing. The module should be a \
                         self-contained function that performs a meaningful computation. \
                         Describe what it computes, any loops, and key operations. \
                         Keep the description to 2-3 sentences.";

    let user_prompt = format!("Please generate a module description for goal: {}", goal_desc);

    let response = teacher.chat(system_prompt, &user_prompt);

    let module_description = match response {
        Some(r) if !r.is_empty() => r,
        _ => {
            warn!("[Generate] Failed to get response from h3. Using default.");
            "A function that computes the sum of squares from 1 to N using a loop".to_string()
        }
    };

    info!("[h3] Module description: {}", module_description);

    info!("[Generate] Translating description to IR...");

    let mut new_module = if module_description.contains("sum") && module_description.contains("squares") {
        // Build sum of squares
        // Placeholder for a real sum of squares builder, using nested_loop for now
        module_builders::build_nested_loop(10)
    } else {
        // Default: simple sum loop
        module_builders::build_simple_sum(20)
    };
    new_module.name = "gen_default".to_string(); // Name should reflect generation, not builder

    info!("[Generate] Generated module:\n{:?}", new_module);

    // Save to blueprints/generated/
    let generated_dir = PathBuf::from("blueprints/generated");
    fs::create_dir_all(&generated_dir)?;
    let filename = generated_dir.join(format!("{}.json", new_module.name));

    #[derive(Serialize)]
    struct GeneratedModuleMeta {
        module_name: String,
        description: String,
        timestamp: u64,
        functions_count: usize,
    }

    let meta = GeneratedModuleMeta {
        module_name: new_module.name.clone(),
        description: module_description,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        functions_count: new_module.functions.len(),
    };

    if let Ok(json_string) = serde_json::to_string_pretty(&meta) {
        fs::write(&filename, json_string)?;
        info!("[Generate] Saved to {}", filename.display());
    }

    info!("\n[Generate] Starting evolution of generated module...");
    let modules = vec![new_module];

    let mut daemon = EvolutionDaemon::new(
        modules,
        "generated_best_pipelines.json",
        false, // No wildcard for generated
        Some(teacher),
    );
    daemon.run();

    Ok(())
}

