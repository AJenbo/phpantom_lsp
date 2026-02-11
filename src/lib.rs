use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;

pub struct Backend {
    name: String,
    version: String,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.1.0".to_string(),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_version(&self) -> &str {
        &self.version
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult::default())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
