## Migrating from Binary

If you're used to binary scheduling (scheduled / idle), ternary adds a **flexible** state — the $0$ where tasks can wait.

| Binary | Ternary |
|--------|---------|
| Run ($1$) | Priority ($+1$) |
| Idle ($0$) | Optional ($0$) |
| | Defer ($-1$) |

Binary scheduling treats all tasks the same once scheduled. Ternary lets the scheduler know which tasks are flexible — "run if resources available, skip otherwise." This is how real operating systems handle background jobs.

See **[From Binary to Ternary](https://github.com/SuperInstance/ternary-cookbook/blob/master/guides/FROM_BINARY.md)** for the full migration guide.
