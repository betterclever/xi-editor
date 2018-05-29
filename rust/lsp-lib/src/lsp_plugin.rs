use std;
use std::sync::Arc;
use std::sync::Mutex;
use language_server::LanguageServer;
use jsonrpc_lite::JsonRpc;
use jsonrpc_lite::Id;
use xi_plugin_lib::{Plugin, ChunkCache, View};
use xi_rope::rope::RopeDelta;
use xi_core::{ConfigTable};
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use parse_helper;
use std::io::{BufWriter, BufReader};
use std::collections::HashMap;

pub struct LSPPlugin {
    language_server_ref: Arc<Mutex<LanguageServer>>
}

impl Clone for LSPPlugin {
    fn clone(&self) -> Self {
        LSPPlugin {
            language_server_ref: self.language_server_ref.clone()
        }
    }
}

impl LSPPlugin {

    pub fn new(command: &str, arguments: &[&str]) -> Self {

        let mut process = Command::new(command)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Error Occurred");

        let child_id = Some(process.id());

        let writer = Box::new(BufWriter::new(process.stdin.take().unwrap()));

        let plugin = LSPPlugin {
            language_server_ref : Arc::new(Mutex::new(LanguageServer::new(writer)))
        };

        {
            let plugin_cloned = plugin.clone();
            std::thread::Builder::new()
                .name("STDIN-Looper".to_string())
                .spawn(move || loop {
                    let mut reader = Box::new(BufReader::new(process.stdout.take().unwrap()));
                    match parse_helper::read_message(&mut reader) {
                        Ok(message_str) => plugin_cloned.handle_message(message_str.as_ref()),
                        Err(err) => eprintln!("Error occurred {:?}", err),
                    };
                });
        }

        plugin
    }

    fn write(&self, msg: &str) {
        let mut lang_server = self.language_server_ref.lock().unwrap();
        lang_server.write(msg);
    }

    pub fn handle_message(&self, message: &str) {
        let mut value = JsonRpc::parse(message).unwrap();
        match value {
            JsonRpc::Request(obj) => eprintln!("client received unexpected request: {:?}", obj),
            JsonRpc::Notification(obj) => eprintln!("recv notification: {:?}", obj),
            JsonRpc::Success(ref obj) => {
                let mut lang_server = self.language_server_ref.lock().unwrap();
                let mut result = value.get_result().unwrap().to_owned();
                let id = number_from_id(value.get_id().as_ref());
                lang_server.handle_response(id, result);
            }
            JsonRpc::Error(ref obj) => {
                let mut lang_server = self.language_server_ref.lock().unwrap();
                let mut error = value.get_error().unwrap().to_owned();
                let id = number_from_id(value.get_id().as_ref());
                lang_server.handle_error(id, error);
            }
        };
    }
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

    fn config_changed(&mut self, _view: &mut View<Self::Cache>, _changes: &ConfigTable) {}
}
