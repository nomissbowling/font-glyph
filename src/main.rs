#![doc(html_root_url = "https://docs.rs/font-glyph/0.3.1")]
//! draw font glyph outline for Rust with plotters
//!
//! link freetype.lib
//!

use std::{error::{Error}, path};

use bezier_interpolation::ncr::{recurse, Itpl, Path2d, OfsSclPch, GlyphContour};

use freetype as ft;

use image;
use image::imageops;
use plotters::prelude::*;
use plotters::prelude::full_palette::{GREY, BLUEGREY, PINK, BROWN};
use plotters::prelude::full_palette::{DEEPORANGE, DEEPPURPLE, ORANGE, PURPLE};
use plotters::prelude::full_palette::{TEAL, AMBER, INDIGO, LIME};
use plotters::prelude::full_palette::{LIGHTBLUE, LIGHTGREEN};
use plotters::backend::RGBPixel;
// use plotters::style::colors::TRANSPARENT; // RGBAPixel not for use RGB
// use plotters::element::Drawable;
use plotters::coord::Shift;

// use std::f64::consts::PI;

use unicode_width::UnicodeWidthStr;

/// manager
pub struct FreeTypeManager<'a> {
  /// font file path
  pub fname: String,
  /// keep lib
  pub libft: ft::Library,
  /// keep face
  pub face: ft::Face<'a>,
  /// previous char for kerning
  pub last: u32
}

/// manager
impl FreeTypeManager<'_> {
  /// construct
  pub fn new(fname: &str) -> Result<FreeTypeManager, Box<dyn Error>> {
    let libft = ft::Library::init()?;
    let face = libft.new_face(fname, 0)?;
    face.set_char_size(40 * 64, 0, 50, 0)?; // for bitmap ?
    face.set_pixel_sizes(24u32, 24u32)?; // for bitmap ?
    // let mut m = ft::Matrix{xx: 1, xy: 0, yx: 0, yy: 1};
    // let mut delta = ft::Vector{x: 1, y: -1};
    // face.set_transform(&mut m, &mut delta);
    Ok(FreeTypeManager{fname: fname.to_string(), libft: libft,
      face: face, last: 0u32})
  }

  /// diplay
  pub fn glyph_metrics_inf(&self, glyph: &ft::GlyphSlot) -> ft::GlyphMetrics {
    let gm = glyph.metrics(); // FT_GLYPH_METRICS
    // println!("{:?}", gm); // no Debug on freetype-rs 0.7 (0.32 is ok)
    //  width: i64, height: i64
    //  horiBearingX: i64, horiBearingY: i64, horiAdvance: i64
    //  vertBearingX: i64, vertBearingY: i64, vertAdvance: i64
    // 'a' in mikaP.ttf
    // FT_Glyph_Metrics {
    //  width: 476, height: 676,
    //  horiBearingX: 19, horiBearingY: 679, horiAdvance: 512,
    //  vertBearingX: -237, vertBearingY: 201, vertAdvance: 1024 }
    // 'あ' in mikaP.ttf
    // FT_Glyph_Metrics {
    //  width: 928, height: 961,
    //  horiBearingX: 40, horiBearingY: 844, horiAdvance: 1024,
    //  vertBearingX: -472, vertBearingY: 36, vertAdvance: 1024 }
    println!(" wh({}, {})", gm.width, gm.height);
    gm
  }

  /// display
  pub fn face_size_metrics_inf(&self) -> ft::ffi::FT_Size_Metrics { // no alias
    let sm = self.face.size_metrics().unwrap(); // FT_SIZE_METRICS
    // println!("{:?}", sm); // no Debug on freetype-rs 0.7
    //  x_ppem: u16, y_ppem: u16, x_scale: i64, y_scale: i64
    //  ascender: i64, descender: i64, height: i64, max_advance: i64
    println!(" ppem: ({}, {})", sm.x_ppem, sm.y_ppem);
    println!(" scale: ({}, {})", sm.x_scale, sm.y_scale);
    println!(" ascender: {} descender: {}", sm.ascender, sm.descender);
    println!(" height: {} max_advance: {}", sm.height, sm.max_advance);
    sm
  }

  /// get glyph outline polygon and metrics
  pub fn glyph2poly(&mut self,
    cp: u32) -> Result<(Vec<GlyphContour>, ft::GlyphMetrics), Box<dyn Error>> {
    let ch = char::from_u32(cp).ok_or("not unicode 32")?;
    let sf = format!("{}", ch);
    print!("cp: 0x{:08x} {}{}", cp, sf, if sf.width() == 1 {" "} else {""});

    // or NO_BITMAP
    // self.face.load_glyph(self.face.get_char_index(cp as usize), ...)?;
    // self.face.load_char(cp as usize, ft::face::LoadFlag::NO_SCALE)?; // 0.32.0
    self.face.load_char(cp as usize, ft::face::NO_SCALE)?; // 0.7.0
    let glyph: &ft::GlyphSlot = self.face.glyph();

    let outline: &ft::Outline = &glyph.outline().ok_or("no glyph")?;
    // println!(" {:?}", outline); // no Debug on freetype-rs 0.32.0

    let kern_ofs: Itpl<i32> = match self.last {
    0u32 => (0, 0),
    _ => {
      let cpi_l = self.face.get_char_index(self.last as usize);
      let cpi_r = self.face.get_char_index(cp as usize);
      let kn: ft::Vector = self.face.get_kerning(cpi_l, cpi_r,
        ft::face::KerningMode::KerningDefault)?;
      // KerningDefault, KerningUnfitted, KerningUnscaled
      (kn.x, kn.y) // when times.ttf etc
    }
    };
    self.last = cp;
    print!(" kerning: {:?}", kern_ofs);

    let gm = self.glyph_metrics_inf(glyph);
    Ok((kern_ofs.outline2contours(outline), gm))
  }

  /// separate chars and draw with OfsSclPch (offset scale pitch)
  pub fn draw_str_glyph(&mut self, bm: &DrawingArea<BitMapBackend<'_>, Shift>,
    fgbg: &Vec<&RGBColor>, pals: &Vec<Vec<&RGBColor>>, ctrl: bool,
    msg: &str, o: OfsSclPch) -> Result<OfsSclPch, Box<dyn Error>> {
    let mut osp: OfsSclPch = o;
    self.last = 0u32;
    for ch in msg.chars() {
      let (poly_glyph, gm) = self.glyph2poly(ch as u32)?; // 0x00003042u32)?;
      recurse(&poly_glyph, &|pg: &Vec<GlyphContour>, draw_glyph| {
        for g in pg {
          let _ = match bm.draw(
            &Polygon::new(g.osp_pts(osp, false), fgbg[if g.lr {1} else {0}])) {
          Err(e) => { println!("{:?}", e); Err(Box::new(std::fmt::Error{})) },
          Ok(_) => Ok(())
          };
          if g.children.len() > 0 { draw_glyph(&g.children); }
        }
      });
      let ctrl_osp = osp; // copy before change osp
      osp.0.0 += (gm.width as f64 * osp.1) as i32 + osp.2;
      if !ctrl { continue; };
      let ss: Vec<Vec<ShapeStyle>> = pals.iter().map(|v| v.iter().map(|&c|
        ShapeStyle::from(c)).collect()).collect();
      for g in poly_glyph.iter() {
        let pal = &ss[if g.lr {1} else {0}];
        let mut sz = g.control.len();
        if sz > 0 && g.control[0] == g.control[sz - 1] { sz -= 1; }
        for (i, &c) in g.osp_pts(ctrl_osp, true).iter().enumerate() {
          if g.spec[i] & 1u8 != 0 { continue; }
          let mut s = pal[g.spec[i] as usize / 2]; // may be 0-6
          if sz > 1 && i == sz - 1 { s = pal[7]; } // i != 0 any mark last is 7
          if i == 0 {
            bm.draw(&Cross::new(c, 8, s.stroke_width(2)))?; // M
          }else{
            let f = if g.spec[i] == 2 || g.spec[i] == 4 || g.spec[i] == 8 {
              s.filled() // L Q2 C3
            }else{ // 6, 10, 12
              s.stroke_width(1) // Q1 C2 C1
            };
            bm.draw(&Circle::new(c, 4, f))?;
          }
        }
      }
    }
    Ok(osp)
  }
}

/// main
fn main() -> Result<(), Box<dyn Error>> {
  let _cname: Vec<&str> = vec![
    "GREY", "BLUEGREY", "PINK", "BROWN",
    "DEEPORANGE", "DEEPPURPLE", "ORANGE", "PURPLE",
    "TEAL", "AMBER", "INDIGO", "LIME",
    "LIGHTBLUE", "LIGHTGREEN"];
  let _cols: Vec<&RGBColor> = vec![
    &GREY, &BLUEGREY, &PINK, &BROWN,
    &&DEEPORANGE, &DEEPPURPLE, &ORANGE, &PURPLE,
    &TEAL, &AMBER, &INDIGO, &LIME,
    &LIGHTBLUE, &LIGHTGREEN];

  let wsz: (u32, u32) = (1024, 768);
  let isz: (u32, u32) = (256, 192);
  let img = image::open("./img/_4c.png")?
    .resize_exact(isz.0, isz.1, imageops::FilterType::Nearest)
    .to_rgb8().to_vec();

  let bases = vec!["./fonts", "/windows/fonts", "/prj/font-glyph/fonts"];
  let ff = (0, "mikaP.ttf"); // (98304 98304 1536)
//  let ff = (1, "mikachanALL.ttc"); // (98304 98304 1536) control points dif
//  let ff = (1, "DFLgs9.ttc"); // (98304 98304 1536)
//  let ff = (1, "msmincho.ttc"); // (393216 393216 1536)
//  let ff = (1, "migu-1m-regular.ttf"); // (100663 100663 1728)
//  let ff = (1, "migu-1p-regular.ttf"); // (100663 100663 2304)
//  let ff = (1, "ipaexm.ttf"); // (49152 49152 1536)
//  let ff = (1, "ipaexg.ttf"); // (49152 49152 1536)
//  let ff = (1, "meiryo.ttc"); // (49152 49152 2304)
//  let ff = (1, "MochiyPopOne-Regular.ttf"); // (100663 100663 1792)
//  let ff = (1, "MochiyPopPOne-Regular.ttf"); // (100663 100663 1792)
//  let ff = (1, "FiraSans-Regular.ttf"); // (100663 100663 2176) no JP
//  let ff = (1, "times.ttf"); // (49152 49152 1792) no JP 2w kern ok
  let pf = path::Path::new(bases[ff.0]).join(ff.1); // keep temp bind
  let fname = pf.to_str().unwrap();
  println!("loading: {} {}", fname, pf.is_file());
  let mut ftm = FreeTypeManager::new(fname)?;
  let sm = ftm.face_size_metrics_inf();

  let buf: &mut Vec<u8> = &mut vec![0u8; (3 * wsz.0 * wsz.1) as usize];
  let bb = match BitMapBackend::<RGBPixel>::with_buffer_and_format(buf, wsz) {
  Err(be) => { println!("error: {:?}", be); Err(Box::new(std::fmt::Error{})) },
  Ok(bm) => Ok(bm)
  }?;
  let bm = bb.into_drawing_area();
  bm.fill(&BLUEGREY)?;

  bm.draw(&BitMapElement::with_ref((256, 192), isz, &img).unwrap())?;

  let fgbg = vec![&PURPLE, &PINK];
  let fb = vec![&AMBER, &TEAL]; // foreground background (to be transparent)
  let pals = vec![ // 0:M 1:L 2:Q2 3:Q1 4:C3 5:C2 6:C1 7:o[last]
    vec![&RED, &RED, &RED, &PURPLE, &RED, &PURPLE, &PURPLE, &BLACK],
    vec![&BLUE, &BLUE, &BLUE, &INDIGO, &BLUE, &INDIGO, &INDIGO, &BLACK]];
  let sc = sm.x_scale as f64 / 300000.0; // scale
  let p = 16; // pitch
  ftm.draw_str_glyph(&bm, &fgbg, &pals, false, "ゐむみ", ((32, 512), sc, p))?;
  ftm.draw_str_glyph(&bm, &fb, &pals, true, "WAあ個", ((0, 384 - 60), sc, p))?;
  ftm.draw_str_glyph(&bm, &fb, &pals, true, "gぬQ鬱", ((0, 768 - 60), sc, p))?;

  bm.present()?;
  drop(bm);

  let im = BitMapBackend::new("./img/_4c_fonts.png",
    wsz).into_drawing_area();
  im.draw(&BitMapElement::with_ref((0, 0), wsz, buf).unwrap())?;
  im.present()?;
  drop(im);

  Ok(())
}

