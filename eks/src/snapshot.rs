use serde::{Serialize, Deserialize};
use crate::Component;


pub trait Snappable<'a>: Serialize + Deserialize<'a> {}


#[derive(Debug, ComponentIdent, Serialize, Deserialize, Snappable)]
pub struct G {}

