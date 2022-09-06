/// Optimized subgroup checks.
///
/// Taken from latest (unreleased) arkworks:
/// See [bls12_381/src/curves/g1.rs](https://github.com/arkworks-rs/curves/blob/dc555882cd867b1e5b6fb16f840ebb0b336136d1/bls12_381/src/curves/g1.rs#L48)
/// See [bls12_381/src/curves/g2.rs](https://github.com/arkworks-rs/curves/blob/dc555882cd867b1e5b6fb16f840ebb0b336136d1/bls12_381/src/curves/g2.rs#L112)
use ark_bls12_381::{Fq, G1Affine, G1Projective, G2Projective, Parameters};
use ark_bls12_381::{Fq2, G2Affine};
use ark_ec::{bls12::Bls12Parameters, AffineCurve, ProjectiveCurve};
use ark_ff::{field_new, BigInteger384, Field, Zero};
use std::ops::Neg;

/// is_in_correct_subgroup_assuming_on_curve
#[inline]
pub fn g1_subgroup_check(p: &G1Affine) -> bool {
    // Algorithm from Section 6 of https://eprint.iacr.org/2021/1130.
    //
    // Check that endomorphism_p(P) == -[X^2]P

    // An early-out optimization described in Section 6.
    // If uP == P but P != point of infinity, then the point is not in the right
    // subgroup.
    let x_times_p = g1_mul_bigint(p, Parameters::X);
    if x_times_p.eq(p) && !p.infinity {
        return false;
    }

    let minus_x_squared_times_p = g1_mul_bigint_proj(&x_times_p, Parameters::X).neg();
    let endomorphism_p = g1_endomorphism(p);
    minus_x_squared_times_p.eq(&endomorphism_p)
}

#[inline]
pub fn g2_subgroup_check(point: &G2Affine) -> bool {
    // Algorithm from Section 4 of https://eprint.iacr.org/2021/1130.
    //
    // Checks that [p]P = [X]P

    let mut x_times_point = g2_mul_bigint(point, Parameters::X);
    if Parameters::X_IS_NEGATIVE {
        x_times_point = -x_times_point;
    }

    let p_times_point = g2_endomorphism(point);

    x_times_point.eq(&p_times_point)
}

#[inline]
fn g1_mul_bigint(base: &G1Affine, scalar: &[u64]) -> G1Projective {
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
fn g1_mul_bigint_proj(base: &G1Projective, scalar: &[u64]) -> G1Projective {
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
fn g2_mul_bigint(base: &G2Affine, scalar: &[u64]) -> G2Projective {
    let mut res = G2Projective::zero();
    for b in ark_ff::BitIteratorBE::without_leading_zeros(scalar) {
        res.double_in_place();
        if b {
            res.add_assign_mixed(base);
        }
    }
    res
}

#[inline]
pub fn g1_endomorphism(p: &G1Affine) -> G1Affine {
    /// BETA is a non-trivial cubic root of unity in Fq.
    const BETA: Fq = field_new!(Fq, "793479390729215512621379701633421447060886740281060493010456487427281649075476305620758731620350");

    // Endomorphism of the points on the curve.
    // endomorphism_p(x,y) = (BETA * x, y)
    // where BETA is a non-trivial cubic root of unity in Fq.
    let mut res = (*p).clone();
    res.x *= BETA;
    res
}

#[inline]
pub fn g2_endomorphism(p: &G2Affine) -> G2Affine {
    // The p-power endomorphism for G2 is defined as follows:
    // 1. Note that G2 is defined on curve E': y^2 = x^3 + 4(u+1).
    //    To map a point (x, y) in E' to (s, t) in E,
    //    one set s = x / ((u+1) ^ (1/3)), t = y / ((u+1) ^ (1/2)),
    //    because E: y^2 = x^3 + 4.
    // 2. Apply the Frobenius endomorphism (s, t) => (s', t'),
    //    another point on curve E, where s' = s^p, t' = t^p.
    // 3. Map the point from E back to E'; that is,
    //    one set x' = s' * ((u+1) ^ (1/3)), y' = t' * ((u+1) ^ (1/2)).
    //
    // To sum up, it maps
    // (x,y) -> (x^p / ((u+1)^((p-1)/3)), y^p / ((u+1)^((p-1)/2)))
    // as implemented in the code as follows.

    // PSI_X = 1/(u+1)^((p-1)/3)
    const P_POWER_ENDOMORPHISM_COEFF_0_1: Fq =
        field_new!(Fq,
            "4002409555221667392624310435006688643935503118305586438271171395842971157480381377015405980053539358417135540939437"
        );
    // PSI_Y = 1/(u+1)^((p-1)/2)
    const P_POWER_ENDOMORPHISM_COEFF_1_0: Fq = field_new!(Fq,
            "2973677408986561043442465346520108879172042883009249989176415018091420807192182638567116318576472649347015917690530");
    const P_POWER_ENDOMORPHISM_COEFF_1_1: Fq = field_new!(Fq,
            "1028732146235106349975324479215795277384839936929757896155643118032610843298655225875571310552543014690878354869257");

    let mut res = *p;
    res.x.frobenius_map(1);
    res.y.frobenius_map(1);

    let tmp_x = res.x.clone();
    res.x.c0 = -P_POWER_ENDOMORPHISM_COEFF_0_1 * &tmp_x.c1;
    res.x.c1 = P_POWER_ENDOMORPHISM_COEFF_0_1 * &tmp_x.c0;
    res.y *= Fq2::new(
        P_POWER_ENDOMORPHISM_COEFF_1_0,
        P_POWER_ENDOMORPHISM_COEFF_1_1,
    );

    res
}
