//! Minimal runnable demo for `ternary-scheduler`.
//!
//! Mirrors the Quick Start example in the README: it builds a small
//! scheduler, prints the priority/deadline-ordered execution plan, then
//! shows the work-stealing pool rebalancing an imbalanced worker.

use ternary_scheduler::{LoadBalancer, Scheduler, Task, WorkStealingPool};

fn main() {
    let mut sched = Scheduler::new();
    sched.add_task(Task::new(1, -1)); // deferred
    sched.add_task(Task::new(2, 1).with_deadline(50)); // urgent, deadline 50
    sched.add_task(Task::new(3, 0).with_deadline(100)); // normal, deadline 100

    let order: Vec<usize> = sched.schedule().iter().map(|t| t.id).collect();
    println!("execution order (urgent first): {order:?}");
    assert_eq!(order, vec![2, 3, 1]);

    let mut pool = WorkStealingPool::new(3);
    for i in 0..6 {
        pool.assign(0, Task::new(i, 0));
    }
    let before: Vec<usize> = pool.workers.iter().map(|w| w.len()).collect();
    let moves = LoadBalancer::balance(&mut pool);
    let after: Vec<usize> = pool.workers.iter().map(|w| w.len()).collect();
    println!("load before balance: {before:?}");
    println!("moved {moves} task(s); load after balance: {after:?}");
}
