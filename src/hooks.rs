// Send actions and update the state. Each tick returns the delta.

pub trait GameHooks: Send + 'static {
    type Delta: Send;
    type Action: Send;
    type Options: Default;

    fn build(options: Self::Options) -> Self;
    fn diff(&self, actions: &[Self::Action]) -> Self::Delta;
    fn update(&mut self, actions: Vec<Self::Action>);
    fn merge(&self, actions: Vec<Self::Delta>) -> Vec<Self::Delta>;
    fn is_finished(&self) -> bool;
}
