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
        Self {
            id,
            priority,
            deadline: None,
            duration: 1,
            dependencies: Vec::new(),
        }
    }
    pub fn with_deadline(mut self, d: usize) -> Self {
        self.deadline = Some(d);
        self
    }
    pub fn with_duration(mut self, d: usize) -> Self {
        self.duration = d;
        self
    }
    pub fn depends_on(mut self, ids: &[usize]) -> Self {
        self.dependencies = ids.to_vec();
        self
    }
}

#[derive(Clone, Debug)]
pub struct Scheduler {
    pub tasks: HashMap<usize, Task>,
    next_id: usize,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn add_task(&mut self, task: Task) {
        self.next_id = self.next_id.max(task.id + 1);
        self.tasks.insert(task.id, task);
    }

    /// Schedule tasks: +1 first, then 0, then -1. Within priority, earliest deadline first.
    pub fn schedule(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.values().collect();
        tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority) // higher priority first
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
        } else {
            false
        }
    }

    /// Utilization: fraction of ticks that were productive
    pub fn utilization(history: &[Option<usize>]) -> f64 {
        if history.is_empty() {
            return 0.0;
        }
        history.iter().filter(|h| h.is_some()).count() as f64 / history.len() as f64
    }
}

/// Work-stealing scheduler with multiple workers
pub struct WorkStealingPool {
    pub workers: Vec<Vec<Task>>,
}

impl WorkStealingPool {
    pub fn new(n_workers: usize) -> Self {
        Self {
            workers: (0..n_workers).map(|_| Vec::new()).collect(),
        }
    }

    pub fn assign(&mut self, worker: usize, task: Task) {
        if worker < self.workers.len() {
            self.workers[worker].push(task);
        }
    }

    pub fn steal(&mut self) -> usize {
        let mut stolen = 0;
        let n = self.workers.len();
        if n < 2 {
            return 0;
        }

        // Find busiest and idlest
        let (busiest, busiest_load) = self
            .workers
            .iter()
            .enumerate()
            .map(|(i, w)| (i, w.len()))
            .max_by_key(|&(_, l)| l)
            .unwrap_or((0, 0));
        let (idlest, idlest_load) = self
            .workers
            .iter()
            .enumerate()
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
        if workers.is_empty() {
            return true;
        }
        let total: usize = workers.iter().map(|w| w.len()).sum();
        let avg = total as f64 / workers.len() as f64;
        workers.iter().all(|w| (w.len() as f64 - avg).abs() <= 1.0)
    }

    /// Redistribute tasks to balance load.
    ///
    /// Repeatedly steals until `is_balanced` holds or no further steal is
    /// possible (for example when the only excess tasks on the busiest
    /// worker are urgent and therefore non-stealable).
    ///
    /// This terminates: each successful steal moves one task from the
    /// busiest worker to the idlest, strictly decreasing the sum of squared
    /// loads (a non-negative integer potential), so progress is impossible
    /// to sustain indefinitely. The `stolen == 0` guard also breaks the
    /// loop if `steal` ever cannot make a move.
    pub fn balance(pool: &mut WorkStealingPool) -> usize {
        let mut moves = 0;
        while !Self::is_balanced(&pool.workers) {
            let stolen = pool.steal();
            if stolen == 0 {
                break;
            }
            moves += stolen;
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
        // The load imbalance must be large enough that steal() actually
        // attempts a steal (busiest_load > idlest_load + 1). With a
        // [2, 1] split the guard `2 > 1 + 1` is false and the test would
        // pass for the wrong reason. Here worker 0 holds three urgent
        // tasks and worker 1 holds none, so the only thing preventing a
        // steal is the urgent-task protection.
        let mut pool = WorkStealingPool::new(2);
        pool.assign(0, Task::new(1, 1)); // urgent
        pool.assign(0, Task::new(2, 1)); // urgent
        pool.assign(0, Task::new(3, 1)); // urgent
        let stolen = pool.steal();
        // Urgent (+1) tasks must never be stolen even when heavily
        // imbalanced.
        assert_eq!(stolen, 0);
        assert_eq!(pool.workers[0].len(), 3);
        assert_eq!(pool.workers[1].len(), 0);
    }

    #[test]
    fn test_load_balanced() {
        let pool = WorkStealingPool {
            workers: vec![vec![Task::new(1, 0)], vec![Task::new(2, 0)]],
        };
        assert!(LoadBalancer::is_balanced(&pool.workers));
    }

    // Regression: balance() previously ran only W steal rounds, which for a
    // heavily overloaded single worker left the pool unbalanced (e.g. a
    // [10, 0, 0] start converged to [7, 2, 1] and stopped). balance() must
    // keep stealing until is_balanced() holds.
    #[test]
    fn test_balance_actually_balances() {
        let mut pool = WorkStealingPool::new(3);
        for i in 0..10 {
            pool.assign(0, Task::new(i, 0));
        }
        let moves = LoadBalancer::balance(&mut pool);
        assert!(moves > 0, "balance should have moved tasks");
        assert!(
            LoadBalancer::is_balanced(&pool.workers),
            "pool not balanced after balance(): {:?}",
            pool.workers.iter().map(|w| w.len()).collect::<Vec<_>>()
        );
        // No tasks were lost.
        let total: usize = pool.workers.iter().map(|w| w.len()).sum();
        assert_eq!(total, 10);
    }

    // balance() must terminate and report zero moves when the imbalance is
    // caused entirely by non-stealable urgent tasks on the busiest worker.
    #[test]
    fn test_balance_skips_urgent_only_imbalance() {
        let mut pool = WorkStealingPool::new(2);
        // Three urgent (+1) tasks on worker 0, none on worker 1. Urgent
        // tasks must never be stolen, so the pool stays imbalanced but
        // balance() must not loop forever or move anything.
        for i in 0..3 {
            pool.assign(0, Task::new(i, 1));
        }
        let moves = LoadBalancer::balance(&mut pool);
        assert_eq!(moves, 0);
        assert_eq!(pool.workers[0].len(), 3);
        assert_eq!(pool.workers[1].len(), 0);
    }

    // --- schedule() branches ---

    #[test]
    fn test_schedule_empty() {
        let s = Scheduler::new();
        assert!(s.schedule().is_empty());
    }

    // Same-priority tasks without deadlines keep a stable relative order
    // and all sort after higher-priority tasks (deadline defaults to MAX).
    #[test]
    fn test_schedule_priority_desc_then_deadline_asc() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0).with_deadline(10));
        s.add_task(Task::new(2, 1));
        s.add_task(Task::new(3, 0).with_deadline(5));
        s.add_task(Task::new(4, -1));
        let order: Vec<usize> = s.schedule().iter().map(|t| t.id).collect();
        assert_eq!(order, vec![2, 3, 1, 4]);
    }

    // --- reschedule() branches ---

    // Tasks without a deadline are never escalated.
    #[test]
    fn test_reschedule_no_deadline_not_escalated() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0).with_duration(3));
        assert_eq!(s.reschedule(0), 0);
        assert_eq!(s.tasks[&1].priority, 0);
    }

    // Only priority==0 tasks escalate; deferred and already-urgent tasks
    // do not, even when their deadline is imminent.
    #[test]
    fn test_reschedule_skips_non_normal_priority() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, -1).with_deadline(10).with_duration(3));
        s.add_task(Task::new(2, 1).with_deadline(10).with_duration(3));
        assert_eq!(s.reschedule(9), 0);
        assert_eq!(s.tasks[&1].priority, -1);
        assert_eq!(s.tasks[&2].priority, 1);
    }

    // priority==0 with plenty of lead time is left alone.
    #[test]
    fn test_reschedule_no_escalation_when_far_from_deadline() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0).with_deadline(100).with_duration(3));
        // remaining = 95, duration*2 = 6 -> no escalation
        assert_eq!(s.reschedule(5), 0);
        assert_eq!(s.tasks[&1].priority, 0);
    }

    // Escalation is idempotent: after 0 -> +1, a second pass does nothing.
    #[test]
    fn test_reschedule_idempotent() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0).with_deadline(10).with_duration(3));
        assert_eq!(s.reschedule(5), 1);
        assert_eq!(s.reschedule(5), 0);
        assert_eq!(s.tasks[&1].priority, 1);
    }

    // A task already past its deadline (remaining saturates to 0) still
    // escalates, since 0 <= duration*2.
    #[test]
    fn test_reschedule_escalates_past_deadline() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0).with_deadline(5).with_duration(2));
        assert_eq!(s.reschedule(100), 1);
        assert_eq!(s.tasks[&1].priority, 1);
    }

    // --- defer() error path ---

    #[test]
    fn test_defer_missing_task_returns_false() {
        let mut s = Scheduler::new();
        s.add_task(Task::new(1, 0));
        assert!(!s.defer(999, 10));
        assert_eq!(s.tasks[&1].priority, 0);
    }

    // --- utilization() edge cases ---

    #[test]
    fn test_utilization_empty_is_zero() {
        assert_eq!(Scheduler::utilization(&[]), 0.0);
    }

    #[test]
    fn test_utilization_all_idle_is_zero() {
        let history: Vec<Option<usize>> = vec![None, None, None];
        assert_eq!(Scheduler::utilization(&history), 0.0);
    }

    #[test]
    fn test_utilization_all_busy_is_one() {
        let history: Vec<Option<usize>> = vec![Some(1), Some(2), Some(3)];
        assert!((Scheduler::utilization(&history) - 1.0).abs() < 1e-10);
    }

    // --- WorkStealingPool branches ---

    #[test]
    fn test_steal_single_worker_is_noop() {
        let mut pool = WorkStealingPool::new(1);
        pool.assign(0, Task::new(1, 0));
        pool.assign(0, Task::new(2, 0));
        assert_eq!(pool.steal(), 0);
        assert_eq!(pool.workers[0].len(), 2);
    }

    #[test]
    fn test_steal_balanced_pool_moves_nothing() {
        let mut pool = WorkStealingPool::new(2);
        pool.assign(0, Task::new(1, 0));
        pool.assign(0, Task::new(2, 0));
        pool.assign(1, Task::new(3, 0));
        pool.assign(1, Task::new(4, 0));
        // loads [2, 2]: busiest_load (2) is not > idlest_load (2) + 1.
        assert_eq!(pool.steal(), 0);
    }

    // assign() to a non-existent worker index is a no-op (task dropped),
    // never a panic / index-out-of-bounds.
    #[test]
    fn test_assign_out_of_bounds_is_noop() {
        let mut pool = WorkStealingPool::new(2);
        pool.assign(5, Task::new(1, 0));
        assert!(pool.workers.iter().all(|w| w.is_empty()));
    }

    // --- LoadBalancer::is_balanced branches ---

    #[test]
    fn test_is_balanced_empty_is_true() {
        assert!(LoadBalancer::is_balanced(&[]));
    }

    #[test]
    fn test_is_balanced_detects_imbalance() {
        let workers = vec![
            vec![Task::new(1, 0), Task::new(2, 0), Task::new(3, 0)],
            vec![],
        ];
        assert!(!LoadBalancer::is_balanced(&workers));
    }

    // --- Default impl ---

    #[test]
    fn test_scheduler_default() {
        let s = Scheduler::default();
        assert!(s.tasks.is_empty());
        assert!(s.schedule().is_empty());
    }
}
