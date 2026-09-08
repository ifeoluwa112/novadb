// use novadb_common::{encode_response, parse};
// use novadb_server::execute;
// use novadb_storage::Database;
use novadb_network::run;


fn main() -> std::io::Result<()> {
    run()?;
    Ok(())
}
