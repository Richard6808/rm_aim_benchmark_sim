# Benchmark Semantics

## Sweep

The default sweep is:

```text
distance:          3, 5, 7, 10 m
RPM:               0, 30, 60, 120, 180
translation speed: 0, 1, 2, 3 m/s
```

One repeat therefore has 80 trials.

## Trial lifecycle

### Warmup

- reset target pose, motion phase, HP, gimbal state, score, and old projectiles;
- apply the new distance/RPM/speed condition;
- target already moves during warmup;
- external auto-aim receives camera/telemetry and can converge;
- projectile firing is blocked from being accepted by the benchmark.

### Running

- external auto-aim fully controls yaw/pitch/fire;
- each actually spawned projectile increments `shots`;
- each shot can score at most one armor hit;
- trial leaves Running when:
  - `rounds_per_trial` actual shots have been generated, or
  - `case_timeout_s` expires.

A fire command alone does not count as a shot.
This makes planner/fire-gate behavior part of the result.

### Drain

- no new benchmark shots are accepted;
- already airborne projectiles can continue and score;
- after `post_fire_grace_s`, the trial is finalized.

This avoids incorrectly marking the final long-distance shots as misses simply because they were still in flight.

## Metrics

### Hit rate

```text
hits / actual shots × 100%
```

### Effective DPS

```text
total scored damage / evaluation duration
```

The duration extends through Drain, so flight time needed for late hits is included.

### Peak rolling DPS

Maximum damage-per-second value observed over the configured rolling window.

### Kill time

Time from Running start until HP first reaches zero.
Only successful kills have a kill-time value.

### Kill success rate

Across repeated trials of one condition:

```text
successful kills / trials × 100%
```

Always consider this alongside mean kill time. Otherwise a method that kills only a few easy trials could have an apparently good mean TTK.

### Timeout rate

Fraction of trials where the external auto-aim failed to produce the requested actual-shot budget before `case_timeout_s`.
This intentionally exposes overly conservative or broken fire logic.

## RPM degradation

When a 0-RPM baseline exists:

```text
degradation(%) = (baseline - current) / baseline × 100
```

The file `rpm_degradation.csv` applies this to hit rate and mean DPS.

## Recommended regression workflow

```text
algorithm commit A
  ↓
3 repeats or more
  ↓
archive benchmark_results/run_A

algorithm commit B
  ↓
same effective_config.toml
  ↓
archive benchmark_results/run_B

compare condition-level and RPM curves
```

For serious comparisons, keep simulation config identical and use more than one repeat per condition.


## Operator inputs during Benchmark

By default `operator.benchmark_auto_hold_inputs = true`. The runner emulates RMB+LMB so no human input is required, but a projectile still requires a fresh external command with `fire=true`. This keeps planner/fire-gate behavior inside the benchmark.

Manual WASD chassis motion is disabled while the runner is active. The shooter root is reset to the world origin at the start of every trial so configured target distance remains reproducible.
