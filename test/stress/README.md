# Library Stress Ladder

This directory carries code-driven stress examples for real libraries with increasing difficulty.

The rolling findings ledger for these examples lives in
[FINDINGS.md](/home/bresilla/data/code/bresilla/linc/test/stress/FINDINGS.md).

Current ladder:

- `zlib.rs`: clean baseline library surface with good function and typedef coverage
- `libpcap.rs`: callback-heavy and struct-heavy capture API surface
- `libcurl.rs`: macro volume, enums, callbacks, and option-heavy API surface
- `openssl.rs`: opaque handles, macro pressure, and intentionally incomplete public records

## Reproducibility Classification

The current stress/example surfaces are not equally hermetic.

| Surface | Current classification | Why |
|---|---|---|
| `zlib.rs` | mostly hermetic | vendored fixture path exists and the example does not depend on host system headers for its default useful path |
| `libpcap.rs` | host-dependent | depends on host-installed development headers and whatever prerequisite system typedef visibility those headers pull in |
| `libcurl.rs` | host-dependent | depends on host-installed development headers and macro surface chosen by that host package |
| `openssl.rs` | host-dependent | depends on host-installed development headers and host-side opaque API packaging |
| `plugin.rs` | hermetic | uses repo-owned fixture headers and producer-side metadata only |
| Linux `epoll` / event-loop examples | host-dependent today | depend on host Linux headers and multiarch include discovery |
| Linux `socketcan` analysis | host-dependent today | depends on host Linux SocketCAN headers |
| Linux `socketcan` runtime smoke | host-dependent and opt-in | depends on host kernel/runtime support and `BIC_RUN_SYSTEM_SOCKETCAN` |

## Findings Matrix

| Library | Main stress area | Current confidence | Main note |
|---|---|---|---|
| `zlib` | clean baseline scan and probe path | high | good baseline for functions, typedefs, and one layout-backed record |
| `libpcap` | callbacks and packet-header structs | medium-high | scan path is solid; header-specific probe behavior is more environment-sensitive |
| `libcurl` | macros, enums, option-heavy API, callbacks | medium | scan path is useful, but the most stable retained macros are infrastructure/version macros rather than every option macro a user might expect |
| `OpenSSL` | opaque handles and macro pressure | medium | scan path is useful precisely because it preserves opaque-handle aliases without pretending those records are layout-probable |

## Current Comparative Findings

- `zlib` remains the cleanest real-library baseline and is the best first consumer target.
- `libpcap` immediately exposes host-header subtleties such as prerequisite system typedef visibility.
- `libcurl` shows that “macro-heavy” does not mean “every user-facing option macro survives as a bindable macro”; some retained macros are more infrastructural than semantic.
- `OpenSSL` is a useful reminder that some important APIs are intentionally opaque and should be modeled that way, not forced through layout probing.

## Consumer Implications

- downstream users should treat the ladder as progressive confidence, not a binary supported/unsupported list
- `zlib` is a strong default smoke target for `fol`
- `libpcap` and `libcurl` are better stress targets for callback and macro policy
- `OpenSSL` is the best current stress target for opaque-handle policy and “do not over-claim ABI evidence” discipline

## Runtime-Loaded Boundary

The plugin ABI fixture in [plugin_abi.h](/home/bresilla/data/code/bresilla/linc/test/stress/plugin_abi.h)
is deliberately separate from the normal library ladder because it models a different kind of
problem.

What `linc` can model well here:

- the plugin ABI header itself
- callback and opaque-state signatures
- declared host-side dependencies such as an explicit `dl` requirement
- record and function-pointer layout evidence for the ABI surface

What `linc` should not pretend to prove here:

- that a specific plugin shared object will be discovered at runtime
- that `dlsym` name resolution policy is equivalent to ordinary link resolution
- that successful header analysis means the runtime loader will find the symbol in a concrete deployment

So the runtime-loaded rule is:

- use `linc` to model the ABI contract
- use downstream runtime policy to model `dlopen`/`dlsym` discovery and failure handling

## Plugin Findings

Current findings from the plugin ABI stress fixture:

- `linc` models the plugin ABI header well as a normal C surface:
  - callback typedefs
  - opaque handles
  - descriptor records
  - exported descriptor factory function
- layout probing is useful for concrete ABI records such as `bic_plugin_descriptor`
- the explicit `dl` link requirement is useful as metadata, but it should not be mistaken for proof
  that runtime loading will succeed in deployment

Current limitations exposed by this fixture:

- there is no direct notion of “symbol must be discovered by `dlsym` under this policy” in the core
  validation model yet
- link planning can describe host-side native requirements, but runtime plugin discovery remains a
  downstream responsibility

Practical implication:

- use `linc` to define and check the plugin ABI contract
- do not use `linc` as a substitute for runtime loader policy, plugin search paths, or deployment-time
  symbol lookup handling
