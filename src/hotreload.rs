use crate::skills::Skill;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;

pub struct SkillWatcher {
    rx: mpsc::Receiver<notify::Result<Event>>,
    _watcher: RecommendedWatcher,
    dirs: Vec<PathBuf>,
}

impl SkillWatcher {
    pub fn new(dirs: &[PathBuf]) -> Self {
        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .expect("failed to create file watcher");

        let mut watched_dirs = Vec::new();
        for dir in dirs {
            if dir.is_dir() {
                if let Err(e) = watcher.watch(dir.as_ref(), RecursiveMode::Recursive) {
                    eprintln!("warning: failed to watch {}: {}", dir.display(), e);
                } else {
                    watched_dirs.push(dir.clone());
                }
            }
        }

        Self {
            rx,
            _watcher: watcher,
            dirs: watched_dirs,
        }
    }

    /// Non-blocking check for skill file changes. Returns true if any change was detected.
    pub fn has_updates(&self) -> bool {
        // Drain all pending events, return true if any were skill-related
        while let Ok(event) = self.rx.try_recv() {
            if let Ok(event) = event {
                if is_skill_event(&event) {
                    // Drain remaining events
                    while self.rx.try_recv().is_ok() {}
                    return true;
                }
            }
        }
        false
    }

    pub fn reload(&self) -> Vec<Skill> {
        Skill::load_dirs(&self.dirs)
    }
}

fn is_skill_event(event: &Event) -> bool {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return false,
    }

    event.paths.iter().any(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext == "alisp" || ext == "json")
    })
}
