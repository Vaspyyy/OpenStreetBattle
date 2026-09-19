//! Deterministic bounded-grid A*. Spatial geometry remains authoritative during movement.
use crate::{Map,Point,WorldError};
use std::{cmp::Reverse,collections::BinaryHeap};

pub struct Navigation { width:usize,height:usize,cell:f64,min:Point,free:Vec<bool> }
impl Navigation {
    pub fn new(map:&Map,cell:f64)->Result<Self,WorldError> {
        map.validate()?;
        if !cell.is_finite() || cell<1.0 { return Err(WorldError::Invalid("navigation cell must be at least one metre".into())); }
        let width=((map.bounds.max.x-map.bounds.min.x)/cell).ceil() as usize;
        let height=((map.bounds.max.y-map.bounds.min.y)/cell).ceil() as usize;
        if width.saturating_mul(height)>1_000_000 { return Err(WorldError::Invalid("navigation exceeds one million cells; use a smaller extract".into())); }
        let mut n=Self {width,height,cell,min:map.bounds.min,free:vec![false;width*height]};
        for i in 0..n.free.len() { n.free[i]=map.walkable(n.center(i)); }
        Ok(n)
    }
    fn center(&self,i:usize)->Point { Point::new(self.min.x+(i%self.width) as f64*self.cell+self.cell*0.5,self.min.y+(i/self.width) as f64*self.cell+self.cell*0.5) }
    fn index(&self,p:Point)->Option<usize> {
        let x=((p.x-self.min.x)/self.cell).floor() as isize;
        let y=((p.y-self.min.y)/self.cell).floor() as isize;
        (x>=0 && y>=0 && x<self.width as isize && y<self.height as isize).then_some(y.max(0) as usize*self.width+x.max(0) as usize)
    }
    fn closest_connected(&self,map:&Map,p:Point)->Option<usize> {
        let initial=self.index(p)?;
        if self.free[initial] && map.clear_segment(p,self.center(initial)) { return Some(initial); }
        let (x,y)=((initial%self.width) as isize,(initial/self.width) as isize);
        let mut best:Option<(f64,usize)>=None;
        for dy in -3..=3 { for dx in -3..=3 {
            let (nx,ny)=(x+dx,y+dy);
            if nx<0 || ny<0 || nx>=self.width as isize || ny>=self.height as isize {continue;}
            let i=ny as usize*self.width+nx as usize;
            if self.free[i] && map.clear_segment(p,self.center(i)) {
                let d=p.distance(self.center(i));
                if best.is_none_or(|b|d<b.0) {best=Some((d,i));}
            }
        }}
        best.map(|b|b.1)
    }
    pub fn nearest_walkable(&self,p:Point)->Option<Point> {
        self.free.iter().enumerate().filter(|(_,f)|**f).min_by(|(a,_),(b,_)|p.distance(self.center(*a)).total_cmp(&p.distance(self.center(*b))).then(a.cmp(b))).map(|(i,_)|self.center(i))
    }
    pub fn path(&self,map:&Map,start:Point,goal:Point)->Vec<Point> {
        if !map.walkable(start) || !map.walkable(goal) {return Vec::new();}
        if map.clear_segment(start,goal) {return vec![goal];}
        let (Some(s),Some(g))=(self.closest_connected(map,start),self.closest_connected(map,goal)) else {return Vec::new();};
        let heuristic=|i:usize|->u32 {
            let dx=(i%self.width).abs_diff(g%self.width) as u32;
            let dy=(i/self.width).abs_diff(g/self.width) as u32;
            10*dx.max(dy)+4*dx.min(dy)
        };
        let mut costs=vec![u32::MAX;self.free.len()];
        let mut parents=vec![usize::MAX;self.free.len()];
        let mut open=BinaryHeap::new();
        costs[s]=0; open.push(Reverse((heuristic(s),0u32,s)));
        while let Some(Reverse((_,cost,i)))=open.pop() {
            if cost!=costs[i] {continue;}
            if i==g {
                let mut rev=vec![goal]; let mut at=g;
                loop {rev.push(self.center(at)); if at==s {break;} at=parents[at];}
                rev.reverse();
                let mut result=Vec::new(); let mut from=start; let mut next=0;
                while next<rev.len() {
                    let mut furthest=next;
                    for (j,&point) in rev.iter().enumerate().skip(next) {if map.clear_segment(from,point) {furthest=j;} else {break;}}
                    result.push(rev[furthest]); from=rev[furthest]; next=furthest+1;
                }
                return result;
            }
            let (x,y)=((i%self.width) as isize,(i/self.width) as isize);
            for dy in -1isize..=1 {for dx in -1isize..=1 {
                if dx==0 && dy==0 {continue;}
                let (nx,ny)=(x+dx,y+dy);
                if nx<0 || ny<0 || nx>=self.width as isize || ny>=self.height as isize {continue;}
                let j=ny as usize*self.width+nx as usize;
                if !self.free[j] {continue;}
                if dx!=0 && dy!=0 && (!self.free[y as usize*self.width+nx as usize] || !self.free[ny as usize*self.width+x as usize]) {continue;}
                if !map.clear_segment(self.center(i),self.center(j)) {continue;}
                let nc=cost+if dx!=0 && dy!=0 {14} else {10};
                if nc<costs[j] { costs[j]=nc;parents[j]=i;open.push(Reverse((nc+heuristic(j),nc,j))); }
            }}
        }
        Vec::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn routes_around_a_building() {let m=Map::demo();let n=Navigation::new(&m,10.0).unwrap();let mut p=Point::new(350.0,300.0);let target=Point::new(550.0,300.0);let path=n.path(&m,p,target);assert!(path.len()>1);for q in path {assert!(m.clear_segment(p,q));p=q;}assert_eq!(p,target);}
    #[test] fn does_not_route_into_building() {let m=Map::demo();let n=Navigation::new(&m,10.0).unwrap();assert!(n.path(&m,Point::new(350.0,300.0),Point::new(450.0,300.0)).is_empty());}
    #[test] fn direct_open_route() {let m=Map::demo();let n=Navigation::new(&m,10.0).unwrap();assert_eq!(n.path(&m,Point::new(10.0,10.0),Point::new(100.0,10.0)),vec![Point::new(100.0,10.0)]);}
    #[test] fn invalid_resolution_rejected() {assert!(Navigation::new(&Map::demo(),0.0).is_err());}
}
