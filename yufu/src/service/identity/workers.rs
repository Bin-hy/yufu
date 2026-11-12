use super::{logout_dispatcher::LogoutDispatcher, repository::PgIdentityRepository};
use crate::config_identity::Identity;
use auth::security::budget::{Clock, SecurityBudget};
use std::sync::Arc;
use tokio::{sync::watch, task::JoinHandle};

pub struct IdentityWorkers {
    stop: watch::Sender<bool>,
    handles: Vec<JoinHandle<()>>,
}
impl IdentityWorkers {
    pub fn start(
        dispatcher: Arc<LogoutDispatcher>,
        repository: Arc<PgIdentityRepository>,
        clock: Arc<dyn Clock>,
        policy: Identity,
    ) -> Self {
        let (stop, mut delivery_stop) = watch::channel(false);
        let mut cleanup_stop = stop.subscribe();
        let delivery = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! { biased;
                    _ = delivery_stop.changed() => break,
                    _ = interval.tick() => {
                        tokio::select! { biased;
                            _ = delivery_stop.changed() => break,
                            result = dispatcher.dispatch_batch() => {
                                if let Err(reason) = result { tracing::warn!(reason=reason.code(),"logout batch incomplete; durable leases will recover"); }
                            }
                        }
                    }
                }
            }
        });
        let cleanup = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! { biased;
                    _ = cleanup_stop.changed() => break,
                    _ = interval.tick() => {
                        let budget = match SecurityBudget::new(clock.clone()) {Ok(budget)=>budget,Err(_)=>continue};
                        tokio::select! { biased;
                            _ = cleanup_stop.changed() => break,
                            result = repository.cleanup_identity(&budget,&policy,100) => {
                                if let Err(reason) = result { tracing::warn!(reason=reason.code(),"identity cleanup incomplete"); }
                            }
                        }
                    }
                }
            }
        });
        Self {
            stop,
            handles: vec![delivery, cleanup],
        }
    }
    pub async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        for handle in self.handles.drain(..) {
            let _ = handle.await;
        }
    }
}
impl Drop for IdentityWorkers {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}
