# Runbook — contracts

## A consumer breaks after a contract change

1. Check which expand/contract step is in progress (1 to 4).
2. If v1 was removed too early: restore it immediately — that is a rollback, not a fix.
3. Post-mortem: why did the "v1 consumers = 0" check not block?

## An overdue contraction

A deprecated version whose removal date has passed. CI is red, and deliberately so: a
permanent intermediate state is settling in.

Two ways out: finish migrating the last consumer, or push the date back **explicitly**,
with a justification.
