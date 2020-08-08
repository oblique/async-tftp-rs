use async_executor::{Spawner, Task};
use once_cell::sync::OnceCell;
use std::future::Future;

use crate::error::*;

static SPAWNER: OnceCell<Spawner> = OnceCell::new();

pub fn set_spawner(spawner: Spawner) -> Result<()> {
    SPAWNER.set(spawner).map_err(|_| Error::SpawnerAlreadySet)
}

pub(crate) fn spawn<T>(
    future: impl Future<Output = T> + Send + 'static,
) -> Task<T>
where
    T: Send + 'static,
{
    match SPAWNER.get() {
        Some(spawner) => spawner.spawn(future),
        None => Task::spawn(future),
    }
}
