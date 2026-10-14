use serde::{Deserialize, Serialize};
use spline as renamed;

// The source filename is deliberately different from the declared Rust module.
#[derive(Debug, PartialEq, Serialize, Deserialize, renamed::Schema)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub value: String,
}

#[renamed::contract(crate = "renamed")]
pub trait UserApi {
    #[query(name = "get")]
    async fn lookup(
        &self,
        #[doc = "The supplied value."] value: self::Input,
        outer: super::Outer,
        note: Option<String>,
    ) -> Result<self::Input, ()>;
}

pub struct Service;
impl UserApi for Service {
    async fn lookup(&self, value: Input, _: super::Outer, _: Option<String>) -> Result<Input, ()> {
        Ok(value)
    }
}
