use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, channel},
};

use notify::{
    Event, EventKind, RecommendedWatcher, Watcher, event::ModifyKind, recommended_watcher,
};

pub struct ShaderWatcher {
    _watcher: RecommendedWatcher,
    pub reciever_x: Receiver<PathBuf>,
}

impl ShaderWatcher {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (sender_x, reciever_x) = channel();

        let mut watcher = recommended_watcher(move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                if let EventKind::Modify(ModifyKind::Data(_)) = event.kind {
                    for p in event.paths {
                        let _ = sender_x.send(p);
                    }
                }
            }
            Err(e) => {
                eprintln!("Watcher has the Error: {:?}", e);
            }
        })
        .expect("Failed to create the watcher!");

        watcher
            .watch(&path, notify::RecursiveMode::NonRecursive)
            .expect("Failed to watch the shader path!");

        Self {
            _watcher: watcher,
            reciever_x,
        }
    }
}
