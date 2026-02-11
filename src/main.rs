use phpantom_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::Server;

#[tokio::main]
async fn main() {
    let (service, socket) = LspService::new(|_client| Backend::new());
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}
