//! Validated intent boundary. There is deliberately no model client or network dependency.
use osb_world::{Map,Point};
use serde::{Deserialize,Serialize};
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Objective {Capture,Defend}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {pub objective:Objective,pub target:Point,pub radius_m:f64,pub casualty_tolerance:f64}
#[derive(Debug,thiserror::Error)]
pub enum IntentError {
    #[error("intent JSON: {0}")] Json(#[from]serde_json::Error),
    #[error("invalid intent: {0}")] Invalid(String),
}
impl Intent {
    pub fn parse(text:&str,map:&Map)->Result<Self,IntentError> {
        if text.len()>16_384 {return Err(IntentError::Invalid("intent exceeds 16 KiB".into()));}
        let intent:Self=serde_json::from_str(text)?;intent.validate(map)?;Ok(intent)
    }
    pub fn validate(&self,map:&Map)->Result<(),IntentError> {
        if !map.walkable(self.target) {return Err(IntentError::Invalid("target must be inside the playable region and outside solid geometry".into()));}
        if !self.radius_m.is_finite() || !(5.0..=1000.0).contains(&self.radius_m) || !self.casualty_tolerance.is_finite() || !(0.0..=1.0).contains(&self.casualty_tolerance) {return Err(IntentError::Invalid("radius or casualty tolerance is out of range".into()));}
        Ok(())
    }
}
/// A future optional translator may implement this. Its output must still pass Intent::parse.
/// It cannot return arbitrary code, paths, tools, shell commands or entity mutations.
pub trait IntentTranslator {fn translate(&self,text:&str)->Result<String,IntentError>;}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn valid_intent(){Intent::parse(r#"{"objective":"capture","target":{"x":350.0,"y":300.0},"radius_m":30.0,"casualty_tolerance":0.5}"#,&Map::demo()).unwrap();}
    #[test] fn rejects_solid_target(){let i=Intent{objective:Objective::Capture,target:Point::new(450.0,300.0),radius_m:30.0,casualty_tolerance:0.5};assert!(i.validate(&Map::demo()).is_err());}
    #[test] fn rejects_extra_instructions(){assert!(Intent::parse(r#"{"objective":"capture","target":{"x":350.0,"y":300.0},"radius_m":30.0,"casualty_tolerance":0.5,"shell":"anything"}"#,&Map::demo()).is_err());}
}
