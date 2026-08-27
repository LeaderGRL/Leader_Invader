use crate::rng::DeterministicRng;

pub const SCREEN_W:i16=128;pub const SCREEN_H:i16=96;pub const PLAYER_Y:i16=87;pub const ALIEN_ROWS:usize=4;pub const ALIEN_COLS:usize=8;pub const ALIEN_X_STEP:i16=12;pub const ALIEN_Y_STEP:i16=13;pub const ALIEN_W:i16=8;pub const ALIEN_H:i16=6;

#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]pub struct InputState{pub horizontal:i8,pub fire:bool}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub struct Projectile{pub x:i16,pub y:i16}
#[derive(Debug,Clone,PartialEq,Eq)]pub struct GameState{pub player_x:i16,pub fleet_x:i16,pub fleet_y:i16,pub fleet_dir:i8,pub player_shot:Option<Projectile>,pub enemy_shot:Option<Projectile>,pub alive_rows:[u8;ALIEN_ROWS],pub score:u16,pub lives:u8,pub frame:u32,pub player_cooldown:u8,pub enemy_cooldown:u8}
impl Default for GameState{fn default()->Self{Self{player_x:60,fleet_x:8,fleet_y:12,fleet_dir:1,player_shot:None,enemy_shot:None,alive_rows:[u8::MAX;ALIEN_ROWS],score:0,lives:3,frame:0,player_cooldown:0,enemy_cooldown:24}}}
impl GameState{
 pub fn alive_count(&self)->u32{self.alive_rows.iter().map(|r|r.count_ones()).sum()} pub fn is_clear(&self)->bool{self.alive_count()==0}
 pub fn alien_alive(&self,row:usize,col:usize)->bool{row<ALIEN_ROWS&&col<ALIEN_COLS&&(self.alive_rows[row]&(1<<col))!=0}
 pub fn alien_origin(&self,row:usize,col:usize)->(i16,i16){(self.fleet_x+col as i16*ALIEN_X_STEP,self.fleet_y+row as i16*ALIEN_Y_STEP)}
 pub fn lowest_target_near(&self,x:i16)->Option<(usize,usize,i16)>{let mut best:Option<(usize,usize,i16,i16)>=None;for row in(0..ALIEN_ROWS).rev(){for col in 0..ALIEN_COLS{if!self.alien_alive(row,col){continue}let(ax,_)=self.alien_origin(row,col);let c=ax+ALIEN_W/2;let d=(c-x).abs();if best.map_or(true,|b|row>b.0||(row==b.0&&d<b.3)){best=Some((row,col,c,d))}}}best.map(|(r,c,x,_)|(r,c,x))}
 pub fn bottom_shooters(&self)->Vec<(usize,usize,i16,i16)>{let mut out=Vec::new();for col in 0..ALIEN_COLS{for row in(0..ALIEN_ROWS).rev(){if self.alien_alive(row,col){let(x,y)=self.alien_origin(row,col);out.push((row,col,x+ALIEN_W/2,y+ALIEN_H));break}}}out}
 pub fn clear_alien(&mut self,row:usize,col:usize)->bool{if!self.alien_alive(row,col){return false}self.alive_rows[row]&=!(1<<col);self.score=self.score.saturating_add(10);true}
 pub fn player_bounds(&self)->(i16,i16){(self.player_x-5,self.player_x+5)}
}
#[derive(Debug,Clone)]pub struct Bot{aim_deadband:i16,fire_aggression:u32}
impl Bot{pub fn seeded(r:&mut DeterministicRng)->Self{Self{aim_deadband:1+r.range_i16(0,2),fire_aggression:70+r.range_u32(26)}}pub fn decide(&self,g:&GameState,r:&mut DeterministicRng)->InputState{let mut h=0;if let Some(s)=g.enemy_shot{if s.y>60&&(s.x-g.player_x).abs()<11{h=if s.x<=g.player_x{1}else{-1}}}let t=g.lowest_target_near(g.player_x);if h==0{if let Some((_,_,x))=t{let d=x-g.player_x;if d.abs()>self.aim_deadband{h=if d<0{-1}else{1}}}}let aligned=t.map(|(_,_,x)|(x-g.player_x).abs()<=self.aim_deadband+1).unwrap_or(false);InputState{horizontal:h,fire:aligned&&g.player_shot.is_none()&&g.player_cooldown==0&&r.chance(self.fire_aggression,100)}}}
#[cfg(test)]mod tests{use super::*;#[test]fn alien_clear_updates_score(){let mut g=GameState::default();assert_eq!(g.alive_count(),32);assert!(g.clear_alien(3,2));assert_eq!(g.score,10);assert_eq!(g.alive_count(),31)}}
