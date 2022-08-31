use axum::{extract::Json, routing::post, Router};
use ruint::Uint;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use valico::json_schema;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum IdType {
    EthAddress,
    EnsName,
    GithubHandle,
}

pub struct ContributeStartRequest {
    id_type: IdType,
    id:      String,
}

pub struct G1([u8; 48]);

pub struct G2([u8; 96]);

pub struct Contribution {
    num_g1_powers: usize,
    num_g2_powers: usize,
    powers_of_tau: PowersOfTau,
    pot_pubkey:    Option<G1>,
}

pub struct PowersOfTau {
    g1_powers: Vec<G1>,
    g2_powers: Vec<G2>,
}

#[instrument]
pub async fn start(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    dbg!(&payload);

    Json(payload)
}
