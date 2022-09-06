#![doc = include_str!("../Readme.md")]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]

mod contribution;
mod pairing_check;
mod parse_g;
mod subgroup_check;

use crate::{
    contribution::{Contribution, ContributionsJson, Transcript},
    parse_g::parse_g,
};
use ark_bls12_381::{Fq, FqParameters, Fr, G1Affine, G2Affine};
use ark_ff::UniformRand;
use axum::{
    routing::{get, post},
    Router, Server,
};
use clap::Parser;
use cli_batteries::await_shutdown;
use eyre::{bail, ensure, Result as EyreResult, Result};
use hex::FromHexError;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use thiserror::Error;
use tower_http::trace::TraceLayer;
use tracing::{error, info, info_span};
use url::{Host, Url};
use valico::json_schema;

pub use crate::subgroup_check::g1_subgroup_check;

#[derive(Clone, Debug, PartialEq, Parser)]
pub struct Options {
    /// API Server url
    #[clap(long, env, default_value = "http://127.0.0.1:8080/")]
    pub server: Url,
}

pub async fn main(options: Options) -> EyreResult<()> {
    let app = Router::new()
        .layer(TraceLayer::new_for_http())
        .route("/login", post(|| async { "Hello, World!" }))
        .route("/ceremony/status", get(|| async { "Hello, World!" }))
        .route("/queue/join", post(|| async { "Hello, World!" }))
        .route("/queue/checkin", post(|| async { "Hello, World!" }))
        .route("/queue/leave", post(|| async { "Hello, World!" }))
        .route("/contribution/start", post(|| async { "Hello, World!" }))
        .route("/contribution/complete", post(|| async { "Hello, World!" }))
        .route("/contribution/abort", post(|| async { "Hello, World!" }));

    // Load initial contribution
    info!("Reading initial contribution.");
    let initial = serde_json::from_str(include_str!("../specs/initialContribution.json")).unwrap();

    info!("Parsing initial contribution.");
    let initial: ContributionsJson = serde_json::from_value(initial)?;
    info!("Parsing initial contribution done.");

    info!("Parsing initial contribution.");
    let contributions = initial.parse()?;
    info!("Parsing initial contribution done.");

    let transcripts = crate::contribution::SIZES
        .iter()
        .map(|(n1, n2)| Transcript::new(*n1, *n2))
        .collect::<Vec<_>>();

    let mut rng = rand::thread_rng();
    let contributions = {
        let span = info_span!("Generating contributions ");
        let _guard = span.enter();
        let contributions = transcripts
            .iter()
            .map(|t| {
                let mut contribution = Contribution::new(t.g1_powers.len(), t.g2_powers.len());
                contribution.add_tau(&Fr::rand(&mut rng));
                contribution
            })
            .collect::<Vec<_>>();
        contributions
    };
    {
        let span = info_span!("Contributions subgroup check", n = contributions.len());
        let _guard = span.enter();
        contributions
            .iter()
            .for_each(|contribution| contribution.subgroup_check());
    };

    {
        let span = info_span!("Verifying contributions");
        let _guard = span.enter();
        transcripts
            .iter()
            .zip(contributions.iter())
            .for_each(|(transcript, contribution)| contribution.verify(&transcript));
    };

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
    use super::*;
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
        parse_g::bench::group(criterion);
        contribution::bench::group(criterion);
        pairing_check::bench::group(criterion);
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
