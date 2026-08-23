#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Set { key: String, value: String },

    Get { key: String },

    Delete { key: String },

    Exists { key: String },

    Keys,

    Ttl { key: String },
}
