extern crate languageserver_types as lsp;
extern crate serde_json;
extern crate xi_core_lib as xi_core;
extern crate xi_plugin_lib;
extern crate xi_rope;

use lsp::notification::Notification;
use serde_json::value::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::Command;
use std::process::Stdio;

mod parse_helper;
pub mod types;

pub enum LspACK {
    Ack(u16),
    Error(String),
}

trait Callable: Send {
    fn call(self: Box<Self>, result: Result<Value, Value>);
}

impl<F: Send + FnOnce(Result<Value, Value>)> Callable for F {
    fn call(self: Box<F>, result: Result<Value, Value>) {
        (*self)(result)
    }
}

type Callback = Box<Callable>;

pub struct LSPPlugin<W: Write> {
    peer: W,
    pending: HashMap<usize, Callback>,
    next_id: usize
}

// Init Server
impl <W: Write> LSPPlugin<W> {
    // This function starts the language server given the command.
    // If the type of the command is TCP, we connect to the TCP Stream
    // else we use a Stdin/Stdout stream

    // For Test, we simple start the JSON Language Server
    fn start_language_server(&mut self) {
        let command = "vscode-json-languageserver";

        let process = Command::new(command)
            .arg("stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let child_id = Some(process.id());
        let reader = Box::new(BufReader::new(process
            .stdout
            .ok_or_else(|| err_msg("Failed to get subprocess stdout"))?));

        let writer = Box::new(BufWriter::new(process
            .stdin
            .ok_or_else(|| err_msg("Failed to get subprocess stdin"))?));

        // Start a new thread and handle the RPC
        std::thread::Builder::new()
            .name("STDIN-Looper".to_string())
            .spawn(move || {
                LSPPlugin::loop_reader(reader)
            })?;
    }
}
