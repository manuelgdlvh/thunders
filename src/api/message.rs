use std::borrow::Cow;

pub enum InputMessage {
    Connect {
        correlation_id: String,
        id: u64,
    },
    Create {
        type_: String,
        id: String,
        options: Option<Vec<u8>>,
    },
    Join {
        type_: String,
        id: String,
    },
    Action {
        type_: String,
        id: String,
        data: Vec<u8>,
    },
}

pub enum OutputMessage<'a> {
    Connect {
        correlation_id: Cow<'a, str>,
        success: bool,
    },

    Diff {
        type_: Cow<'static, str>,
        id: Cow<'a, str>,
        finished: bool,
        data: Vec<u8>,
    },
    GenericError {
        description: Cow<'static, str>,
    },
}
