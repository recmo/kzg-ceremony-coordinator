#![doc = include_str!("../Readme.md")]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]

mod contribution;

use ark_bls12_381::{Fq, FqParameters, G1Affine};
use ark_ec::AffineCurve;
use ark_ff::{fields::FpParameters, BigInteger384, FromBytes, Zero};
use ark_serialize::CanonicalSerialize;
use axum::{
    routing::{get, post},
    Router, Server,
};
use clap::Parser;
use cli_batteries::await_shutdown;
use eyre::{bail, ensure, Result as EyreResult, Result};
use hex::FromHexError;
use ruint::{
    aliases::{U256, U384},
    uint,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use thiserror::Error;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use url::{Host, Url};
use valico::json_schema;
use ark_ff::BigInteger;

#[derive(Clone, Debug, PartialEq, Parser)]
pub struct Options {
    /// API Server url
    #[clap(long, env, default_value = "http://127.0.0.1:8080/")]
    pub server: Url,
}

#[derive(Clone, Copy, PartialEq, Debug, Error)]
pub enum ParseError {
    #[error("Invalid length of hex string, expected {0} chars, got {1}")]
    InvalidLength(usize, usize),
    #[error("Invalid size of hex string, expected prefix \"0x\"")]
    MissingPrefix,
    #[error(transparent)]
    InvalidHex(#[from] FromHexError),
    #[error("Invalid x coordinate")]
    BigIntError,
    #[error("Point is not compressed")]
    NotCompressed,
    #[error("Point at infinity must have zero x coordinate")]
    InvalidInfinity,
    #[error("number is not in the field")]
    InvalidXField,
    #[error("not a valid x coordinate")]
    InvalidXCoordinate,
    #[error("curve point is not in prime order subgroup")]
    InvalidSubgroup,
}

pub fn parse_hex<const N: usize>(hex: &str) -> Result<[u8; N], ParseError> {
    let expected_len = 2 + 2 * N;
    if hex.len() != expected_len {
        return Err(ParseError::InvalidLength(expected_len, hex.len()));
    }
    if &hex[0..2] != "0x" {
        return Err(ParseError::MissingPrefix);
    }
    let mut buffer = [0u8; N];
    hex::decode_to_slice(&hex[2..], &mut buffer)?;
    Ok(buffer)
}

/// See <https://github.com/zcash/librustzcash/blob/6e0364cd42a2b3d2b958a54771ef51a8db79dd29/pairing/src/bls12_381/README.md#serialization>
pub fn parse_g1(hex: &str) -> Result<G1Affine, ParseError> {
    // Read hex string
    let mut bytes = parse_hex::<48>(hex)?;

    // Read and mask flags
    let compressed = bytes[0] & 0x80 != 0;
    let infinity = bytes[0] & 0x40 != 0;
    let greatest = bytes[0] & 0x20 != 0;
    bytes[0] &= 0x1f;

    // Read x coordinate
    let x = {
        let mut x = BigInteger384::default();
        bytes.reverse();
        let mut reader = &bytes[..];
        x.read_le(&mut reader).map_err(|_| ParseError::BigIntError)?;
        if reader.len() != 0 {
            return Err(ParseError::BigIntError);
        }
        x
    };
    if x >= FqParameters::MODULUS {
        return Err(ParseError::InvalidXField);
    }
    let x: Fq = x.into();

    // Construct point
    if !compressed {
        return Err(ParseError::NotCompressed);
    }
    if infinity {
        if greatest || x != Fq::zero() {
            return Err(ParseError::InvalidInfinity);
        }
        return Ok(G1Affine::zero());
    }
    let point = G1Affine::get_point_from_x(x, greatest).ok_or(ParseError::InvalidXCoordinate)?;
    debug_assert!(point.is_on_curve()); // Always true
    if !point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(ParseError::InvalidSubgroup);
    }

    Ok(point)
}

pub async fn main(options: Options) -> EyreResult<()> {
    let app = Router::new()
        .layer(TraceLayer::new_for_http())
        .route("/login", post(|| async { "Hello, World!" }))
        .route("/ceremony/status", get(|| async { "Hello, World!" }))
        .route("/queue/join", post(|| async { "Hello, World!" }))
        .route("/queue/checkin", post(|| async { "Hello, World!" }))
        .route("/queue/leave", post(|| async { "Hello, World!" }))
        .route("/contribution/start", post(contribution::start))
        .route("/contribution/complete", post(|| async { "Hello, World!" }))
        .route("/contribution/abort", post(|| async { "Hello, World!" }));

    // Load schema
    let schema = serde_json::from_str(include_str!("../specs/contributionSchema.json")).unwrap();
    let mut scope = json_schema::Scope::new();
    let schema = scope.compile_and_return(schema, false).unwrap();

    // Load initial contribution
    let initial = serde_json::from_str(include_str!("../specs/initialContribution.json")).unwrap();
    let validation = schema.validate(&initial);
    if !validation.is_strictly_valid() {
        for error in validation.errors {
            error!("{}", error);
        }
        for missing in validation.missing {
            error!("Missing {}", missing);
        }
        // TODO bail!("Initial contribution is not valid.");
    }
    info!("Initial contribution is json-schema valid.");

    let initial = serde_json::from_value::<contribution::Contributions>(initial)?;

    // Verify that the initial contribution is valid
    let g1 = ark_bls12_381::G1Affine::prime_subgroup_generator();
    dbg!(g1);

    let mut buffer = Vec::new();
    dbg!(g1.serialize(&mut buffer));
    dbg!(hex::encode(buffer));

    // bbc622db0af03afbef1a7af93fe8556c58ac1b173f3a4ea105b974974f8c68c30faca94f8c63952694d79731a7d3f117
    // 0x97f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb

    dbg!(U384::from(g1.x));
    dbg!(U384::from(g1.y));

    // Run the server
    let (addr, prefix) = parse_url(&options.server)?;
    let app = Router::new().nest(prefix, app);
    let server = Server::try_bind(&addr)?.serve(app.into_make_service());
    info!("Listening on http://{}{}", server.local_addr(), prefix);
    server.with_graceful_shutdown(await_shutdown()).await?;
    Ok(())
}

fn parse_url(url: &Url) -> Result<(SocketAddr, &str)> {
    ensure!(
        url.scheme() == "http",
        "Only http:// is supported in {}",
        url
    );
    let prefix = url.path();
    let ip: IpAddr = match url.host() {
        Some(Host::Ipv4(ip)) => ip.into(),
        Some(Host::Ipv6(ip)) => ip.into(),
        Some(_) => bail!("Cannot bind {}", url),
        None => Ipv4Addr::LOCALHOST.into(),
    };
    let port = url.port().unwrap_or(8080);
    let addr = SocketAddr::new(ip, port);
    Ok((addr, prefix))
}

#[cfg(test)]
pub mod test {
    use super::*;
    use proptest::proptest;
    use tracing::{error, warn};
    use tracing_test::traced_test;

    #[test]
    fn test_parse_g1() {
        assert_eq!(parse_g1("0xc00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap(), G1Affine::zero());
        assert_eq!(parse_g1("0x97f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb").unwrap(), G1Affine::prime_subgroup_generator());
    }

    #[test]
    #[allow(clippy::eq_op)]
    fn test_with_proptest() {
        proptest!(|(a in 0..5, b in 0..5)| {
            assert_eq!(a + b, b + a);
        });
    }

    #[test]
    #[allow(clippy::disallowed_methods)] // False positive from macro
    #[traced_test]
    fn test_with_log_output() {
        error!("logged on the error level");
        assert!(logs_contain("logged on the error level"));
    }

    #[tokio::test]
    #[allow(clippy::disallowed_methods)] // False positive from macro
    #[traced_test]
    #[allow(clippy::semicolon_if_nothing_returned)] // False positive
    async fn async_test_with_log() {
        // Local log
        info!("This is being logged on the info level");

        // Log from a spawned task (which runs in a separate thread)
        tokio::spawn(async {
            warn!("This is being logged on the warn level from a spawned task");
        })
        .await
        .unwrap();

        // Ensure that `logs_contain` works as intended
        assert!(logs_contain("logged on the info level"));
        assert!(logs_contain("logged on the warn level"));
        assert!(!logs_contain("logged on the error level"));
    }
}

#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench {
    use criterion::{black_box, BatchSize, Criterion};
    use proptest::{
        strategy::{Strategy, ValueTree},
        test_runner::TestRunner,
    };
    use std::time::Duration;
    use tokio::runtime;

    pub fn group(criterion: &mut Criterion) {
        bench_example_proptest(criterion);
        bench_example_async(criterion);
    }

    /// Constructs an executor for async tests
    pub(crate) fn runtime() -> runtime::Runtime {
        runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    /// Example proptest benchmark
    /// Uses proptest to randomize the benchmark input
    fn bench_example_proptest(criterion: &mut Criterion) {
        let input = (0..5, 0..5);
        let mut runner = TestRunner::deterministic();
        // Note: benchmarks need to have proper identifiers as names for
        // the CI to pick them up correctly.
        criterion.bench_function("example_proptest", move |bencher| {
            bencher.iter_batched(
                || input.new_tree(&mut runner).unwrap().current(),
                |(a, b)| {
                    // Benchmark number addition
                    black_box(a + b)
                },
                BatchSize::LargeInput,
            );
        });
    }

    /// Example async benchmark
    /// See <https://bheisler.github.io/criterion.rs/book/user_guide/benchmarking_async.html>
    fn bench_example_async(criterion: &mut Criterion) {
        let duration = Duration::from_micros(1);
        criterion.bench_function("example_async", move |bencher| {
            bencher.to_async(runtime()).iter(|| async {
                tokio::time::sleep(duration).await;
            });
        });
    }
}
