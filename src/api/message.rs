use std::borrow::Cow;

pub enum InputMessage<'a> {
    Connect {
        correlation_id: Cow<'a, str>,
        id: u64,
    },
    Create {
        correlation_id: Cow<'a, str>,
        type_: Cow<'a, str>,
        id: Cow<'a, str>,
        options: Option<Vec<u8>>,
    },
    Join {
        correlation_id: Cow<'a, str>,
        type_: Cow<'a, str>,
        id: Cow<'a, str>,
    },
    Action {
        type_: Cow<'a, str>,
        id: Cow<'a, str>,
        data: Vec<u8>,
    },
}

pub enum OutputMessage<'a> {
    Connect {
        correlation_id: Cow<'a, str>,
        success: bool,
    },
    Create {
        correlation_id: Cow<'a, str>,
        success: bool,
    },
    Join {
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
