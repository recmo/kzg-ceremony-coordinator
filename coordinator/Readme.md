# KZG Ceremony Coordinator

Implements the coordinator for the [Ethereum KZG Ceremony](https://github.com/ethereum/kzg-ceremony-specs/).

## Hints

Lint, build, test and run

```shell
cargo fmt && cargo clippy --all-targets --all-features && cargo build --release --all-targets --all-features && cargo test --all-targets --all-features && cargo run --
```

Run benchmarks

```shell
cargo criterion
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
