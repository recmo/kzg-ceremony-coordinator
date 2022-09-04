use ark_bls12_381::{Bls12_381, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{AffineCurve, PairingEngine, ProjectiveCurve};
use ark_ff::UniformRand;
use rand::prelude::*;

type Pair = (G1Affine, G2Affine);

pub struct BatchPairingCheck {
    lhs: (G1Projective, G2Projective),
    rhs: (G1Projective, G2Projective),
}

impl BatchPairingCheck {
    pub fn new() -> Self {
        Self {
            lhs: (
                G1Projective::prime_subgroup_generator(),
                G2Projective::prime_subgroup_generator(),
            ),
            rhs: (
                G1Projective::prime_subgroup_generator(),
                G2Projective::prime_subgroup_generator(),
            ),
        }
    }

    pub fn add_check(&mut self, lhs: Pair, rhs: Pair) {
        // The default rng in [`rand`] is cryptographically secure.
        let mut rng = rand::thread_rng();
        let factor = Fr::rand(&mut rng);
        self.lhs.0 += lhs.0.mul(factor);
        self.lhs.1 += lhs.1.mul(factor);
        self.rhs.0 += rhs.0.mul(factor);
        self.rhs.1 += rhs.1.mul(factor);
    }

    pub fn merge(&mut self, other: Self) {
        self.lhs.0 += other.lhs.0;
        self.lhs.1 += other.lhs.1;
        self.rhs.0 += other.rhs.0;
        self.rhs.1 += other.rhs.1;
    }

    pub fn check(self) -> bool {
        Bls12_381::pairing(self.lhs.0, self.lhs.1) == Bls12_381::pairing(self.rhs.0, self.rhs.1)
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use ark_bls12_381::{G1Affine, G2Affine};
    use ark_ec::AffineCurve;
    use proptest::proptest;
}

#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench {
    use super::*;
    use ark_bls12_381::{g1, g2};
    use criterion::{black_box, BatchSize, Criterion};
    use proptest::{
        strategy::{Strategy, ValueTree},
        test_runner::TestRunner,
    };

    pub fn group(criterion: &mut Criterion) {
        bench_rand(criterion);
        bench_eq(criterion);
        bench_merge(criterion);
        bench_check(criterion);
    }

    fn rand_pairing() -> (Pair, Pair) {
        let mut rng = rand::thread_rng();
        let a = Fr::rand(&mut rng);
        let b = Fr::rand(&mut rng);
        let r = Fr::rand(&mut rng);
        (
            (
                G1Affine::prime_subgroup_generator()
                    .mul(r * a)
                    .into_affine(),
                G2Affine::prime_subgroup_generator().mul(a).into_affine(),
            ),
            (
                G1Affine::prime_subgroup_generator()
                    .mul(r * b)
                    .into_affine(),
                G2Affine::prime_subgroup_generator().mul(b).into_affine(),
            ),
        )
    }

    fn rand_check() -> BatchPairingCheck {
        let (lhs, rhs) = rand_pairing();
        let mut check = BatchPairingCheck::new();
        check.add_check(lhs, rhs);
        check
    }

    fn bench_rand(criterion: &mut Criterion) {
        criterion.bench_function("pairing/rand", move |bencher| {
            let mut rng = rand::thread_rng();
            bencher.iter(|| black_box(Fr::rand(&mut rng)));
        });
    }

    fn bench_eq(criterion: &mut Criterion) {
        criterion.bench_function("pairing/add", move |bencher| {
            let mut check = rand_check();
            bencher.iter_batched(
                || rand_pairing(),
                |(lhs, rhs)| check.add_check(black_box(lhs), black_box(rhs)),
                BatchSize::SmallInput,
            );
        });
    }

    fn bench_merge(criterion: &mut Criterion) {
        criterion.bench_function("pairing/merge", move |bencher| {
            bencher.iter_batched(
                || (rand_check(), rand_check()),
                |(lhs, rhs)| black_box(lhs).merge(black_box(rhs)),
                BatchSize::SmallInput,
            );
        });
    }

    fn bench_check(criterion: &mut Criterion) {
        criterion.bench_function("pairing/check", move |bencher| {
            bencher.iter_batched(
                || rand_check(),
                |check| black_box(check).check(),
                BatchSize::SmallInput,
            );
        });
    }
}
