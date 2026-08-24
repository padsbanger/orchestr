---
name: crap
description: >
  Analyze code using the Change Risk Anti-Patterns (CRAP) metric. Use this
  skill when reviewing code quality, identifying risky functions, evaluating
  changed code, preparing a refactor, or enforcing a quality gate before
  review/merge. Works across languages by combining cyclomatic complexity
  with test coverage using the best tooling available in the repository.
---

# CRAP Analysis

Use the CRAP metric to identify code that is both difficult to understand
and insufficiently protected by tests.

CRAP stands for **Change Risk Anti-Patterns**.

The metric for a function or method is:

```text
CRAP = CC^2 * (1 - coverage)^3 + CC
```

Where:

- `CC` is cyclomatic complexity.
- `coverage` is test coverage expressed as a value from `0.0` to `1.0`.

Examples:

```text
CC = 2, coverage = 100%
CRAP = 2

CC = 10, coverage = 100%
CRAP = 10

CC = 10, coverage = 50%
CRAP = 22.5

CC = 10, coverage = 0%
CRAP = 110
```

A high score means the code is risky to change because it combines
complex control flow with insufficient automated test protection.

## Core Principle

Do not treat complexity or coverage independently.

Complex code may be acceptable when thoroughly tested.

Simple code may be acceptable with limited coverage.

The primary concern is code that has both:

1. high cyclomatic complexity
2. low test coverage

Use CRAP to find that intersection.

# When to Use This Skill

Use this skill when:

- reviewing a pull request or task
- evaluating code changed by an agent
- deciding what to refactor
- deciding where additional tests are most valuable
- investigating fragile or risky areas of a codebase
- preparing a module for significant changes
- enforcing a pre-merge quality gate
- comparing implementation alternatives
- checking whether a change made maintainability worse

Do not run a full-repository CRAP analysis unnecessarily when the task
only affects a small number of files. Prefer changed-code analysis first.

# Workflow

## 1. Determine Scope

Identify the code affected by the current task.

Prefer this order:

1. changed functions
2. changed files
3. directly related modules
4. entire repository only when explicitly requested or justified

When Git is available, inspect the diff against the repository's normal
base branch.

Typical commands may include:

```bash
git status
git diff
git diff --name-only
git diff main...HEAD
```

Adapt to the repository's branch conventions.

## 2. Detect Existing Tooling

Inspect the repository before installing or introducing anything.

Look for:

- package manager files
- test configuration
- coverage configuration
- linting configuration
- complexity tooling
- CI configuration
- existing CRAP tooling
- project documentation
- AGENTS.md or equivalent agent instructions

Prefer existing project tooling.

Do not introduce a new dependency if existing tools can provide the
required complexity and coverage data.

## 3. Obtain Cyclomatic Complexity

Determine cyclomatic complexity at function or method level.

Use the best available tool for the project language.

Examples of possible sources include:

- dedicated CRAP tools
- static-analysis tools
- linters exposing complexity
- language-specific complexity analyzers
- compiler or ecosystem tooling

The exact tool is implementation-specific.

Do not approximate complexity from file length, nesting depth, or number
of lines when a proper cyclomatic complexity tool is available.

## 4. Obtain Test Coverage

Run the repository's normal test suite with coverage enabled.

Prefer function-level coverage when supported.

If only line or branch coverage is available, use the most precise
coverage measure that can be reliably mapped to the function.

Do not invent coverage values.

If coverage cannot be obtained, report that CRAP cannot be calculated
reliably and explain what data is missing.

## 5. Calculate CRAP

For each relevant function:

```text
CRAP = CC^2 * (1 - coverage)^3 + CC
```

Use coverage as a fraction:

```text
0%   -> 0.00
50%  -> 0.50
80%  -> 0.80
100% -> 1.00
```

Keep enough precision for ranking, but normally report scores rounded to
one or two decimal places.

## 6. Rank Findings

Sort functions by CRAP score descending.

Focus attention on the highest-risk functions first.

A practical interpretation is:

```text
CRAP < 5      low risk
CRAP 5-15     moderate
CRAP 15-30    elevated
CRAP >= 30    high risk
```

These ranges are guidance, not universal laws.

Repository-specific thresholds take precedence.

# Fixing High CRAP Scores

When a function has a high score, determine why.

There are two main ways to reduce CRAP:

## Reduce Complexity

Prefer reducing complexity when the implementation is unnecessarily
difficult to reason about.

Possible improvements:

- split a function by responsibility
- replace deeply nested branching
- remove duplicated conditionals
- extract meaningful domain operations
- simplify boolean expressions
- use early returns where they improve clarity
- replace condition-heavy logic with appropriate data structures
- separate orchestration from computation
- remove dead branches

Do not split functions mechanically just to reduce a metric.

A lower complexity score is useful only when the resulting code is
actually easier to understand and maintain.

## Improve Test Coverage

Add tests when complexity is justified by the domain or when behavior is
insufficiently protected.

Prefer tests that exercise behavior and meaningful branches.

Cover:

- important branches
- boundary conditions
- failure paths
- state transitions
- regression cases
- business rules

Do not add meaningless tests solely to increase a percentage.

# Anti-Gaming Rules

CRAP is a diagnostic metric, not a target to manipulate.

Do not:

- exclude risky files from coverage
- mark code ignored without justification
- write assertions that do not verify behavior
- split functions into arbitrary fragments solely to reduce CC
- delete useful branches to satisfy a threshold
- replace readable code with clever abstractions
- lower repository thresholds simply to make a check pass

Prefer a genuine reduction in change risk.

# Changed-Code Review

When reviewing a task or pull request, compare CRAP before and after when
practical.

Prioritize newly introduced or modified functions.

A change should normally not introduce a new high-CRAP function.

Default quality gate:

```text
No newly introduced or materially modified function should have CRAP >= 30.
```

If a changed function already exceeded the threshold before the task:

- do not automatically require unrelated cleanup
- avoid making the score worse
- report the existing risk
- improve it when reasonably within scope

Never expand a small task into a large refactor solely because unrelated
legacy code has high CRAP scores.

# Language and Tool Independence

This skill describes the analysis, not a specific implementation.

Use repository-native tooling whenever possible.

Possible ecosystems include:

```text
TypeScript / JavaScript
Rust
Go
Python
Java
Kotlin
C#
C / C++
Ruby
PHP
Clojure
Swift
```

If a dedicated CRAP implementation exists and is appropriate for the
repository, prefer it.

Otherwise combine:

1. cyclomatic complexity data
2. test coverage data
3. the CRAP formula

The resulting analysis should be equivalent regardless of language.

# Tool Selection

When multiple tools are available, prefer tools in this order:

1. existing CRAP tool already configured by the repository
2. existing project static-analysis + coverage tooling
3. ecosystem-standard tooling already present in the dependency graph
4. temporary local tooling that does not modify the project
5. adding a development dependency, only when justified

Do not silently modify project dependencies merely to calculate CRAP.

# Reporting

For a normal review, report only actionable findings.

Recommended format:

```text
CRAP Analysis

Highest-risk changed functions:

1. parseConfig
   CRAP: 42.3
   Complexity: 9
   Coverage: 37%
   Risk: high

   Reason:
   Several independent branches are only partially tested.

   Recommendation:
   Add tests around invalid configuration paths, then simplify validation.

2. createTask
   CRAP: 8.1
   Complexity: 6
   Coverage: 82%
   Risk: moderate
```

Avoid dumping hundreds of low-risk functions into the response.

For large repositories, show:

- highest CRAP scores
- new regressions
- threshold violations
- changed functions
- summary statistics

# Review Decision

When CRAP is part of a quality gate, conclude with one of:

```text
PASS
PASS WITH WARNINGS
FAIL
```

Use `FAIL` when changed code violates an explicit repository threshold.

Without an explicit threshold, use the default changed-code threshold of
`CRAP >= 30` as a strong warning rather than automatically blocking work,
unless project instructions say otherwise.

# Before Refactoring

Before modifying a high-CRAP function:

1. understand its current behavior
2. inspect existing tests
3. add characterization tests when behavior is insufficiently protected
4. make small changes
5. rerun tests
6. recalculate CRAP

Do not perform a large structural rewrite of poorly tested high-risk code
without first protecting important behavior.

# Agent Workflow

When implementing code:

```text
implement
    ↓
run tests
    ↓
collect coverage
    ↓
measure complexity
    ↓
calculate CRAP
    ↓
fix newly introduced high-risk code
    ↓
lint / typecheck / normal project checks
    ↓
review
```

When reviewing another agent's work:

1. inspect the diff
2. identify changed functions
3. run the relevant tests
4. collect coverage
5. calculate CRAP for changed code
6. flag regressions
7. distinguish existing technical debt from newly introduced risk
8. return a concise review result

# Completion Checklist

Before declaring CRAP analysis complete, verify:

- [ ] Scope matches the task.
- [ ] Complexity came from an appropriate analyzer.
- [ ] Coverage came from actual tests.
- [ ] Coverage values were converted correctly to fractions.
- [ ] CRAP was calculated at function or method level where possible.
- [ ] Changed functions were prioritized.
- [ ] Existing debt was distinguished from newly introduced risk.
- [ ] High scores include actionable recommendations.
- [ ] No project dependencies were modified unnecessarily.
- [ ] The metric was not gamed merely to pass a threshold.
- [ ] Normal repository tests and quality checks still pass.
