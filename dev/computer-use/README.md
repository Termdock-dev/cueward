# Computer-use performance measurements

For the separate fresh-agent task preparation and result checker, see the [task acceptance framework](task-acceptance.md). Benchmark/probe outcomes are not fresh-agent task acceptance results.

The [read-only desktop observer](desktop-observer.md) supplies timestamped task intervals, per-online-display visible Spaces, foreground/pointer samples and receiver activation events. It remains separate from task execution and independent artifact checks.

Run on an unlocked macOS desktop with Accessibility and Screen Recording permissions:

```sh
cargo build --release
python3 dev/computer-use/benchmark.py --samples 10 --output benchmark.json
```

The runner creates two disposable AppKit windows from the existing input fixture. It never selects an existing user window, activates the fixture, switches Spaces or posts global input. It terminates its receiver and deletes its temporary files and generated snapshots, including on failure. The sample output contains timings, platform/version information, source/binary hashes and aggregate fixture results, without window titles, field contents or local paths.

The measured operations are window enumeration, AX exploration, a matching element wait, a background Right-arrow pair, and screenshot capture. The CLI workloads use the release executable. Snapshot preparation and receiver checks occur outside the timed operation. Model inference time is excluded.

The native helper sources are copied from production and instrumented in the temporary directory. Instrumentation records the interval from the first executable statement through platform calls and validation, then final JSON serialization. The benchmark stops if the timing insertion points no longer match the sources. Both interpreter and precompiled runs execute the same instrumented source, retaining its guards and request semantics. The benchmark uses an isolated lock for direct helper calls; workloads run sequentially and never overlap CLI input.

Each report preserves every sample, the first invocation, median, nearest-rank p95, minimum and maximum. Explicit `swiftc` compilation is timed once per helper. `startup_and_transport_ms` is the measured subprocess wall time minus in-process work and final serialization. It includes executable/runtime loading, transport and, for interpreted sources, Swift driver compilation/cache work; it is not an isolated platform API measurement. Work includes validation, source initialization and internal fingerprint serialization. The system and compiler caches are not reset. First invocation is reported separately but does not represent a machine with empty caches. The screenshot executable's internals are opaque; its complete invocation and the complete CLI snapshot workload are reported separately.

For input, each sample also records the observed increment in receiver key-downs and key-ups, with a bounded 500 ms check. These checks do not resend input. The final receiver verification checks total pairs, unchanged text and its inactive state. A missing effect leaves the report on disk and exits with an error; a fast dispatch with missing receiver events is never counted as a successful optimization. Endpoint checks do not provide continuous foreground, pointer or visible-Space isolation evidence.

## Recorded run: 2026-10-01

[Machine-readable samples](results/2026-10-01.json) contain five samples per operation on macOS 27.0, arm64. The report includes the exact instrumented-source hashes and release binary hash. Timing samples vary with system load; five samples establish a local baseline, not a portable latency guarantee.

CLI times in milliseconds:

| Operation | First | Median | p95 |
| --- | ---: | ---: | ---: |
| Catalog | 565.0 | 565.0 | 1117.7 |
| AX inspect | 2166.1 | 826.1 | 2166.1 |
| Snapshot | 704.0 | 717.8 | 740.2 |
| Matching wait | 579.8 | 684.2 | 739.2 |
| Background key | 1540.3 | 1540.3 | 2009.1 |

Helper medians in milliseconds:

| Helper | Explicit compilation | Interpreted subprocess | Interpreted work | Serialization | Startup/transport residual | Precompiled subprocess |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Catalog | 503.8 | 549.8 | 77.7 | 0.8 | 471.0 | 39.1 |
| AX inspect | 972.8 | 973.9 | 135.1 | 0.2 | 833.7 | 82.2 |
| Matching wait | 774.5 | 1008.4 | 102.2 | 0.1 | 878.9 | 86.8 |
| Background key | 1032.3 | 1435.2 | 205.1 | 0.1 | 1288.2 | 103.0 |

Columns are independently calculated medians and need not add up. The screenshot executable's median was 129.4 ms, while the full CLI snapshot also includes catalog reads, file publication and token serialization.

All five interpreted helper key pairs and all five CLI key pairs reached the receiver. Four of five precompiled pairs arrived; one returned `sent_unverified` with two posted events but produced no receiver down/up within the check or in the final trace. Fourteen total pairs arrived, releases stayed paired, the receiver text stayed empty and the fixture was inactive at the final observation. The report's overall input verification is therefore **failed**. An earlier unpaced run also lost precompiled effects; its timings are not substituted for this run's recorded samples.

These samples show a large interpreter startup/transport residual and much smaller final JSON serialization cost. They justify investigating helper startup, but do not establish a usable input optimization. The missing precompiled effect remains unexplained: event delivery/lifetime and command pacing are hypotheses, not confirmed causes. Production continues to use its existing helper route. No cache, persistent helper, service or new dependency is introduced, and no speedup is claimed for the delivered CLI.

The initial matching wait uncovered unconditional reads of irrelevant AXDescription attributes. The production wait now reads only selector/condition attributes; deterministic tests preserve required-attribute failures, and the native text receiver and existing wait desktop tests pass. This correction enables the matching-wait baseline without weakening role or secure-value checks.
