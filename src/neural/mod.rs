//! 009 optional semantic retrieval. `provider` and `protocol` are the shared
//! boundary between core preparation and the supervised embedding worker;
//! neither loads a model, a tokenizer or Python.
pub mod profile;
pub mod protocol;
pub mod provider;
