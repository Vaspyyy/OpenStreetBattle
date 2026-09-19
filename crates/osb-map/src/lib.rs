//! Presentation-only camera. MapLibre integration is a separate, unimplemented backend.
use osb_world::Point;
#[derive(Clone,Debug)]
pub struct MapCamera {pub center:Point,pub pixels_per_metre:f64}
impl MapCamera {
    pub fn new(center:Point)->Self {Self{center,pixels_per_metre:1.0}}
    pub fn to_screen(&self,world:Point,viewport_center:Point)->Point {Point::new(viewport_center.x+(world.x-self.center.x)*self.pixels_per_metre,viewport_center.y-(world.y-self.center.y)*self.pixels_per_metre)}
    pub fn to_world(&self,screen:Point,viewport_center:Point)->Point {Point::new(self.center.x+(screen.x-viewport_center.x)/self.pixels_per_metre,self.center.y-(screen.y-viewport_center.y)/self.pixels_per_metre)}
    pub fn pan_pixels(&mut self,dx:f64,dy:f64) {self.center.x-=dx/self.pixels_per_metre;self.center.y+=dy/self.pixels_per_metre;}
    pub fn zoom_at(&mut self,factor:f64,screen:Point,viewport_center:Point) {
        if !factor.is_finite() || factor<=0.0 {return;}
        let before=self.to_world(screen,viewport_center);
        self.pixels_per_metre=(self.pixels_per_metre*factor).clamp(0.01,40.0);
        let after=self.to_world(screen,viewport_center);
        self.center.x+=before.x-after.x;self.center.y+=before.y-after.y;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn projection_roundtrip(){let c=MapCamera::new(Point::new(100.0,300.0));let p=Point::new(30.0,80.0);let v=Point::new(640.0,360.0);assert_eq!(c.to_world(c.to_screen(p,v),v),p);}
    #[test] fn zoom_keeps_cursor_anchor(){let mut c=MapCamera::new(Point::new(100.0,300.0));let p=Point::new(300.0,200.0);let v=Point::new(640.0,360.0);let before=c.to_world(p,v);c.zoom_at(2.0,p,v);assert!(before.distance(c.to_world(p,v))<1e-8);}
    #[test] fn bad_zoom_ignored(){let mut c=MapCamera::new(Point::default());c.zoom_at(f64::NAN,Point::default(),Point::default());assert_eq!(c.pixels_per_metre,1.0);}
}
