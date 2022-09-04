# KZG Ceremony Coordinator

![lines of code](https://img.shields.io/tokei/lines/github/recmo/kzg-ceremony-coordinator)
[![dependency status](https://deps.rs/repo/github/recmo/kzg-ceremony-coordinator/status.svg)](https://deps.rs/repo/github/recmo/kzg-ceremony-coordinator)
[![codecov](https://codecov.io/gh/recmo/kzg-ceremony-coordinator/branch/main/graph/badge.svg?token=WBPZ9U4TTO)](https://codecov.io/gh/recmo/kzg-ceremony-coordinator)
[![CI](https://github.com/recmo/kzg-ceremony-coordinator/actions/workflows/build-test-deploy.yml/badge.svg)](https://github.com/recmo/kzg-ceremony-coordinator/actions/workflows/build-test-deploy.yml)

Implements <https://github.com/ethereum/kzg-ceremony-specs/>

## Hints

Lint, build, test, run

```shell
cargo fmt && cargo clippy --all-targets --all-features && cargo build --all-targets --all-features && cargo test --all-targets --all-features && cargo run --
```

Run benchmarks

```shell
cargo criterion
```

```shell
cargo test -- -Z unstable-options --report-time --show-output
```

## To do

* [x] Group element deserializer.
* [ ] Reduce allocations in group deserializer.
* [ ] Group element serializer.
* [ ] Contribution deserializer.
* [ ] Use &str in contribution deserializer.
* [ ] Move validation to shutdown-interruptable background compute task.
* [ ] Validate either all or none potPubkeys.
* [ ] Validate non-trivial potPubkeys.
* [ ] Validate distinct potPubkeys in subContributions.
* [ ] Validate distinct potPubkeys in transcript.
* [ ] Contribution validate by pairing checks.
* [ ] Contribution parallel validator.
* [ ] Contribution serializer.
* [ ] Contribution validator.
* Separate out the core cryptography `/contribution` from queue, login and
  json schema validation.
