# ternary-scheduler

Ternary task scheduler with **three-level priority classification** `{+1=urgent, 0=normal, -1=deferred}`, deadline-aware rescheduling, work-stealing parallelism, and load balancing across workers.

## Why It Matters

Binary priority queues (high/low) lack granularity for real workloads, and EDF (earliest-deadline-first) ignores semantic priority entirely. Ternary scheduling combines both:

| Priority | Value | Semantics |
|----------|-------|-----------|
| Urgent | `+1` | Deadline-critical, never stolen |
| Normal | `0` | Standard execution; may escalate |
| Deferred | `-1` | Background, best-effort |

The scheduler auto-escalates normal tasks to urgent when deadlines approach, and the work-stealing layer ensures CPU-bound tasks redistribute across workers without moving urgent tasks.

## How It Works

### Priority + Deadline Ordering

Tasks are sorted by a composite key:

```
sort_key = (priority DESC, deadline ASC)
```

Higher priority first; within the same priority, earliest deadline first. Tasks without deadlines sort as `deadline = ∞`.

**Complexity:** O(N log N) per full schedule sort. O(1) per insertion (HashMap).

### Automatic Rescheduling (Escalation)

The `reschedule(current_tick)` method escalates normal tasks approaching deadlines:

```
if priority == 0 and (deadline - current_tick) ≤ 2 · duration:
    priority → +1
```

This gives tasks two duration-units of lead time before their deadline — enough to complete even if the scheduler has a one-tick latency.

**Complexity:** O(N) per reschedule pass.

### Work-Stealing

The `WorkStealingPool` distributes tasks across *W* workers. When a load imbalance is detected (busiest worker has ≥ 2 more tasks than idlest), one task is stolen:

```
if load(busiest) > load(idlest) + 1:
    steal a non-urgent task from busiest → idlest
```

**Critical constraint:** Urgent (`+1`) tasks are **never stolen**. They must execute on their assigned worker to preserve locality and ordering guarantees.

**Complexity:** O(W) per steal attempt (scan for busiest/idlest). O(N) per task to find a stealable candidate.

### Load Balancing

The `LoadBalancer` checks if all workers are within ±1 of the average:

```
balanced ⟺ ∀ w: |load(w) - avg| ≤ 1
```

If unbalanced, it calls `steal()` repeatedly until balanced or no more stealable tasks remain.

**Complexity:** O(W²) worst case for full balancing (W steal rounds × W scan each).

### Utilization Metric

```
utilization = |{ticks with work}| / |all ticks|
```

Tracks the fraction of scheduler ticks that had at least one executed task.

## Quick Start

```rust
use ternary_scheduler::{Scheduler, Task, WorkStealingPool, LoadBalancer};

let mut sched = Scheduler::new();
sched.add_task(Task::new(1, -1));                    // deferred
sched.add_task(Task::new(2, 1).with_deadline(50));   // urgent, deadline 50
sched.add_task(Task::new(3, 0).with_deadline(100));  // normal, deadline 100

let order: Vec<usize> = sched.schedule().iter().map(|t| t.id).collect();
assert_eq!(order, vec![2, 3, 1]); // urgent first

// Work-stealing pool
let mut pool = WorkStealingPool::new(3);
pool.assign(0, Task::new(1, 0));
pool.assign(0, Task::new(2, 0));
pool.assign(0, Task::new(3, 0));
let stolen = pool.steal();
assert!(stolen > 0);
```

## API

### `Scheduler`

| Method | Returns | Description |
|--------|---------|-------------|
| `new()` | `Self` | Empty scheduler |
| `add_task(task)` | `()` | Register a task |
| `schedule()` | `Vec<&Task>` | Get sorted execution order |
| `reschedule(current_tick)` | `usize` | Escalate approaching deadlines |
| `defer(id, ticks)` | `bool` | Set task to deferred |
| `utilization(history)` | `f64` | Static: compute utilization |

### `Task` Builder

```rust
Task::new(id, priority)
    .with_deadline(ticks)
    .with_duration(ticks)
    .depends_on(&[dep_ids])
```

### `WorkStealingPool` / `LoadBalancer`

| Method | Description |
|--------|-------------|
| `assign(worker, task)` | Add task to worker queue |
| `steal()` | Steal one task (non-urgent) from busiest to idlest |
| `LoadBalancer::is_balanced(workers)` | Check ±1 balance |
| `LoadBalancer::balance(pool)` | Repeatedly steal until balanced |

## Architecture Notes

The **γ + η = C** invariant maps cleanly: *generation* (γ) is the task arrival/creation process, *entropy* (η) is the priority distribution `{+1, 0, -1}` across active tasks, and *conservation* (C) is the invariant that total work is preserved — every task is either executing, queued, or completed (never lost). The escalation rule (`0 → +1` near deadline) converts entropy (priority diversity) into generation (urgency-driven execution), maintaining the conservation law that no task misses its deadline without explicit deferral.

## References

- **Priority scheduling:** Liu, C. L. & Layland, J. W. "Scheduling Algorithms for Multiprogramming" (1973)
- **Work-stealing:** Blumofe, R. & Leiserson, C. "Scheduling Multithreaded Computations by Work Stealing" (1999)
- **Earliest-deadline-first:** Stankovic, J. et al. "Implications of Classical Scheduling Results" (1995)
- **Load balancing theory:** Cybenko, G. "Dynamic Load Balancing for Distributed Memory Multiprocessors" (1989)

## License

MIT
