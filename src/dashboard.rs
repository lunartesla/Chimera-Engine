use std::collections::HashMap;
use std::io::{stdout, Write};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{MoveTo, Hide, Show},
    execute,
    style::{Print, ResetColor, SetForegroundColor, Color},
    terminal::{Clear, ClearType, enable_raw_mode, disable_raw_mode},
    ExecutableCommand,
};
use log::info; // For internal logging, not dashboard output

// From C++ Dashboard.h
#[derive(Debug, Clone, Default)]
pub struct DashboardState {
    pub total_gens: i64,
    pub best_fitness: f64,
    pub stuck: i32,
    pub active_module: String,
    pub neat_gen: i32,
    pub neat_species: i32,
    pub neat_fitness: f64,
    pub neat_records: i32,
    pub active_strains: i32,
    pub strain_summary: String,
    pub teacher_connected: bool,
    pub injections: i32,
    pub last_injection: String,
    pub runtime_secs: i64,
    pub model: String,
}

pub struct Dashboard {
    state: Arc<Mutex<DashboardState>>,
    running: Arc<AtomicBool>,
    render_thread: Option<thread::JoinHandle<()>>,
    first_render: bool,
    start_row: u16,
}

impl Dashboard {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DashboardState::default())),
            running: Arc::new(AtomicBool::new(false)),
            render_thread: None,
            first_render: true,
            start_row: 0,
        }
    }

    pub fn start(&mut self) {
        if self.running.load(Ordering::SeqCst) { return; }

        info!("Starting dashboard...");
        self.running.store(true, Ordering::SeqCst);
        self.first_render = true;

        // Clear screen and hide cursor on start
        let mut stdout = stdout();
        stdout.execute(Clear(ClearType::All)).unwrap();
        stdout.execute(Hide).unwrap();
        stdout.execute(MoveTo(0,0)).unwrap(); // Start rendering from top-left

        let state_clone = Arc::clone(&self.state);
        let running_clone = Arc::clone(&self.running);

        self.render_thread = Some(thread::spawn(move || {
            let mut out = std::io::stdout();
            while running_clone.load(Ordering::SeqCst) {
                // To avoid flickering, only update every 2 seconds as in C++
                thread::sleep(Duration::from_secs(2));
                Self::render_loop_step(&state_clone, &mut out);
            }
            // Restore cursor and show it on exit
            out.execute(Show).unwrap();
            out.flush().unwrap();
            info!("Dashboard thread stopped.");
        }));
    }

    pub fn stop(&mut self) {
        if !self.running.load(Ordering::SeqCst) { return; }
        info!("Stopping dashboard...");
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.render_thread.take() {
            handle.join().expect("Failed to join dashboard render thread");
        }
    }

    pub fn update(&self, new_state: DashboardState) {
        let mut state = self.state.lock().unwrap();
        *state = new_state;
    }

    fn render_loop_step(state_arc: &Arc<Mutex<DashboardState>>, stdout: &mut std::io::Stdout) {
        let state = state_arc.lock().unwrap().clone(); // Clone for rendering to avoid holding lock too long

        // Move to the top-left for re-rendering
        stdout.execute(MoveTo(0, 0)).unwrap();

        let sep = format!("{:-<57}", ""); // 57 hyphens
        let mut print_line = |s: String| {
            stdout.execute(Print(format!("{}\r\n", s))).unwrap(); // Clear line and print
        };

        print_line(sep.clone());
        print_line(format!(" BlackWall Evolution System | Runtime: {}", Self::fmt_runtime(state.runtime_secs)));
        print_line(sep.clone());

        print_line(format!(
            " GEN  | {:>8}  BEST | {:.3}   STUCK | {:>3}   MOD | {}",
            Self::fmt_num(state.total_gens),
            state.best_fitness,
            state.stuck,
            state.active_module
        ));

        print_line(format!(
            " NEAT | Gen {:>3}  Species: {:>2}   Fitness: {:.3}   Records: {:>5}",
            state.neat_gen,
            state.neat_species,
            state.neat_fitness,
            state.neat_records
        ));

        print_line(format!(
            " STRAINS | Active: {:>2}   {}",
            state.active_strains,
            if state.strain_summary.is_empty() { "No strains yet".to_string() } else { state.strain_summary }
        ));

        print_line(format!(
            " h3   | {}   Injections: {:>3}   Last: {}",
            if state.teacher_connected { format!("CONNECTED ({})", state.model) } else { "DISCONNECTED".to_string() },
            state.injections,
            if state.last_injection.is_empty() { "none".to_string() } else { state.last_injection }
        ));

        print_line(sep);

        stdout.flush().unwrap();
    }

    fn fmt_runtime(secs: i64) -> String {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", h, m, s)
    }

    fn fmt_num(n: i64) -> String {
        // Simple comma formatting for numbers
        n.to_string().chars().rev().enumerate().fold(String::new(), |mut s, (i, c)| {
            if i > 0 && i % 3 == 0 {
                s.push(',');
            }
            s.push(c);
            s
        }).chars().rev().collect()
    }

    pub fn get_state(&self) -> std::sync::MutexGuard<'_, DashboardState> {
        self.state.lock().unwrap()
    }

    pub fn print_event(tag: &str, msg: &str) {
        let mut stdout = stdout();
        // Move to the bottom of the dashboard + 1 line for events, then clear and print
        // This is tricky with crossterm as we don't have absolute screen coordinates easily here.
        // A simpler approach for events is to just print them, they will scroll.
        // C++ version is designed for specific Windows console behavior.
        // For now, just print to stderr to avoid interfering with dashboard updates.
        eprintln!("[{}] {}", tag, msg);
    }

    pub fn print_prompt() {
        let mut stdout = stdout();
        // C++ version moves cursor and prints prompt below events.
        // For now, just print to stderr.
        eprint!("[YOU] > ");
        stdout.flush().unwrap();
    }
}

impl Drop for Dashboard {
    fn drop(&mut self) {
        self.stop(); // Ensure thread is joined and cursor is shown
    }
}
