pub enum InputMessage<'a> {
    Connect {
        correlation_id: &'a str,
        id: u64,
    },
    Create {
        correlation_id: &'a str,
        type_: &'a str,
        id: &'a str,
        options: Option<&'a [u8]>,
    },
    Join {
        correlation_id: &'a str,
        type_: &'a str,
        id: &'a str,
    },
    Action {
        type_: &'a str,
        id: &'a str,
        data: &'a [u8],
    },
}

pub enum OutputMessage<'a> {
    Connect {
        correlation_id: &'a str,
        success: bool,
    },
    Create {
        correlation_id: &'a str,
        success: bool,
    },
    Join {
        correlation_id: &'a str,
        success: bool,
    },
    Diff {
        type_: &'a str,
        id: &'a str,
        finished: bool,
        data: &'a [u8],
    },
    GenericError {
        description: &'a str,
    },
}
