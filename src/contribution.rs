use ark_bls12_381::{Fq, Fr, G1Affine, G2Affine};
use axum::{extract::Json, routing::post, Router};
use ruint::{aliases::U384, Uint};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use valico::json_schema;

type U768 = Uint<768, 12>;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum IdType {
    EthAddress,
    EnsName,
    GithubHandle,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributeStartRequest {
    id_type: IdType,
    id:      String,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributions {
    sub_contributions: [Contribution; 4],
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contribution {
    num_g1_powers: usize,
    num_g2_powers: usize,
    powers_of_tau: PowersOfTau,
    pot_pubkey:    Option<G2>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowersOfTau {
    g1_powers: Vec<G1>,
    g2_powers: Vec<G2>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct G1(U384);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct G2(U768);

#[instrument]
pub async fn start(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    dbg!(&payload);

    Json(payload)
}
