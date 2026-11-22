use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type FetchedAt = HashMap<String, Instant>;

pub struct FetchGate {
    fresh_for: Duration,
    repos: Mutex<HashMap<String, Arc<tokio::sync::Mutex<FetchedAt>>>>,
}

impl FetchGate {
    pub fn new(fresh_for: Duration) -> Self {
        Self {
            fresh_for,
            repos: Mutex::new(HashMap::new()),
        }
    }

    pub async fn fetch_unless_fresh<T, Fut>(
        &self,
        repo: &str,
        name: &str,
        fetch: impl FnOnce() -> Fut,
    ) -> Option<T>
    where
        Fut: Future<Output = (T, bool)>,
    {
        let turn = {
            let mut repos = self.repos.lock().unwrap_or_else(|e| e.into_inner());
            repos.entry(repo.to_owned()).or_default().clone()
        };
        let mut fetched_at = turn.lock().await;
        fetched_at.retain(|_, at| at.elapsed() < self.fresh_for);
        if fetched_at.contains_key(name) {
            return None;
        }
        let started = Instant::now();
        let (result, fetched) = fetch().await;
        if fetched {
            fetched_at.insert(name.to_owned(), started);
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const WINDOW: Duration = Duration::from_secs(60);

    async fn counted(runs: &AtomicUsize, ok: bool) -> (usize, bool) {
        (runs.fetch_add(1, Ordering::SeqCst) + 1, ok)
    }

    #[tokio::test]
    async fn a_ref_fetched_within_the_window_is_not_fetched_again() {
        let gate = FetchGate::new(WINDOW);
        let runs = AtomicUsize::new(0);
        let first = gate
            .fetch_unless_fresh("r", "main", || counted(&runs, true))
            .await;
        let second = gate
            .fetch_unless_fresh("r", "main", || counted(&runs, true))
            .await;
        assert_eq!(first, Some(1));
        assert_eq!(second, None);
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn another_ref_or_repo_is_fetched() {
        let gate = FetchGate::new(WINDOW);
        let runs = AtomicUsize::new(0);
        gate.fetch_unless_fresh("r", "main", || counted(&runs, true))
            .await;
        let other_ref = gate
            .fetch_unless_fresh("r", "develop", || counted(&runs, true))
            .await;
        let other_repo = gate
            .fetch_unless_fresh("other", "main", || counted(&runs, true))
            .await;
        assert!(other_ref.is_some() && other_repo.is_some());
        assert_eq!(runs.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_failed_fetch_is_not_remembered() {
        let gate = FetchGate::new(WINDOW);
        let runs = AtomicUsize::new(0);
        let failed = gate
            .fetch_unless_fresh("r", "main", || counted(&runs, false))
            .await;
        let retried = gate
            .fetch_unless_fresh("r", "main", || counted(&runs, true))
            .await;
        let skipped = gate
            .fetch_unless_fresh("r", "main", || counted(&runs, true))
            .await;
        assert_eq!((failed, retried, skipped), (Some(1), Some(2), None));
    }

    #[tokio::test]
    async fn a_ref_older_than_the_window_is_fetched_again() {
        let gate = FetchGate::new(Duration::from_millis(40));
        let runs = AtomicUsize::new(0);
        gate.fetch_unless_fresh("r", "main", || counted(&runs, true))
            .await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        let again = gate
            .fetch_unless_fresh("r", "main", || counted(&runs, true))
            .await;
        assert!(again.is_some());
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_second_ask_waits_for_the_running_fetch_and_skips() {
        let gate = Arc::new(FetchGate::new(WINDOW));
        let runs = Arc::new(AtomicUsize::new(0));
        let slow = |runs: Arc<AtomicUsize>| async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            counted(&runs, true).await
        };
        let first = {
            let (gate, runs) = (gate.clone(), runs.clone());
            tokio::spawn(async move { gate.fetch_unless_fresh("r", "main", || slow(runs)).await })
        };
        tokio::time::sleep(Duration::from_millis(30)).await;
        let started = Instant::now();
        let second = gate
            .fetch_unless_fresh("r", "main", || slow(runs.clone()))
            .await;
        assert_eq!(second, None);
        assert!(started.elapsed() >= Duration::from_millis(80));
        assert_eq!(first.await.unwrap(), Some(1));
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetches_into_one_repo_never_overlap() {
        let gate = Arc::new(FetchGate::new(WINDOW));
        let live = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for name in ["main", "develop", "release"] {
            let (gate, live, peak) = (gate.clone(), live.clone(), peak.clone());
            tasks.push(tokio::spawn(async move {
                gate.fetch_unless_fresh("r", name, || async move {
                    let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(40)).await;
                    live.fetch_sub(1, Ordering::SeqCst);
                    ((), true)
                })
                .await
            }));
        }
        for task in tasks {
            assert!(task.await.unwrap().is_some());
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }
}
