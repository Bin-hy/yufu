use crate::config::Config;

pub(crate) mod hello;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
}
