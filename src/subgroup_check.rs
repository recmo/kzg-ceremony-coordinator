use ark_bls12_381::{Fq, G1Affine, G1Projective, Parameters};
use ark_ec::{bls12::Bls12Parameters, AffineCurve, ProjectiveCurve};
use ark_ff::{field_new, BigInteger384, Zero};
use std::ops::Neg;

/// BETA is a non-trivial cubic root of unity in Fq.
pub const BETA: Fq = field_new!(Fq, "793479390729215512621379701633421447060886740281060493010456487427281649075476305620758731620350");

/// is_in_correct_subgroup_assuming_on_curve
#[inline]
pub fn g1_subgroup_check(p: &G1Affine) -> bool {
    // Algorithm from Section 6 of https://eprint.iacr.org/2021/1130.
    //
    // Check that endomorphism_p(P) == -[X^2]P

    // An early-out optimization described in Section 6.
    // If uP == P but P != point of infinity, then the point is not in the right
    // subgroup.
    let x_times_p = mul_bigint(p, Parameters::X);
    if x_times_p.eq(p) && !p.infinity {
        return false;
    }

    let minus_x_squared_times_p = mul_bigint_proj(&x_times_p, Parameters::X).neg();
    let endomorphism_p = g1_endomorphism(p);
    minus_x_squared_times_p.eq(&endomorphism_p)
}

#[inline]
fn mul_bigint(base: &G1Affine, scalar: &[u64]) -> G1Projective {
    let mut res = G1Projective::zero();
    for b in ark_ff::BitIteratorBE::without_leading_zeros(scalar) {
        res.double_in_place();
        if b {
            res.add_assign_mixed(base);
        }
    }
    res
}

#[inline]
fn mul_bigint_proj(base: &G1Projective, scalar: &[u64]) -> G1Projective {
    let mut res = G1Projective::zero();
    for b in ark_ff::BitIteratorBE::without_leading_zeros(scalar) {
        res.double_in_place();
        if b {
            res += base;
        }
    }
    res
}

#[inline]
pub fn g1_endomorphism(p: &G1Affine) -> G1Affine {
    // Endomorphism of the points on the curve.
    // endomorphism_p(x,y) = (BETA * x, y)
    // where BETA is a non-trivial cubic root of unity in Fq.
    let mut res = (*p).clone();
    res.x *= BETA;
    res
}
