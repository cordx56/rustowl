//! Generation-tagged publication of analysis results.

use super::backend::Snapshot;
use crate::models::Workspace;
use arc_swap::ArcSwap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The published analysis view, tagged with the generation it belongs to.
///
/// A run stamps its results with the generation it started in; results from a
/// superseded generation are dropped on arrival, which closes the window
/// in-flight cancellation cannot. The view is published by an atomic pointer
/// swap, so a reader traverses it without a lock.
pub struct Results {
    generation: AtomicU64,
    view: ArcSwap<Published>,
}

/// The published view behind a pointer: cloning it is a refcount bump, so an
/// update that changes nothing hands the current view back without copying.
#[derive(Clone)]
struct Published(Arc<Snapshot>);

impl Default for Results {
    fn default() -> Self {
        Self::new()
    }
}

impl Results {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            view: ArcSwap::from_pointee(Published(Arc::new(Snapshot::empty()))),
        }
    }

    /// Bump the generation: runs started earlier can no longer contribute. The
    /// published view is kept, so results for unchanged files stay valid.
    pub fn start_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Publish one analysed workspace, unless its run has been superseded.
    pub fn accept(&self, generation: u64, target: &str, ws: &Workspace) -> bool {
        let mut accepted = false;
        self.view.rcu(|current| {
            // The generation is read inside the update: a result whose run
            // was superseded while the update was in flight is dropped.
            if generation != self.generation.load(Ordering::Acquire) {
                return Published(Arc::clone(&current.0));
            }
            match current.0.merged(target, ws) {
                Some(merged) => {
                    accepted = true;
                    Published(Arc::new(merged))
                }
                None => Published(Arc::clone(&current.0)),
            }
        });
        accepted
    }

    /// An edit supersedes in-flight runs and the edited file's results; other
    /// files stay published.
    pub fn invalidate_file(&self, path: &Path) {
        self.start_generation();
        let wanted = path.to_string_lossy();
        self.view
            .rcu(|current| match current.0.without_file(&wanted) {
                Some(snapshot) => Published(Arc::new(snapshot)),
                None => Published(Arc::clone(&current.0)),
            });
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        Arc::clone(&self.view.load().0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Crate, File, Function, Workspace};
    use archery::{ArcK, SharedPointer};
    use std::path::Path;
    use std::sync::Arc;

    fn function(fn_id: u32) -> SharedPointer<Function, ArcK> {
        SharedPointer::new(Function {
            fn_id,
            name: Arc::from(format!("f{fn_id}")),
            basic_blocks: Arc::from(Vec::new()),
            decls: Vec::new(),
        })
    }

    fn delivery(path: &str, item: SharedPointer<Function, ArcK>) -> Workspace {
        let key: Arc<str> = Arc::from(path);
        let file: SharedPointer<File, ArcK> = SharedPointer::new(File::new().push(item));
        let krate: SharedPointer<Crate, ArcK> =
            SharedPointer::new(Crate([(key, file)].into_iter().collect()));
        let name: Arc<str> = Arc::from("krate");
        Workspace([(name, krate)].into_iter().collect())
    }

    /// The generation is checked inside the atomic update, not only before
    /// it: a result arriving during an invalidation must not resurrect the
    /// stale body.
    #[test]
    fn an_invalidation_wins_over_a_concurrent_delivery() {
        let results = Results::new();
        let generation = results.start_generation();
        let stale = delivery("/ws/src/lib.rs", function(1));

        std::thread::scope(|scope| {
            scope.spawn(|| results.invalidate_file(Path::new("/ws/src/lib.rs")));
            results.accept(generation, "/ws", &stale);
        });

        assert!(
            results
                .snapshot()
                .files_of(Path::new("/ws/src/lib.rs"))
                .is_empty(),
            "the invalidated file must not come back"
        );
    }

    /// The atomic update retries against the current value: two roots
    /// delivering at once must both be present.
    #[test]
    fn concurrent_deliveries_do_not_lose_a_root() {
        let results = Results::new();
        let generation = results.start_generation();

        std::thread::scope(|scope| {
            scope.spawn(|| {
                results.accept(
                    generation,
                    "/ws-a",
                    &delivery("/ws-a/src/lib.rs", function(1)),
                );
            });
            scope.spawn(|| {
                results.accept(
                    generation,
                    "/ws-b",
                    &delivery("/ws-b/src/lib.rs", function(2)),
                );
            });
        });

        let snapshot = results.snapshot();
        assert_eq!(snapshot.files_of(Path::new("/ws-a/src/lib.rs")).len(), 1);
        assert_eq!(snapshot.files_of(Path::new("/ws-b/src/lib.rs")).len(), 1);
    }

    #[test]
    fn a_result_is_published_as_it_arrives() {
        let results = Results::new();
        let generation = results.start_generation();

        assert!(results.accept(generation, "/ws", &delivery("/ws/src/lib.rs", function(1))));
        assert_eq!(
            results
                .snapshot()
                .files_of(Path::new("/ws/src/lib.rs"))
                .len(),
            1
        );
    }

    #[test]
    fn a_superseded_result_is_not_published() {
        let results = Results::new();
        let stale = results.start_generation();
        let fresh = results.start_generation();

        assert!(!results.accept(stale, "/ws", &delivery("/ws/src/lib.rs", function(1))));
        assert!(!results.snapshot().has_files());

        assert!(results.accept(fresh, "/ws", &delivery("/ws/src/lib.rs", function(1))));
        assert!(results.snapshot().has_files());
    }

    #[test]
    fn an_edit_drops_only_the_changed_file() {
        let results = Results::new();
        let generation = results.start_generation();
        results.accept(generation, "/ws", &delivery("/ws/src/a.rs", function(1)));
        results.accept(generation, "/ws", &delivery("/ws/src/b.rs", function(2)));

        results.invalidate_file(Path::new("/ws/src/a.rs"));

        let snapshot = results.snapshot();
        assert!(snapshot.files_of(Path::new("/ws/src/a.rs")).is_empty());
        assert_eq!(snapshot.files_of(Path::new("/ws/src/b.rs")).len(), 1);
    }

    #[test]
    fn an_edit_supersedes_in_flight_runs() {
        let results = Results::new();
        let generation = results.start_generation();
        results.invalidate_file(Path::new("/ws/src/a.rs"));

        assert!(!results.accept(generation, "/ws", &delivery("/ws/src/a.rs", function(1))));
        assert!(
            results
                .snapshot()
                .files_of(Path::new("/ws/src/a.rs"))
                .is_empty()
        );
    }

    #[test]
    fn two_roots_with_the_same_file_name_do_not_collide() {
        let results = Results::new();
        let generation = results.start_generation();
        results.accept(
            generation,
            "/ws",
            &delivery("/ws/sub/src/lib.rs", function(1)),
        );
        results.accept(
            generation,
            "/ws/sub",
            &delivery("/ws/sub/src/lib.rs", function(2)),
        );

        assert_eq!(
            results
                .snapshot()
                .files_of(Path::new("/ws/sub/src/lib.rs"))
                .len(),
            2
        );
    }
}
