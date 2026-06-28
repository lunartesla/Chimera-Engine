#![allow(unused)]

pub mod ir;
pub mod passes;
pub mod engine;
pub mod validator;
pub mod profiler;
pub mod interpreter;
pub mod self_evolving_engine;
pub mod evolution_daemon;
pub mod module_builders;
pub mod engine_server;
pub mod terminal_chat;
pub mod blueprint_archive;
pub mod strain;
pub mod goal_definition;
pub mod ir_generator;
pub mod dashboard;
pub mod teacher;
pub mod neural_predictor;
pub mod llvm_frontend;

#[cfg(not(feature = "no-capi"))]
pub mod capi;

pub use ir::*;
pub use passes::*;
pub use engine::*;
pub use validator::*;
pub use profiler::*;
pub use interpreter::*;
pub use self_evolving_engine::*;
pub use evolution_daemon::EvolutionDaemon;
pub use module_builders::*;
pub use engine_server::EngineServer;
pub use terminal_chat::TerminalChat;
pub use blueprint_archive::*;
pub use strain::*;
pub use goal_definition::*;
pub use ir_generator::*;
pub use dashboard::*;
pub use teacher::*;
pub use neural_predictor::*;
pub use llvm_frontend::*;

#[cfg(not(feature = "no-capi"))]
pub use capi::*;
