use std::sync::{Mutex};

use once_cell::sync::Lazy;
use valico::json_schema::{
    keywords,
    schema::{self, CompilationSettings},
    Schema,
};

pub static CONTRIBUTION_SCHEMA: Lazy<Mutex<Schema>> = Lazy::new(|| {
    // Load schema
    let schema = serde_json::from_str(include_str!("../../specs/contributionSchema.json")).unwrap();

    Mutex::new(
        schema::compile(
            schema,
            None,
            CompilationSettings::new(&keywords::default(), true),
        )
        .unwrap(),
    )
});
