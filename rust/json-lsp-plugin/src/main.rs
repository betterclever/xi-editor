extern crate xi_lsp_lib;

use xi_lsp_lib::{LSPPlugin, start_language_server};

fn main() {

    let lsp_process = start_language_server("vscode-json-languageserver");
    let plugin = LSPPlugin::new(lsp_process);

    plugin.start_mainloop();
}
