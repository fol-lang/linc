# Linux Code-Driven Examples

This directory carries Linux-focused, code-driven integration examples for `bic`.

These examples are intentionally library-first:

- no sidecar config files
- no CLI assumptions
- no generated manifests

The goal is to show what a downstream consumer such as `fol` would construct directly in Rust.

Current examples:

- `socketcan.rs`: analyze the Linux SocketCAN headers, attach explicit Linux/link metadata, and
  request ABI-sensitive layout probes entirely from code
- `epoll.rs`: analyze `sys/epoll.h`, attach Linux/libc metadata, and request a layout probe for
  `struct epoll_event`
- `linux_event_loop.rs`: combine `epoll`, `timerfd`, and `signalfd` headers into one code-driven
  event-loop-style system example

## Prerequisites By Example

These examples are intentionally code-driven, but some still depend on host Linux headers or
runtime availability.

| Example | Default path | Host prerequisites | Extra opt-in requirement |
|---|---|---|---|
| `epoll.rs` | analysis only | Linux target, libc headers, and either a host `sys/epoll.h` or the repo fixture fallback when present | none |
| `linux_event_loop.rs` | analysis only | Linux target plus host `sys/epoll.h`, `sys/timerfd.h`, and `sys/signalfd.h` headers | none |
| `socketcan.rs` analysis | analysis only | Linux target plus host `linux/can.h` and `linux/can/raw.h` headers | none |
| `socketcan.rs` runtime smoke | ignored by default | same as analysis path | set `BIC_RUN_SYSTEM_SOCKETCAN=1` and run on a host/kernel that exposes the SocketCAN socket boundary |

Practical package expectations on Ubuntu-like systems:

- libc development headers
- Linux libc/kernel headers
- for the SocketCAN runtime smoke, a kernel/runtime that accepts `socket(AF_CAN, SOCK_RAW, CAN_RAW)`
  or at least fails with one of the expected "not supported here" errno values

## SocketCAN Runtime Boundary

SocketCAN is intentionally useful here because it is not a standalone library boundary.

What comes from header analysis:

- constants and macros from `linux/can.h` and `linux/can/raw.h`
- record layouts such as `struct can_frame`, `struct canfd_frame`, and `struct sockaddr_can`
- Linux-only native metadata such as the `linux` platform constraint and the explicit `c` link
  requirement attached in code

What comes from runtime behavior:

- actual socket creation happens through the libc/kernel entry point
  `socket(AF_CAN, SOCK_RAW, CAN_RAW)`
- success or expected kernel errors such as unsupported-address-family results are runtime facts,
  not declaration-extraction facts

The example keeps those layers separate on purpose:

- `analyze_socketcan()` exercises the header-analysis path
- `socketcan_runtime_smoke_check()` exercises the syscall/libc boundary

That split is important for downstream consumers such as `fol`.
They should use `bic` to understand the C surface and native requirements, then apply their own
runtime/build policy on top.

## Linux System Findings

Current Linux system-example findings from `socketcan.rs`, `epoll.rs`, and
`linux_event_loop.rs`:

- code-driven construction works well for Linux system APIs that are primarily header- and
  libc-backed
- host-path discovery still matters because several headers can live either in `/usr/include` or in
  multiarch directories such as `/usr/include/x86_64-linux-gnu`
- layout probing is the most stable part of these examples:
  - `struct can_frame`
  - `struct sockaddr_can`
  - `struct epoll_event`
  - `struct signalfd_siginfo`
- macro capture is useful for system APIs where constants are part of the public contract, such as
  `CAN_EFF_FLAG`
- attaching explicit Linux target constraints and an explicit `c` link requirement keeps the
  resulting package honest about what is platform-specific and what runtime is expected
- `epoll.rs` now has a repo-owned fixture fallback, so its default analysis path is no longer
  purely host-header dependent

Current limitations exposed by these examples:

- `linux_event_loop.rs` and `socketcan.rs` still rely on host-installed Linux headers
- the strongest runtime proof currently exists only for SocketCAN, where the repo explicitly tests
  the `socket(AF_CAN, SOCK_RAW, CAN_RAW)` boundary
- event-loop examples currently prove header and layout consumption, not end-to-end runtime event
  loop behavior

What this means for downstream consumers:

- `bic` is already a good fit for Linux system-header analysis when consumers want code-driven
  inputs
- downstream generators should treat runtime behavior as a separate policy layer instead of assuming
  that a successful scan proves runtime availability

## Planned Torture Target

The synthetic torture target is meant to concentrate difficult C interop constructs into one
header-level surface so `bic` limitations are easier to observe and classify.

The first version is intended to include:

- typedef chains and alias-mediated records
- anonymous nested structs and unions
- bitfields and packed records
- flexible array members
- opaque forward declarations
- function-pointer callbacks
- variadic functions
- macro constants and ABI-affecting configuration macros
- one or more intentionally unsupported declarations

The purpose is not realism.
The purpose is to force one scan to answer:

- what extracted cleanly
- what extracted partially with diagnostics
- what was represented as unsupported
- what can be layout-probed with high confidence

## First Torture Findings

Current first-pass findings from [c_interop_torture.h](/home/bresilla/data/code/bresilla/bic/test/linus/c_interop_torture.h):

- the header preprocesses cleanly through `HeaderConfig`
- the public declarations remain visible in `PreprocessingReport.preprocessed_source`
- the first parser-hostile typedef shape, the packed typedef form
  `typedef struct TORTURE_PACKED torture_packet { ... } torture_packet;`, now recovers into normal
  declaration extraction with retained partial diagnostics
- requested ABI probes still run on this path, so layout evidence is retained alongside recovered
  declarations
- macro capture also survives this path, so the package still retains evidence such as
  `TORTURE_API_LEVEL`
- the next most realistic parser-hostile shape to target is the aligned typedef form:
  `typedef struct __attribute__((aligned(...))) name { ... } name;`

What this means today:

- `bic` can now recover at least one important parser-hostile record typedef form instead of
  collapsing into parse failure
- downstream consumers should still distinguish fully clean extraction from extraction that required
  retained partial diagnostics
- the next improvement area is parser coverage for additional attribute-bearing typedef forms,
  especially aligned record declarations that place attributes between `struct` and the tag name
