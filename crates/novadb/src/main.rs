use novadb_network::run;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _ = run().await?;
    Ok(())
}
