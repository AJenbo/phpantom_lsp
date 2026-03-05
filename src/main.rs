use clap::builder::styling::AnsiColor;
use clap::builder::Styles;
use clap::Parser;
use phpantom_lsp::Backend;
use tower_lsp::{LspService, Server};

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Yellow.on_default().bold())
    .usage(AnsiColor::Yellow.on_default().bold())
    .literal(AnsiColor::Green.on_default().bold())
    .placeholder(AnsiColor::Green.on_default());

#[derive(Parser)]
#[command(name = "phpantom_lsp", styles = STYLES)]
#[command(
    version,
    about = "A fast and lightweight PHP Language Server Protocol implementation"
)]
struct Cli {}

#[tokio::main]
async fn main() {
    Cli::parse();

    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
