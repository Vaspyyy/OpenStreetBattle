use crate::Soldier;
use osb_world::Point;
use std::collections::BTreeMap;
/// Stable iteration order avoids randomized hash traversal changing target selection.
pub(crate) struct Spatial {buckets:BTreeMap<(i32,i32),Vec<usize>>}
impl Spatial {
    pub fn new(soldiers:&[Soldier])->Self {
        let mut buckets:BTreeMap<_,Vec<_>>=BTreeMap::new();
        for (i,s) in soldiers.iter().enumerate() {buckets.entry(Self::cell(s.position)).or_default().push(i);}
        Self{buckets}
    }
    fn cell(p:Point)->(i32,i32) {((p.x/100.0).floor() as i32,(p.y/100.0).floor() as i32)}
    pub fn near(&self,p:Point,radius:f64)->Vec<usize> {
        let (x0,y0)=Self::cell(Point::new(p.x-radius,p.y-radius));
        let (x1,y1)=Self::cell(Point::new(p.x+radius,p.y+radius));
        let mut out=Vec::new();
        for y in y0..=y1 {for x in x0..=x1 {if let Some(v)=self.buckets.get(&(x,y)) {out.extend(v);}}}
        out.sort_unstable();out
    }
}
