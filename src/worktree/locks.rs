use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use tokio::sync::{Mutex, OwnedMutexGuard};

static CREATION_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

pub(super) async fn for_path(path: &Path) -> OwnedMutexGuard<()> {
    let lock = {
        let locks = CREATION_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut locks = locks.lock().await;
        locks
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    lock.lock_owned().await
}
