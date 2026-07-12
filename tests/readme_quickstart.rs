//! Integration test that pins the README "Quick Start" example.
//!
//! If this test drifts from README.md, the README's claims are no longer
//! backed by CI. The snippet is reproduced verbatim (including the
//! `LoadBalancer` import) and then extended only enough to exercise that
//! import, so clippy `-D warnings` over `--all-targets` cannot flag an
//! unused import.

use ternary_scheduler::{LoadBalancer, Scheduler, Task, WorkStealingPool};

#[test]
fn readme_quickstart_holds() {
    let mut sched = Scheduler::new();
    sched.add_task(Task::new(1, -1)); // deferred
    sched.add_task(Task::new(2, 1).with_deadline(50)); // urgent, deadline 50
    sched.add_task(Task::new(3, 0).with_deadline(100)); // normal, deadline 100

    let order: Vec<usize> = sched.schedule().iter().map(|t| t.id).collect();
    assert_eq!(order, vec![2, 3, 1]); // urgent first

    // Work-stealing pool
    let mut pool = WorkStealingPool::new(3);
    pool.assign(0, Task::new(1, 0));
    pool.assign(0, Task::new(2, 0));
    pool.assign(0, Task::new(3, 0));
    let stolen = pool.steal();
    assert!(stolen > 0);

    // Exercise the LoadBalancer import the README advertises. After the
    // single steal above the pool ([2, 1, 0]) already satisfies the +/-1
    // rule, so balance() has no work to do -- but it must still leave the
    // pool balanced and must not drop any tasks.
    let moves = LoadBalancer::balance(&mut pool);
    assert_eq!(moves, 0);
    assert!(LoadBalancer::is_balanced(&pool.workers));
    let total: usize = pool.workers.iter().map(|w| w.len()).sum();
    assert_eq!(total, 3);
}
