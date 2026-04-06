// Autonomous task queue — port of internal/agent/autonomous.go
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskStep {
    pub command: String,
    pub status: TaskStatus,
    pub output: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub steps: Vec<TaskStep>,
    pub status: TaskStatus,
    pub created_at: i64,
    pub progress: f32,
}

/// Thread-safe task queue with bounded concurrency.
pub struct TaskQueue {
    tasks: Arc<DashMap<String, Task>>,
    max_concurrent: u32,
    active_count: Arc<AtomicU32>,
}

impl TaskQueue {
    pub fn new(max_concurrent: u32) -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
            max_concurrent,
            active_count: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Enqueue a new task. Returns error if a task with the same id exists.
    pub fn enqueue(&self, mut task: Task) -> anyhow::Result<()> {
        if self.tasks.contains_key(&task.id) {
            anyhow::bail!("task {} already exists", task.id);
        }
        task.status = TaskStatus::Queued;
        task.created_at = task.created_at.max(1); // ensure non-zero
        self.tasks.insert(task.id.clone(), task);
        Ok(())
    }

    /// Cancel a queued or running task.
    pub fn cancel(&self, id: &str) -> bool {
        if let Some(mut task) = self.tasks.get_mut(id) {
            if task.status == TaskStatus::Queued || task.status == TaskStatus::Running {
                task.status = TaskStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// List all tasks.
    pub fn list(&self) -> Vec<Task> {
        self.tasks.iter().map(|r| r.value().clone()).collect()
    }

    /// Get a task by id.
    pub fn get(&self, id: &str) -> Option<Task> {
        self.tasks.get(id).map(|r| r.value().clone())
    }

    /// Pick the next queued task if under concurrency limit.
    /// Returns the task id on success.
    pub fn run_next(&self) -> Option<String> {
        if self.active_count.load(Ordering::Relaxed) >= self.max_concurrent {
            return None;
        }
        for mut entry in self.tasks.iter_mut() {
            if entry.value().status == TaskStatus::Queued {
                entry.value_mut().status = TaskStatus::Running;
                self.active_count.fetch_add(1, Ordering::Relaxed);
                return Some(entry.key().clone());
            }
        }
        None
    }

    /// Mark a task as completed and decrement active count.
    pub fn complete(&self, id: &str, status: TaskStatus) {
        if let Some(mut task) = self.tasks.get_mut(id) {
            task.status = status;
        }
        self.active_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Current number of active (running) tasks.
    pub fn active_count(&self) -> u32 {
        self.active_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: &str) -> Task {
        Task {
            id: id.into(),
            description: format!("task {id}"),
            steps: vec![TaskStep { command: "echo hi".into(), status: TaskStatus::Queued, output: None }],
            status: TaskStatus::Queued,
            created_at: 1,
            progress: 0.0,
        }
    }

    #[test]
    fn test_enqueue_and_list() {
        let q = TaskQueue::new(4);
        q.enqueue(make_task("t1")).unwrap();
        q.enqueue(make_task("t2")).unwrap();
        assert_eq!(q.list().len(), 2);
    }

    #[test]
    fn test_duplicate_enqueue_fails() {
        let q = TaskQueue::new(4);
        q.enqueue(make_task("t1")).unwrap();
        assert!(q.enqueue(make_task("t1")).is_err());
    }

    #[test]
    fn test_cancel_queued() {
        let q = TaskQueue::new(4);
        q.enqueue(make_task("t1")).unwrap();
        assert!(q.cancel("t1"));
        assert_eq!(q.get("t1").unwrap().status, TaskStatus::Cancelled);
    }

    #[test]
    fn test_cancel_running() {
        let q = TaskQueue::new(4);
        q.enqueue(make_task("t1")).unwrap();
        q.run_next().unwrap();
        assert!(q.cancel("t1"));
        assert_eq!(q.get("t1").unwrap().status, TaskStatus::Cancelled);
    }

    #[test]
    fn test_cancel_completed_fails() {
        let q = TaskQueue::new(4);
        q.enqueue(make_task("t1")).unwrap();
        q.run_next().unwrap();
        q.complete("t1", TaskStatus::Completed);
        assert!(!q.cancel("t1"));
    }

    #[test]
    fn test_run_next_respects_limit() {
        let q = TaskQueue::new(2);
        q.enqueue(make_task("t1")).unwrap();
        q.enqueue(make_task("t2")).unwrap();
        q.enqueue(make_task("t3")).unwrap();
        let first = q.run_next();
        assert!(first.is_some());
        let second = q.run_next();
        assert!(second.is_some());
        assert_ne!(first, second);
        assert_eq!(q.run_next(), None); // limit reached
        assert_eq!(q.active_count(), 2);
    }

    #[test]
    fn test_complete_decrements_active() {
        let q = TaskQueue::new(4);
        q.enqueue(make_task("t1")).unwrap();
        q.run_next().unwrap();
        assert_eq!(q.active_count(), 1);
        q.complete("t1", TaskStatus::Completed);
        assert_eq!(q.active_count(), 0);
        assert_eq!(q.get("t1").unwrap().status, TaskStatus::Completed);
    }

    #[test]
    fn test_status_transitions() {
        let q = TaskQueue::new(4);
        q.enqueue(make_task("t1")).unwrap();
        assert_eq!(q.get("t1").unwrap().status, TaskStatus::Queued);

        q.run_next().unwrap();
        assert_eq!(q.get("t1").unwrap().status, TaskStatus::Running);

        q.complete("t1", TaskStatus::Failed);
        assert_eq!(q.get("t1").unwrap().status, TaskStatus::Failed);
    }

    #[test]
    fn test_get_nonexistent() {
        let q = TaskQueue::new(4);
        assert!(q.get("nope").is_none());
    }
}
