use std::collections::HashMap;

#[derive(Debug)]
pub struct PlayerContext {
    id: u64,
    attrs: HashMap<String, String>,
}

impl PlayerContext {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            attrs: HashMap::default(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}
