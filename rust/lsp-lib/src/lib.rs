extern crate jsonrpc_lite;
extern crate languageserver_types as lsp;
extern crate serde_json;
extern crate xi_core_lib as xi_core;
extern crate xi_plugin_lib;
extern crate xi_rope;

use jsonrpc_lite::{JsonRpc, Id, Error};
use lsp::notification::Notification;
use serde_json::value::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, BufWriter, Write};
use std::process::{Command, Child, ChildStdout, ChildStdin};
use std::process::Stdio;
use std::path::Path;
use xi_core::{ViewIdentifier, ConfigTable};
use xi_rope::rope::RopeDelta;
use xi_plugin_lib::{Cache, Plugin, ChunkCache, View, mainloop};

mod parse_helper;
pub mod types;

pub enum LspACK {
    Ack(u16),
    Error(String),
}

trait Callable: Send {
    fn call(self: Box<Self>, result: Result<Value, Error>);
}

impl<F: Send + FnOnce(Result<Value, Error>)> Callable for F {
    fn call(self: Box<F>, result: Result<Value, Error>) {
        (*self)(result)
    }
}

type Callback = Box<Callable>;

pub struct LSPPlugin {
    writer: Box<Write>,
    reader: Box<BufRead>,
    pending: HashMap<usize, Callback>,
    next_id: usize,
}

pub fn start_language_server(command: String) -> Child {
    
    let process = Command::new(command)
        .arg("stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Error Occurred");

    let child_id = Some(process.id());
    //let reader = Box::new(BufReader::new(process.stdout.take().unwrap()));
    //let writer = Box::new(BufWriter::new(process.stdin.take().unwrap()));

    process
}

fn number_from_id(id: Option<&Id>) -> usize {
    let id = id.expect("response missing id field");
    let id = match id {
        &Id::Num(n) => n as u64,
        &Id::Str(ref s) => u64::from_str_radix(s, 10).expect("failed to convert string id to u64"),
        other => panic!("unexpected value for id field: {:?}", other),
    };

    id as usize
}

impl Plugin for LSPPlugin {
    type Cache = ChunkCache;

    fn update(&mut self, view: &mut View<Self::Cache>, delta: Option<&RopeDelta>,
              _edit_type: String, _author: String) {}

    fn did_save(&mut self, view: &mut View<Self::Cache>, _old: Option<&Path>) {
        eprintln!("saved view {}", view.get_id());
    }

    fn did_close(&mut self, view: &View<Self::Cache>) {
        eprintln!("close view {}", view.get_id());
    }

    fn new_view(&mut self, view: &mut View<Self::Cache>) {
        eprintln!("new view {}", view.get_id());
    }

    fn config_changed(&mut self, _view: &mut View<Self::Cache>, _changes: &ConfigTable) {
    }


}

impl LSPPlugin {
    // This function starts the language server given the command.
    // If the type of the command is TCP, we connect to the TCP Stream
    // else we use a Stdin/Stdout stream

    // For Test, we simple start the JSON Language Server

    /// Create a new LSP Plugin with given start command.
    pub fn new(child: Child) -> Self {

        let reader = Box::new(BufReader::new(child.stdout.take().unwrap()));
        let writer = Box::new(BufWriter::new(child.stdin.take().unwrap()));

        LSPPlugin {
            reader,
            writer,
            pending: HashMap::new(),
            next_id: 1,
        }
    }

    fn write(&mut self, msg: &str) {
        self.writer
            .write_all(msg.as_bytes())
            .expect("error writing to stdin");

        self.writer.flush().expect("error flushing child stdin");
    }

    fn handle_response(&mut self, id: usize, result: Value) {
        let callback = self.pending.remove(&id).expect(&format!("id {} missing from request table", id));
        callback.call(Ok(result));
    }

    fn handle_error(&mut self, id: usize, error: Error) {
        let callback = self.pending.remove(&id).expect(&format!("id {} missing from request table", id));
        callback.call(Err(error));
    }

    fn handle_message(&mut self, message: &str) {
        let value = JsonRpc::parse(message).unwrap();
        match value{
            JsonRpc::Request(obj) => eprintln!("client received unexpected request: {:?}", obj),
            JsonRpc::Notification(obj) => eprintln!("recv notification: {:?}", obj),
            JsonRpc::Success(ref mut obj) => {
                let mut result = value.get_result().unwrap().to_owned();
                let id = number_from_id(value.get_id().as_ref());
                self.handle_response(
                    id,
                    result,
                );
            }
            JsonRpc::Error(ref mut obj) => {
                let id = number_from_id(value.get_id().as_ref());

                let mut error = value.get_error().unwrap().to_owned();
                self.handle_error(id, error);
            }
        };
    }

    // Start the plugin
    pub fn start_mainloop(&mut self) {

        // Start a new thread and handle the RPC
        std::thread::Builder::new()
            .name("STDIN-Looper".to_string())
            .spawn(move || loop {
                match parse_helper::read_message(&mut self.reader) {
                    Ok(message_str) => self.handle_message(message_str.as_ref()),
                    Err(err) => eprintln!("Error occurred {:?}", err),
                };
            });

        mainloop(&mut self).unwrap();
    }

    /// Send a JSON-RPC request to the Language Server. The supplied callback is called
    /// on the response to the plugin.
    pub fn send_request<CB>(&self, method: &str, params: &Value, completion: CB)
    where
        CB: 'static + Send + FnOnce(Result<Value, Value>),
    {
        self.writer.send_request(method, params, Box::new(completion));
    }
}
