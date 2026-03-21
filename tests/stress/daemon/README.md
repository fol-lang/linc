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

The next most valuable deepening target is the daemon core object boundary itself:

- it is defined in-repo by `max_pain.c`
- it exports the lifecycle and subsystem entry points already modeled in the header
- it lets `linc` prove one concrete validation path without pretending that optional packet, TLS, or
  runtime-loader deployment dependencies are solved

## Consolidated Findings

Current findings from the code-driven combined target in
[max_pain.rs](/home/bresilla/data/code/bresilla/linc/test/stress/daemon/max_pain.rs):

- the combined surface now scans through normal `HeaderConfig` usage without sidecar files
- mixed records and hooks extract cleanly:
  - `bic_daemon_packet`
  - `bic_daemon_config`
  - `bic_daemon_hooks`
- mixed lifecycle and subsystem functions extract cleanly:
  - `bic_daemon_create`
  - `bic_daemon_submit_packet`
  - `bic_daemon_enable_socketcan`
  - `bic_daemon_enable_pcap`
  - `bic_daemon_enable_tls`
- layout probing is useful for the concrete packet/config records, which means the mixed target still
  preserves ABI evidence for the parts that are passed by value or by pointer to concrete records
- the daemon core object now has a concrete validation path through the checked-in inventory
  fixture, so the mixed target proves more than header parsing alone for:
  - `bic_daemon_create`
  - `bic_daemon_destroy`
  - `bic_daemon_submit_packet`
  - `bic_daemon_enable_socketcan`
  - `bic_daemon_enable_pcap`
  - `bic_daemon_enable_tls`
- opaque handles remain intentionally opaque:
  - `bic_daemon`
  - `bic_tls_client`
- the link surface stays honest about what the fixture actually declares:
  - the combined target carries the host-side `dl` requirement
  - it does not pretend that `pcap`, `curl`, or `OpenSSL` are proven native providers just because
    the fixture surface mentions those subsystems

## Extraction, Validation, and Link-Model Boundary

The current combined target proves three different things at once:

- extraction:
  - `linc` can still extract a useful mixed API surface even when one header mixes event-loop,
    packet, TLS, and plugin concepts
- validation:
  - validation is still a separate step that needs real native artifacts or checked-in artifact
    fixtures
  - the current daemon-core inventory fixture proves the core exported entry points
  - it still does not prove that a deployment artifact exports every optional subsystem dependency
- link-model:
  - the combined target currently models the explicit host/runtime-loader dependency (`dl`)
  - subsystem activation paths such as SocketCAN, packet capture, or TLS remain consumer policy
    until a concrete native bundle is inspected

## Practical Conclusion

The combined daemon target is now useful as a realistic system-level analysis fixture.
It is not yet an end-to-end deployment proof.

That distinction is exactly the point of this target:

- `linc` can describe the mixed C surface and preserve ABI evidence for it
- downstream consumers still need to decide which optional subsystems must be present in a given
  deployment and which validation findings are blocking
