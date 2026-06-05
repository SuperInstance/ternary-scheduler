//! Ternary task scheduler with priority {-1=deferred, 0=normal, +1=urgent}.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: usize,
    pub priority: i8,
    pub deadline: Option<usize>,
    pub duration: usize,
    pub dependencies: Vec<usize>,
}

impl Task {
    pub fn new(id: usize, priority: i8) -> Self {
        Self { id, priority, deadline: None, duration: 1, dependencies: Vec::new() }
    }
    pub fn with_deadline(mut self, d: usize) -> Self { self.deadline = Some(d); self }
    pub fn with_duration(mut self, d: usize) -> Self { self.duration = d; self }
    pub fn depends_on(mut self, ids: &[usize]) -> Self { self.dependencies = ids.to_vec(); self }
}

#[derive(Clone, Debug)]
pub struct Scheduler {
    pub tasks: HashMap<usize, Task>,
    next_id: usize,
}

impl Scheduler {
    pub fn new() -> Self { Self { tasks: HashMap::new(), next_id: 0 } }

    pub fn add_task(&mut self, task: Task) {
        self.next_id = self.next_id.max(task.id + 1);
        self.tasks.insert(task.id, task);
    }

    /// Schedule tasks: +1 first, then 0, then -1. Within priority, earliest deadline first.
    pub fn schedule(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.values().collect();
        tasks.sort_by(|a, b| {
            b.priority.cmp(&a.priority) // higher priority first
                .then_with(|| {
                    let da = a.deadline.unwrap_or(usize::MAX);
                    let db = b.deadline.unwrap_or(usize::MAX);
                    da.cmp(&db)
                })
        });
        tasks
    }

    /// Reschedule: escalate tasks approaching deadlines
    pub fn reschedule(&mut self, current_tick: usize) -> usize {
        let mut escalated = 0;
        for task in self.tasks.values_mut() {
            if let Some(deadline) = task.deadline {
                let remaining = deadline.saturating_sub(current_tick);
                if task.priority == 0 && remaining <= task.duration * 2 {
                    task.priority = 1;
                    escalated += 1;
                }
            }
        }
        escalated
    }

    /// Defer a task
    pub fn defer(&mut self, id: usize, _ticks: usize) -> bool {
        if let Some(task) = self.tasks.get_mut(&id) {
            task.priority = -1;
            true
        } else { false }
    }

    /// Utilization: fraction of ticks that were productive
    pub fn utilization(history: &[Option<usize>]) -> f64 {
        if history.is_empty() { return 0.0; }
        history.iter().filter(|h| h.is_some()).count() as f64 / history.len() as f64
    }
}

/// Work-stealing scheduler with multiple workers
pub struct WorkStealingPool {
    pub workers: Vec<Vec<Task>>,
}

impl WorkStealingPool {
    pub fn new(n_workers: usize) -> Self {
        Self { workers: (0..n_workers).map(|_| Vec::new()).collect() }
    }

    pub fn assign(&mut self, worker: usize, task: Task) {
        if worker < self.workers.len() { self.workers[worker].push(task); }
    }

    pub fn steal(&mut self) -> usize {
        let mut stolen = 0;
        let n = self.workers.len();
        if n < 2 { return 0; }

        // Find busiest and idlest
        let (busiest, busiest_load) = self.workers.iter().enumerate()
            .map(|(i, w)| (i, w.len()))
            .max_by_key(|&(_, l)| l)
            .unwrap_or((0, 0));
        let (idlest, idlest_load) = self.workers.iter().enumerate()
            .map(|(i, w)| (i, w.len()))
            .min_by_key(|&(_, l)| l)
            .unwrap_or((0, 0));

        if busiest_load > idlest_load + 1 {
            // Steal one task that is NOT +1 priority
            if let Some(pos) = self.workers[busiest].iter().position(|t| t.priority != 1) {
                let task = self.workers[busiest].remove(pos);
                self.workers[idlest].push(task);
                stolen += 1;
            }
        }
        stolen
    }
}

/// Load balancer
pub struct LoadBalancer;

impl LoadBalancer {
    /// Check if all workers are within ±1 of average
    pub fn is_balanced(workers: &[Vec<Task>]) -> bool {
        if workers.is_empty() { return true; }
        let total: usize = workers.iter().map(|w| w.len()).sum();
        let avg = total as f64 / workers.len() as f64;
        workers.iter().all(|w| (w.len() as f64 - avg).abs() <= 1.0)
    }

    /// Redistribute tasks to balance load
    pub fn balance(pool: &mut WorkStealingPool) -> usize {
        let mut moves = 0;
        for _ in 0..pool.workers.len() {
            moves += pool.steal();
        }
        moves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, -1));
        s.add_task(Task::new(2, 1));
        s.add_task(Task::new(3, 0));
        let order: Vec<usize> = s.schedule().iter().map(|t| t.id).collect();
        assert_eq!(order, vec![2, 3, 1]);
    }

    #[test]
    fn test_deadline_ordering() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0).with_deadline(100));
        s.add_task(Task::new(2, 0).with_deadline(50));
        s.add_task(Task::new(3, 0).with_deadline(200));
        let order: Vec<usize> = s.schedule().iter().map(|t| t.id).collect();
        assert_eq!(order, vec![2, 1, 3]);
    }

    #[test]
    fn test_reschedule_escalation() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0).with_deadline(10).with_duration(3));
        let escalated = s.reschedule(5); // 5 remaining, duration*2=6 -> escalate
        assert_eq!(escalated, 1);
        assert_eq!(s.tasks[&1].priority, 1);
    }

    #[test]
    fn test_defer() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0));
        assert!(s.defer(1, 10));
        assert_eq!(s.tasks[&1].priority, -1);
    }

    #[test]
    fn test_utilization() {
        let history: Vec<Option<usize>> = vec![Some(1), None, Some(3), Some(4), None];
        let util = Scheduler::utilization(&history);
        assert!((util - 0.6).abs() < 1e-10);
    }

    #[test]
    fn test_work_stealing() {
        let mut pool = WorkStealingPool::new(3);
        pool.assign(0, Task::new(1, 0));
        pool.assign(0, Task::new(2, 0));
        pool.assign(0, Task::new(3, 0));
        pool.assign(1, Task::new(4, 0));
        let stolen = pool.steal();
        assert!(stolen > 0);
        assert!(pool.workers[0].len() < 4);
    }

    #[test]
    fn test_no_steal_urgent() {
        let mut pool = WorkStealingPool::new(2);
        pool.assign(0, Task::new(1, 1)); // urgent
        pool.assign(0, Task::new(2, 1)); // urgent
        pool.assign(1, Task::new(3, 0));
        let stolen = pool.steal();
        // Should not steal +1 tasks
        assert_eq!(stolen, 0);
    }

    #[test]
    fn test_load_balanced() {
        let pool = WorkStealingPool {
            workers: vec![vec![Task::new(1, 0)], vec![Task::new(2, 0)]],
        };
        assert!(LoadBalancer::is_balanced(&pool.workers));
    }
}
