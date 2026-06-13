use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::collections::VecDeque;
use crossterm::{event::{read, Event, KeyCode, KeyEvent, KeyModifiers}, terminal::{enable_raw_mode, disable_raw_mode, Clear, ClearType}, ExecutableCommand};
use crossterm::cursor::{MoveTo, position};
use crossterm::style::{SetForegroundColor, Print, ResetColor, Color};
use std::io::{self, stdout, Write as IoWrite};
use std::fmt::Write as FmtWrite; // For write! macro
use log::{info, warn};

use crate::dashboard::{Dashboard, DashboardState};
use crate::evolution_daemon::EvolutionDaemon;
use crate::teacher::Teacher; // Assuming Teacher will be used for H3 communication

// Defined in C++ TerminalChat.h
#[derive(Debug, Clone)]
pub struct MutationOutcome {
    pub pass_id: String,
    pub mutation_type: String,
    pub fitness_delta: f64,
}

pub struct TerminalChat {
    daemon_handle: Arc<Mutex<Option<EvolutionDaemon>>>, // To send commands to daemon
    teacher: Arc<Mutex<Option<Teacher>>>, // For H3 communication
    dashboard: Arc<Mutex<Dashboard>>,

    input_thread: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,

    command_queue: Arc<Mutex<VecDeque<String>>>,
    outcome_mutex: Arc<Mutex<VecDeque<MutationOutcome>>>, // C++ had outcomeMutex

    // Context for dashboard update (C++ stored these directly)
    total_gens_ref: Arc<Mutex<i64>>,
    runtime_secs_ref: Arc<Mutex<i64>>,
    total_injections: Arc<Mutex<i32>>,
    last_injection_str: Arc<Mutex<String>>,
}

impl TerminalChat {
    pub fn new(daemon_handle: Arc<Mutex<Option<EvolutionDaemon>>>, teacher: Arc<Mutex<Option<Teacher>>>) -> Self {
        Self {
            daemon_handle,
            teacher,
            dashboard: Arc::new(Mutex::new(Dashboard::new())),
            input_thread: None,
            running: Arc::new(AtomicBool::new(false)),
            command_queue: Arc::new(Mutex::new(VecDeque::new())),
            outcome_mutex: Arc::new(Mutex::new(VecDeque::new())),
            total_gens_ref: Arc::new(Mutex::new(0)),
            runtime_secs_ref: Arc::new(Mutex::new(0)),
            total_injections: Arc::new(Mutex::new(0)),
            last_injection_str: Arc::new(Mutex::new(String::new())),
        }
    }

    pub fn start(&mut self) {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        info!("Starting TerminalChat...");
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        self.dashboard.lock().unwrap().start();
        TerminalChat::print_event("BlackWall", "Terminal ready. Type /help for commands.");

        // Enable raw mode for direct keyboard input
        enable_raw_mode().expect("Failed to enable raw mode");

        let running_clone = Arc::clone(&self.running);
        let command_queue_clone = Arc::clone(&self.command_queue);
        let dashboard_clone = Arc::clone(&self.dashboard);

        self.input_thread = Some(thread::spawn(move || {
            let mut input_buf = String::new();
            Self::print_prompt(&dashboard_clone); // Print initial prompt

            while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = read() {
                    match code {
                        KeyCode::Enter => {
                            if !input_buf.is_empty() {
                                let mut queue = command_queue_clone.lock().unwrap();
                                queue.push_back(input_buf.clone());
                                input_buf.clear();
                            }
                            Self::print_new_line();
                            Self::print_prompt(&dashboard_clone);
                        }
                        KeyCode::Backspace => {
                            if !input_buf.is_empty() {
                                input_buf.pop();
                                let mut stdout = stdout();
                                stdout.execute(crossterm::cursor::MoveLeft(1)).unwrap();
                                stdout.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::UntilNewLine)).unwrap();
                                (& mut stdout as &mut dyn IoWrite).flush().unwrap();
                            }
                        }
                        KeyCode::Char(c) => {
                            input_buf.push(c);
                            print!("{}", c);
                            stdout().flush().unwrap();
                        }
                        // Handle Ctrl+C (equivalent to daemon stop)
                        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                            let mut queue = command_queue_clone.lock().unwrap();
                            queue.push_back("/quit".to_string()); // Simulate /quit command
                            input_buf.clear();
                            Self::print_new_line();
                            Self::print_prompt(&dashboard_clone);
                        }
                        _ => {}
                    }
                }
            }
        }));
    }

    pub fn stop(&mut self) {
        if !self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        info!("Stopping TerminalChat...");
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.input_thread.take() {
            handle.join().expect("Failed to join input thread");
        }
        disable_raw_mode().expect("Failed to disable raw mode");
        self.dashboard.lock().unwrap().stop();
    }

    pub fn process_commands(&self) {
        let mut queue = self.command_queue.lock().unwrap();
        while let Some(cmd) = queue.pop_front() {
            self.process_input(&cmd);
        }
    }

    pub fn post_daemon_message(&self, msg: &str) {
        // C++: Events go to log only - dashboard is the screen.
        // Rust: Use log macros for daemon messages.
        info!("[Daemon] {}", msg);
    }

    pub fn post_h3_message(&self, msg: &str) {
        Dashboard::print_event("h3", msg);
    }

    pub fn post_daemon_status(&self, msg: &str) {
        // C++: dashboard handles display
        // Rust: dashboard update takes DashboardState, this is a separate "event"
        Dashboard::print_event("BlackWall", msg);
    }

    pub fn update_context(
        &self,
        mode: &str,
        mod_name: &str,
        mod_shape: &str,
        best_fit: f64,
        goal_thresh: f64,
        neat_gen: i32,
        neat_str: &str,
        strain_info: &Vec<(String, f64, f64)>,
        stuck: i32,
    ) {
        // Build DashboardState from context and update dashboard
        let mut ds = DashboardState::default();
        ds.total_gens = *self.total_gens_ref.lock().unwrap();
        ds.runtime_secs = *self.runtime_secs_ref.lock().unwrap();
        ds.best_fitness = best_fit;
        ds.stuck = stuck;
        ds.active_module = mod_name.to_string();
        ds.neat_gen = neat_gen;
        ds.neat_records = 0; // Placeholder
        ds.neat_species = 0; // Placeholder
        ds.neat_fitness = 0.0; // Placeholder
        ds.strain_summary = strain_info.iter().map(|(id, fit, orig)| {
            let pct = if *orig != 0.0 { (fit - orig) / orig.abs() * 100.0 } else { 0.0 };
            format!("{}: {:.2} vs {:.2} ({:+.1}%)", id, fit, orig, pct)
        }).collect::<Vec<String>>().join("  ");
        ds.active_strains = strain_info.len() as i32;
        ds.teacher_connected = self.teacher.lock().unwrap().as_ref().map_or(false, |t| t.is_available());
        ds.injections = *self.total_injections.lock().unwrap();
        ds.last_injection = self.last_injection_str.lock().unwrap().clone();
        ds.model = "nemotron-120b".to_string(); // Placeholder or get from Teacher

        self.dashboard.lock().unwrap().update(ds);
    }

    pub fn add_mutation_outcome(&self, pass_id: &str, mut_type: &str, delta: f64) {
        let mut outcomes = self.outcome_mutex.lock().unwrap();
        outcomes.push_back(MutationOutcome {
            pass_id: pass_id.to_string(),
            mutation_type: mut_type.to_string(),
            fitness_delta: delta,
        });
        while outcomes.len() > 5 { // C++ keeps 5 recent outcomes
            outcomes.pop_front();
        }
        *self.last_injection_str.lock().unwrap() = format!("+{}", pass_id);
        *self.total_injections.lock().unwrap() += 1;
    }

    // Helper to print events without interfering with dashboard re-renders.
    fn print_event(tag: &str, msg: &str) {
        let mut stdout = stdout();
        // Move to current line, clear to end, print.
        // This is a basic way to print without overwriting the dashboard area.
        stdout.execute(MoveTo(0, crossterm::cursor::position().unwrap().1)).unwrap();
        stdout.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)).unwrap();
        stdout.execute(SetForegroundColor(Color::Cyan)).unwrap();
        stdout.execute(Print(format!("[{}] {}", tag, msg))).unwrap();
        stdout.execute(ResetColor).unwrap();
        stdout.execute(Print("\r\n")).unwrap();
        (&mut stdout as &mut dyn IoWrite).flush().unwrap();
    }

    fn print_prompt(dashboard: &Arc<Mutex<Dashboard>>) {
        let mut stdout = stdout();
        // Move to the line below the dashboard.
        // DASH_LINES is not public on Dashboard. Assuming fixed size.
        stdout.execute(MoveTo(0, 8)).unwrap(); // Adjust to be below dashboard
        stdout.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)).unwrap();
        stdout.execute(SetForegroundColor(Color::Green)).unwrap();
        stdout.execute(Print("[YOU] > ")).unwrap();
        stdout.execute(ResetColor).unwrap();
        (&mut stdout as &mut dyn IoWrite).flush().unwrap();
    }

    fn print_new_line() {
        let mut stdout = stdout();
        stdout.execute(Print("\r\n")).unwrap();
        (&mut stdout as &mut dyn IoWrite).flush().unwrap();
    }

    fn process_input(&self, input: &str) {
        Dashboard::print_event("YOU", input); // Echo command to event log

        if input.starts_with('/') {
            self.handle_slash_command(input);
        } else {
            if self.teacher.lock().unwrap().as_ref().map_or(false, |t| t.is_available()) {
                self.send_to_h3(input);
            } else {
                TerminalChat::print_event("BlackWall", "Teacher not available - set OPENROUTER_API_KEY");
            }
        }
    }

    fn handle_slash_command(&self, cmd: &str) {
        if cmd == "/pause" || cmd == "/quit" {
            if let Some(daemon_ref) = self.daemon_handle.lock().unwrap().as_ref() {
                //daemon_ref.stop(); // Daemon needs to be mutable here
            }
            TerminalChat::print_event("BlackWall", if cmd == "/quit" { "Shutting down..." } else { "Stop requested." });
            self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        } else if cmd == "/status" {
            let mut s = String::new();
            // Need to get context from daemon directly for accurate status
            // For now, use the last updated context
            let ds = self.dashboard.lock().unwrap().get_state().clone();
            write!(s, "Mode={} Module={} Best={:.4} Stuck={} NEATGen={} Strains={}",
                "daemon", // Get actual mode from daemon
                ds.active_module,
                ds.best_fitness,
                ds.stuck,
                ds.neat_gen,
                ds.active_strains,
            ).unwrap();
            TerminalChat::print_event("BlackWall", &s);
        } else if cmd == "/strains" {
            // Need to retrieve strain info from daemon
            TerminalChat::print_event("BlackWall", "Strain info not yet available.");
        } else if cmd == "/neat" {
            if let Some(daemon_ref) = self.daemon_handle.lock().unwrap().as_ref() {
                TerminalChat::print_event("BlackWall", &daemon_ref.persistent_predictor.lock().unwrap().get_status_string());
            } else {
                TerminalChat::print_event("BlackWall", "Daemon not available to get NEAT status.");
            }
        } else if cmd.starts_with("/goal") {
            let gt = cmd.trim_start_matches("/goal").trim();
            TerminalChat::print_event("BlackWall", &format!("Goal: {} (restart with --goal to apply)", gt));
        } else if cmd == "/generate" {
            TerminalChat::print_event("BlackWall", "Use --generate flag on startup.");
        } else if cmd == "/help" {
            TerminalChat::print_event("BlackWall", "/pause /quit /status /strains /neat /goal <text> /generate /help");
        } else {
            TerminalChat::print_event("BlackWall", &format!("Unknown: {} (try /help)", cmd));
        }
    }

    fn send_to_h3(&self, user_message: &str) {
        let mut teacher_guard = self.teacher.lock().unwrap();
        let Some(teacher_ref) = teacher_guard.as_mut() else {
            TerminalChat::print_event("BlackWall", "Teacher not available");
            return;
        };

        if !teacher_ref.is_available() {
            TerminalChat::print_event("BlackWall", "Teacher not available");
            return;
        }

        let ctx = self.build_h3_context();
        let sys = "You are Nemotron, AI co-pilot of the BlackWall Evolution System. Work alongside the daemon to optimize code. Be concise. Use tags when needed: [SET_GOAL: desc] [FORK_STRAIN] [PAUSE] [GENERATE_MODULE: desc].";
        let response = teacher_ref.chat(sys, &format!("Context: {}\nUser: {}", ctx, user_message));

        if let Some(resp) = response {
            self.parse_h3_commands(&resp);
        } else {
            TerminalChat::print_event("h3", "(no response - check API key)");
        }
    }

    fn build_h3_context(&self) -> String {
        let ds = self.dashboard.lock().unwrap().get_state().clone();
        format!("ENGINE: mode={} module={} best={:.4} stuck={} NEATGen={} strains={}",
            "daemon", // Assuming daemon mode
            ds.active_module,
            ds.best_fitness,
            ds.stuck,
            ds.neat_gen,
            ds.active_strains
        )
    }

    fn parse_h3_commands(&self, resp: &str) {
        self.post_h3_message(resp);
        // Implement parsing logic for [SET_GOAL:], [FORK_STRAIN], [PAUSE], [GENERATE_MODULE:]
        // This will involve sending commands back to the daemon.
        if resp.contains("[PAUSE]") {
            if let Some(daemon_ref) = self.daemon_handle.lock().unwrap().as_ref() {
                //daemon_ref.stop(); // Needs mutable daemon ref
            }
        }
        // Other command parsing...
    }
}