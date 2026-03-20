# Combined Daemon Stress Target

This directory holds the design and fixtures for the “max pain” combined target.

The target is meant to look like a realistic userspace daemon rather than an isolated library API.

## Intended Surface

The combined surface should mix:

- Linux event-loop primitives such as `epoll`, `timerfd`, and `signalfd`
- optional packet inputs such as SocketCAN and packet-capture-style callbacks
- compression-oriented message flow
- HTTPS or TLS-oriented output state
- plugin-style output hooks through a runtime-loaded ABI boundary

## Why this matters

Single-library tests isolate one kind of difficulty.
The daemon target is where those difficulties overlap:

- callback-rich APIs
- macro-heavy settings
- host/runtime separation
- link metadata plus runtime-loaded boundaries
- ABI-sensitive message and descriptor records

## Header Boundary Plan

The combined target will use one public stress header that:

- defines the daemon-facing records and callback contracts
- keeps some handles opaque
- embeds plugin-facing descriptors
- exposes event-loop-facing submission and lifecycle functions
- keeps enough structure to be meaningful without needing a full application implementation

## Expected First Goal

The first goal is not to run the daemon.
The first goal is to make the combined surface scanable and analyzable through pure Rust code, then
record which mixed-surface assumptions hold and which break.
