// use novadb_common::{encode_response, parse};
// use novadb_server::execute;
// use novadb_storage::Database;
use novadb_network::run;

// fn _run_command(db: &mut Database, input: &str) {
//     let command = parse(input).unwrap();
//     let response = execute(db, command);
//     let encoded = encode_response(&response);

//     println!("{encoded}");
// }

fn main() -> std::io::Result<()> {
    run()?;
    Ok(())
}
