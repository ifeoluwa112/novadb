pub mod command;
pub mod parser;
pub mod resp;

pub use command::Command;
pub use parser::{ParseError, parse};
pub use resp::{RespValue, RespError, parse_resp};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
