extern crate xi_lsp_lib;

use xi_lsp_lib::LspPlugin;

struct JSONLspPlugin {
    
}

impl LspPlugin for JSONLspPlugin {

    fn init_language_server() -> bool {

    }

}

fn main() {

    println!("Hello, world!");
}
