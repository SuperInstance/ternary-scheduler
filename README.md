# ternary-scheduler

Task scheduling where every decision reduces to a trivalent judgment: defer, proceed, or escalate.

## Why This Exists

Most schedulers treat priority as a scalar continuum. In practice, what you actually need is a *disposition* toward each task — should it wait, run normally, or drop everything and handle it now? Binary priority (high/low) can't express the ambient middle ground; a float priority introduces tuning parameters nobody calibrates. Three values — `{-1, 0, +1}` — map to real operational semantics: **deferred**, **normal**, **urgent**. That's all you need.

The interesting part is what happens when you add *deadlines* and *work stealing* to a ternary scheduler. Tasks approaching their deadline get automatically escalated. Workers that finish early steal from busier queues — but never steal urgent tasks (those belong where they are). The result is a system that self-balances without a central planner.

## Architecture

```
Task ──► Scheduler ──► schedule() ──► Vec<&Task>  (priority-ordered)
              │
              ├── reschedule(tick) → escalates tasks nearing deadline
              └── defer(id, ticks) → demotes to -1

WorkStealingPool
  ├── workers: Vec<Vec<Task>>
  ├── steal() → moves non-urgent task from busiest to idlest worker
  └── LoadBalancer::balance() → repeated steal until balanced
```

**Key types:**

- **`Task`** — id, priority `{-1, 0, +1}`, optional deadline, duration, dependency list. Builder pattern for construction.
- **`Scheduler`** — priority-ordered task registry. `schedule()` sorts by priority (descending), then deadline (ascending). `reschedule()` escalates normal tasks within `2 × duration` of their deadline.
- **`WorkStealingPool`** — multi-worker scheduler with load-aware stealing. Never steals `+1` (urgent) tasks — those stay on the assigning worker.
- **`LoadBalancer`** — checks if all workers are within ±1 of average load; rebalances via steal.

## Usage

```rust
use ternary_scheduler::{Scheduler, Task, WorkStealingPool, LoadBalancer};

// Create tasks with builder pattern
let urgent = Task::new(1, 1).with_deadline(100).with_duration(5);
let normal = Task::new(2, 0).with_deadline(500).with_duration(10);
let deferred = Task::new(3, -1).depends_on(&[1, 2]);

let mut scheduler = Scheduler::new();
scheduler.add_task(urgent);
scheduler.add_task(normal);
scheduler.add_task(deferred);

// Schedule: +1 first, then 0, then -1. Within priority, earliest deadline first.
let order = scheduler.schedule();
assert_eq!(order[0].id, 1); // urgent
assert_eq!(order[1].id, 2); // normal
assert_eq!(order[2].id, 3); // deferred

// Escalate tasks approaching deadlines
let escalated = scheduler.reschedule(495); // normal task at tick 495, deadline 500, duration 10
// → escalated to +1 if remaining time ≤ 2 × duration

// Multi-worker work stealing
let mut pool = WorkStealingPool::new(4);
pool.assign(0, Task::new(1, 0));
pool.assign(0, Task::new(2, 0));
pool.assign(0, Task::new(3, 0)); // worker 0 is overloaded
pool.assign(1, Task::new(4, 0));

pool.steal(); // moves a task from worker 0 to least-loaded worker

// Check balance
assert!(LoadBalancer::is_balanced(&pool.workers));
LoadBalancer::balance(&mut pool); // rebalance until within ±1

// Measure utilization
let history: Vec<Option<usize>> = vec![Some(1), None, Some(3), Some(4), None];
let utilization = Scheduler::utilization(&history); // 0.6
```

## API Reference

### `Task`

| Method | Description |
|--------|-------------|
| `Task::new(id, priority)` | Create task with id, priority `{-1, 0, +1}`, no deadline, duration 1, no dependencies |
| `.with_deadline(d)` | Set deadline (tick count) |
| `.with_duration(d)` | Set estimated duration in ticks |
| `.depends_on(&[ids])` | Declare dependencies on other task ids |

Fields: `id: usize`, `priority: i8`, `deadline: Option<usize>`, `duration: usize`, `dependencies: Vec<usize>`

### `Scheduler`

| Method | Description |
|--------|-------------|
| `Scheduler::new()` | Empty scheduler |
| `.add_task(task)` | Register a task |
| `.schedule()` | Return tasks sorted: priority desc, then deadline asc |
| `.reschedule(current_tick)` | Escalate normal tasks within `2 × duration` of deadline. Returns count escalated. |
| `.defer(id, ticks)` | Demote task to priority -1. Returns false if task not found. |
| `Scheduler::utilization(history)` | Static: fraction of `Some(_)` entries in tick history |

### `WorkStealingPool`

| Method | Description |
|--------|-------------|
| `WorkStealingPool::new(n_workers)` | Create pool with n workers |
| `.assign(worker, task)` | Assign task to specific worker |
| `.steal()` | Move one non-urgent task from busiest to idlest worker. Returns count stolen. |

### `LoadBalancer`

| Method | Description |
|--------|-------------|
| `LoadBalancer::is_balanced(workers)` | True if all workers within ±1 of average load |
| `LoadBalancer::balance(pool)` | Repeatedly steal until balanced. Returns total moves. |

## The Deeper Idea

Ternary scheduling maps to a fundamental pattern in control systems: **hysteresis**. A task doesn't gradually become urgent — it crosses a threshold and snaps from `0` to `+1`. This discontinuity is intentional. It prevents the scheduler from making marginal decisions that oscillate at the boundary.

The `reschedule()` method implements a soft real-time guarantee: if a task's remaining slack falls below `2 × duration`, it escalates. The factor of 2 is a safety margin — you need at least the duration to execute, and an equal buffer for scheduling overhead, preemption, and dependency resolution.

Work stealing with the "never steal urgent" constraint is a recognition that urgent tasks carry implicit context. They were assigned to a specific worker for a reason — maybe cache warmth, maybe data locality, maybe they're part of a critical path. Stealing them would save load at the cost of correctness.

## Related Crates

- **`ternary-pid`** — PID controller with ternary output, the control-theory dual of this scheduler
- **`ternary-thermostat`** — climate control using the same {-1, 0, +1} state machine
- **`ternary-route`** — routing with ternary health awareness, complementary load balancing
- **`ternary-negotiate`** — multi-agent negotiation where the scheduler becomes a mediator
