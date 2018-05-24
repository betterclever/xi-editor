extern crate jsonrpc_lite;
extern crate languageserver_types as lsp;
extern crate serde_json;
extern crate xi_core_lib as xi_core;
extern crate xi_plugin_lib;
extern crate xi_rope;

use jsonrpc_lite::JsonRpc;
use lsp::notification::Notification;
use serde_json::value::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, BufWriter, Write};
use std::process::{Command, Child, ChildStdout, ChildStdin};
use std::process::Stdio;
use std::path::Path;
use xi_core::{ViewIdentifier, ConfigTable};
use xi_rope::rope::RopeDelta;
use xi_plugin_lib::{Cache, Plugin, StateCache, View, mainloop};

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

pub struct LSPPlugin<W: Write, R: Read> {
    writer: Box<BufWriter<W>>,
    reader: Box<BufReader<R>>,
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

fn number_from_id(id: Option<&Value>) -> usize {
    let id = id.expect("response missing id field");
    let id = match id {
        &Value::Number(ref n) => n.as_u64().expect("failed to take id as u64"),
        &Value::String(ref s) => u64::from_str_radix(s, 10).expect("failed to convert string id to u64"),
        other => panic!("unexpected value for id field: {:?}", other),
    };

    id as usize
}

impl<W: Write, R: Read> LSPPlugin<W,R> {
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
        self.peer.flush().expect("error flushing child stdin");
    }

    fn handle_message(&mut self, message: &Value) {
        match JsonRpc::parse_object(message) {
            JsonRpc::Request(obj) => eprintln!("client received unexpected request: {:?}", obj),
            JsonRpc::Notification(obj) => eprintln!("recv notification: {:?}", obj),
            JsonRpc::Success(ref mut obj) => {
                let mut inner = self.0.lock().unwrap();
                let mut obj = obj.as_object_mut().unwrap();
                let id = number_from_id(obj.get("id"));
                inner.handle_response(
                    id,
                    obj.remove("result")
                        .expect("response missing 'result' field"),
                );
            }
            JsonRpc::Error(ref mut obj) => {
                if obj.get("id").expect("error missing id field").is_null() {
                    let mut inner = self.0.lock().unwrap();
                    let mut obj = obj.as_object_mut().unwrap();
                    inner.handle_error(number_from_id(obj.get("id")), obj.remove("error").unwrap());
                } else {
                    eprintln!("received error: {:?}", obj);
                }
            }
            JsonRpc::ErrorRequst(err) => eprintln!("JSON-RPC error {:?}", err),
        };
    }

    // Start the plugin
    pub fn start_mainloop(&mut self) {

        // Start a new thread and handle the RPC
        std::thread::Builder::new()
            .name("STDIN-Looper".to_string())
            .spawn(move || loop {
                match parse_helper::read_message(self.reader) {
                    Ok(ref val) => self.handle_msg(val),
                    Err(err) => eprintln!("Error occurred {}", err),
                };
            })?;

        mainloop(&mut self).unwrap();
    }

    /// Send a JSON-RPC request to the Language Server. The supplied callback is called
    /// on the response to the plugin.
    pub fn send_request<CB>(&self, method: &str, params: &Value, completion: CB)
    where
        CB: 'static + Send + FnOnce(Result<Value, Value>),
    {
        let mut inner = self.0.lock().unwrap();
        inner.send_request(method, params, Box::new(completion));
    }
}

impl <W: Write, R: Read> Plugin for LSPPlugin<W,R> {
    type Cache = StateCache<()>;

    fn update(
        &mut self,
        view: &mut View<Self::Cache>,
        delta: Option<&RopeDelta>,
        edit_type: String,
        author: String,
    ) {

    }

    fn did_save(&mut self, view: &mut View<Self::Cache>, old_path: Option<&Path>) {}

    fn did_close(&mut self, view: &View<Self::Cache>) {}

    fn new_view(&mut self, view: &mut View<Self::Cache>) {}

    fn config_changed(&mut self, view: &mut View<Self::Cache>, changes: &ConfigTable) {}

    #[allow(unused_variables)]
    fn idle(&mut self, view: &mut View<Self::Cache>) {}
}
