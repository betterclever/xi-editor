extern crate xi_lsp_lib;
use xi_lsp_lib::{LSPPlugin, start_mainloop};

fn main() {

    let mut plugin = LSPPlugin::new("vscode-json-languageserver",&["--stdio"]);
    start_mainloop(&mut plugin);
    
}

