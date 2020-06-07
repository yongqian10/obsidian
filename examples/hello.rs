use obsidian::{context::Context, App};

#[tokio::main]
async fn main() {
    let mut app: App = App::new();

    app.get("/", |_ctx: Context| async { "Hello, Obsidian!" });

    app.listen(3000).await;
}
