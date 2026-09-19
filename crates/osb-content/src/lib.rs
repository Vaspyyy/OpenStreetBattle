//! Data-driven fictional equipment. These numbers are prototype game parameters, not validated ballistics.
use serde::{Deserialize,Serialize};
use std::collections::BTreeSet;
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weapon {
    pub id:String,pub label:String,pub ammunition:String,
    pub range_m:f64,pub cycle_ticks:u32,pub hit_probability:f64,pub suppression:f64,pub wound_severity:f64,
    pub starting_rounds:u32,
}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {pub schema_version:u32,pub weapons:Vec<Weapon>}
#[derive(Debug,thiserror::Error)]
pub enum ContentError {
    #[error("content JSON: {0}")] Json(#[from]serde_json::Error),
    #[error("invalid content: {0}")] Invalid(String),
}
impl Content {
    pub fn generic()->Self {Self::parse(include_str!("../../../assets/content/generic/equipment.json")).expect("embedded equipment is validated by tests")}
    pub fn parse(text:&str)->Result<Self,ContentError> {let c:Self=serde_json::from_str(text)?;c.validate()?;Ok(c)}
    pub fn validate(&self)->Result<(),ContentError> {
        if self.schema_version!=1 || self.weapons.is_empty() {return Err(ContentError::Invalid("unsupported version or empty weapon catalog".into()));}
        let mut ids=BTreeSet::new();
        for w in &self.weapons {
            if w.id.is_empty() || !ids.insert(&w.id) || w.ammunition.is_empty() || !w.range_m.is_finite() || !(1.0..=2000.0).contains(&w.range_m) || w.cycle_ticks==0 || w.starting_rounds>10_000 || [w.hit_probability,w.suppression,w.wound_severity].into_iter().any(|v|!v.is_finite() || !(0.0..=1.0).contains(&v)) {
                return Err(ContentError::Invalid(format!("invalid weapon {}",w.id)));
            }
        }
        Ok(())
    }
    pub fn weapon(&self,id:&str)->Option<&Weapon> {self.weapons.iter().find(|w|w.id==id)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn generic_is_valid(){Content::generic().validate().unwrap();}
    #[test] fn duplicate_ids_rejected(){let mut c=Content::generic();c.weapons.push(c.weapons[0].clone());assert!(c.validate().is_err());}
    #[test] fn negative_range_rejected(){let mut c=Content::generic();c.weapons[0].range_m=-1.0;assert!(c.validate().is_err());}
    #[test] fn unknown_fields_rejected(){assert!(Content::parse(r#"{"schema_version":1,"weapons":[],"execute":"shell"}"#).is_err());}
}
