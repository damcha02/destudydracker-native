//! Flags for Flaggle (Stage 20): production's 195 `flag-icons` SVGs (`desktop/src/assets/flags`,
//! MIT, lipis/flag-icons), stored gzip-compressed (SVGZ; usvg decompresses them) and touched only
//! when Flaggle needs them:
//!
//! * thumbnails (dropdown, history) are Slint vector images, rendered and cached by the renderer at
//!   their 72 x 54 display size;
//! * the colour comparison rasterizes flags exactly like production's canvas: drawn into a 240 x 180
//!   RGBA buffer, compared pixel by pixel (`study_tracker_core::break_room::flaggle`).
//!
//! Chromium (Skia) and resvg (tiny-skia) antialias edges slightly differently, so a similarity can
//! differ from production's in the first decimal; the rules and the rasters' size are the same.

use std::cell::RefCell;
use std::collections::HashMap;

use resvg::{tiny_skia, usvg};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use study_tracker_core::break_room::countries::find_country;
use study_tracker_core::break_room::flaggle::{reveal, similarity, FLAG_HEIGHT, FLAG_WIDTH};

use crate::break_room_controller::FlagSimilarity;

fn bytes(iso2_lower: &str) -> Option<&'static [u8]> {
    Some(match iso2_lower {
        "ad" => include_bytes!("../assets/break/flags/ad.svgz"),
        "ae" => include_bytes!("../assets/break/flags/ae.svgz"),
        "af" => include_bytes!("../assets/break/flags/af.svgz"),
        "ag" => include_bytes!("../assets/break/flags/ag.svgz"),
        "al" => include_bytes!("../assets/break/flags/al.svgz"),
        "am" => include_bytes!("../assets/break/flags/am.svgz"),
        "ao" => include_bytes!("../assets/break/flags/ao.svgz"),
        "ar" => include_bytes!("../assets/break/flags/ar.svgz"),
        "at" => include_bytes!("../assets/break/flags/at.svgz"),
        "au" => include_bytes!("../assets/break/flags/au.svgz"),
        "az" => include_bytes!("../assets/break/flags/az.svgz"),
        "ba" => include_bytes!("../assets/break/flags/ba.svgz"),
        "bb" => include_bytes!("../assets/break/flags/bb.svgz"),
        "bd" => include_bytes!("../assets/break/flags/bd.svgz"),
        "be" => include_bytes!("../assets/break/flags/be.svgz"),
        "bf" => include_bytes!("../assets/break/flags/bf.svgz"),
        "bg" => include_bytes!("../assets/break/flags/bg.svgz"),
        "bh" => include_bytes!("../assets/break/flags/bh.svgz"),
        "bi" => include_bytes!("../assets/break/flags/bi.svgz"),
        "bj" => include_bytes!("../assets/break/flags/bj.svgz"),
        "bn" => include_bytes!("../assets/break/flags/bn.svgz"),
        "bo" => include_bytes!("../assets/break/flags/bo.svgz"),
        "br" => include_bytes!("../assets/break/flags/br.svgz"),
        "bs" => include_bytes!("../assets/break/flags/bs.svgz"),
        "bt" => include_bytes!("../assets/break/flags/bt.svgz"),
        "bw" => include_bytes!("../assets/break/flags/bw.svgz"),
        "by" => include_bytes!("../assets/break/flags/by.svgz"),
        "bz" => include_bytes!("../assets/break/flags/bz.svgz"),
        "ca" => include_bytes!("../assets/break/flags/ca.svgz"),
        "cd" => include_bytes!("../assets/break/flags/cd.svgz"),
        "cf" => include_bytes!("../assets/break/flags/cf.svgz"),
        "cg" => include_bytes!("../assets/break/flags/cg.svgz"),
        "ch" => include_bytes!("../assets/break/flags/ch.svgz"),
        "ci" => include_bytes!("../assets/break/flags/ci.svgz"),
        "cl" => include_bytes!("../assets/break/flags/cl.svgz"),
        "cm" => include_bytes!("../assets/break/flags/cm.svgz"),
        "cn" => include_bytes!("../assets/break/flags/cn.svgz"),
        "co" => include_bytes!("../assets/break/flags/co.svgz"),
        "cr" => include_bytes!("../assets/break/flags/cr.svgz"),
        "cu" => include_bytes!("../assets/break/flags/cu.svgz"),
        "cv" => include_bytes!("../assets/break/flags/cv.svgz"),
        "cy" => include_bytes!("../assets/break/flags/cy.svgz"),
        "cz" => include_bytes!("../assets/break/flags/cz.svgz"),
        "de" => include_bytes!("../assets/break/flags/de.svgz"),
        "dj" => include_bytes!("../assets/break/flags/dj.svgz"),
        "dk" => include_bytes!("../assets/break/flags/dk.svgz"),
        "dm" => include_bytes!("../assets/break/flags/dm.svgz"),
        "do" => include_bytes!("../assets/break/flags/do.svgz"),
        "dz" => include_bytes!("../assets/break/flags/dz.svgz"),
        "ec" => include_bytes!("../assets/break/flags/ec.svgz"),
        "ee" => include_bytes!("../assets/break/flags/ee.svgz"),
        "eg" => include_bytes!("../assets/break/flags/eg.svgz"),
        "er" => include_bytes!("../assets/break/flags/er.svgz"),
        "es" => include_bytes!("../assets/break/flags/es.svgz"),
        "et" => include_bytes!("../assets/break/flags/et.svgz"),
        "fi" => include_bytes!("../assets/break/flags/fi.svgz"),
        "fj" => include_bytes!("../assets/break/flags/fj.svgz"),
        "fm" => include_bytes!("../assets/break/flags/fm.svgz"),
        "fr" => include_bytes!("../assets/break/flags/fr.svgz"),
        "ga" => include_bytes!("../assets/break/flags/ga.svgz"),
        "gb" => include_bytes!("../assets/break/flags/gb.svgz"),
        "gd" => include_bytes!("../assets/break/flags/gd.svgz"),
        "ge" => include_bytes!("../assets/break/flags/ge.svgz"),
        "gh" => include_bytes!("../assets/break/flags/gh.svgz"),
        "gm" => include_bytes!("../assets/break/flags/gm.svgz"),
        "gn" => include_bytes!("../assets/break/flags/gn.svgz"),
        "gq" => include_bytes!("../assets/break/flags/gq.svgz"),
        "gr" => include_bytes!("../assets/break/flags/gr.svgz"),
        "gt" => include_bytes!("../assets/break/flags/gt.svgz"),
        "gw" => include_bytes!("../assets/break/flags/gw.svgz"),
        "gy" => include_bytes!("../assets/break/flags/gy.svgz"),
        "hn" => include_bytes!("../assets/break/flags/hn.svgz"),
        "hr" => include_bytes!("../assets/break/flags/hr.svgz"),
        "ht" => include_bytes!("../assets/break/flags/ht.svgz"),
        "hu" => include_bytes!("../assets/break/flags/hu.svgz"),
        "id" => include_bytes!("../assets/break/flags/id.svgz"),
        "ie" => include_bytes!("../assets/break/flags/ie.svgz"),
        "il" => include_bytes!("../assets/break/flags/il.svgz"),
        "in" => include_bytes!("../assets/break/flags/in.svgz"),
        "iq" => include_bytes!("../assets/break/flags/iq.svgz"),
        "ir" => include_bytes!("../assets/break/flags/ir.svgz"),
        "is" => include_bytes!("../assets/break/flags/is.svgz"),
        "it" => include_bytes!("../assets/break/flags/it.svgz"),
        "jm" => include_bytes!("../assets/break/flags/jm.svgz"),
        "jo" => include_bytes!("../assets/break/flags/jo.svgz"),
        "jp" => include_bytes!("../assets/break/flags/jp.svgz"),
        "ke" => include_bytes!("../assets/break/flags/ke.svgz"),
        "kg" => include_bytes!("../assets/break/flags/kg.svgz"),
        "kh" => include_bytes!("../assets/break/flags/kh.svgz"),
        "ki" => include_bytes!("../assets/break/flags/ki.svgz"),
        "km" => include_bytes!("../assets/break/flags/km.svgz"),
        "kn" => include_bytes!("../assets/break/flags/kn.svgz"),
        "kp" => include_bytes!("../assets/break/flags/kp.svgz"),
        "kr" => include_bytes!("../assets/break/flags/kr.svgz"),
        "kw" => include_bytes!("../assets/break/flags/kw.svgz"),
        "kz" => include_bytes!("../assets/break/flags/kz.svgz"),
        "la" => include_bytes!("../assets/break/flags/la.svgz"),
        "lb" => include_bytes!("../assets/break/flags/lb.svgz"),
        "lc" => include_bytes!("../assets/break/flags/lc.svgz"),
        "li" => include_bytes!("../assets/break/flags/li.svgz"),
        "lk" => include_bytes!("../assets/break/flags/lk.svgz"),
        "lr" => include_bytes!("../assets/break/flags/lr.svgz"),
        "ls" => include_bytes!("../assets/break/flags/ls.svgz"),
        "lt" => include_bytes!("../assets/break/flags/lt.svgz"),
        "lu" => include_bytes!("../assets/break/flags/lu.svgz"),
        "lv" => include_bytes!("../assets/break/flags/lv.svgz"),
        "ly" => include_bytes!("../assets/break/flags/ly.svgz"),
        "ma" => include_bytes!("../assets/break/flags/ma.svgz"),
        "mc" => include_bytes!("../assets/break/flags/mc.svgz"),
        "md" => include_bytes!("../assets/break/flags/md.svgz"),
        "me" => include_bytes!("../assets/break/flags/me.svgz"),
        "mg" => include_bytes!("../assets/break/flags/mg.svgz"),
        "mh" => include_bytes!("../assets/break/flags/mh.svgz"),
        "mk" => include_bytes!("../assets/break/flags/mk.svgz"),
        "ml" => include_bytes!("../assets/break/flags/ml.svgz"),
        "mm" => include_bytes!("../assets/break/flags/mm.svgz"),
        "mn" => include_bytes!("../assets/break/flags/mn.svgz"),
        "mr" => include_bytes!("../assets/break/flags/mr.svgz"),
        "mt" => include_bytes!("../assets/break/flags/mt.svgz"),
        "mu" => include_bytes!("../assets/break/flags/mu.svgz"),
        "mv" => include_bytes!("../assets/break/flags/mv.svgz"),
        "mw" => include_bytes!("../assets/break/flags/mw.svgz"),
        "mx" => include_bytes!("../assets/break/flags/mx.svgz"),
        "my" => include_bytes!("../assets/break/flags/my.svgz"),
        "mz" => include_bytes!("../assets/break/flags/mz.svgz"),
        "na" => include_bytes!("../assets/break/flags/na.svgz"),
        "ne" => include_bytes!("../assets/break/flags/ne.svgz"),
        "ng" => include_bytes!("../assets/break/flags/ng.svgz"),
        "ni" => include_bytes!("../assets/break/flags/ni.svgz"),
        "nl" => include_bytes!("../assets/break/flags/nl.svgz"),
        "no" => include_bytes!("../assets/break/flags/no.svgz"),
        "np" => include_bytes!("../assets/break/flags/np.svgz"),
        "nr" => include_bytes!("../assets/break/flags/nr.svgz"),
        "nz" => include_bytes!("../assets/break/flags/nz.svgz"),
        "om" => include_bytes!("../assets/break/flags/om.svgz"),
        "pa" => include_bytes!("../assets/break/flags/pa.svgz"),
        "pe" => include_bytes!("../assets/break/flags/pe.svgz"),
        "pg" => include_bytes!("../assets/break/flags/pg.svgz"),
        "ph" => include_bytes!("../assets/break/flags/ph.svgz"),
        "pk" => include_bytes!("../assets/break/flags/pk.svgz"),
        "pl" => include_bytes!("../assets/break/flags/pl.svgz"),
        "pt" => include_bytes!("../assets/break/flags/pt.svgz"),
        "pw" => include_bytes!("../assets/break/flags/pw.svgz"),
        "py" => include_bytes!("../assets/break/flags/py.svgz"),
        "qa" => include_bytes!("../assets/break/flags/qa.svgz"),
        "ro" => include_bytes!("../assets/break/flags/ro.svgz"),
        "rs" => include_bytes!("../assets/break/flags/rs.svgz"),
        "ru" => include_bytes!("../assets/break/flags/ru.svgz"),
        "rw" => include_bytes!("../assets/break/flags/rw.svgz"),
        "sa" => include_bytes!("../assets/break/flags/sa.svgz"),
        "sb" => include_bytes!("../assets/break/flags/sb.svgz"),
        "sc" => include_bytes!("../assets/break/flags/sc.svgz"),
        "sd" => include_bytes!("../assets/break/flags/sd.svgz"),
        "se" => include_bytes!("../assets/break/flags/se.svgz"),
        "sg" => include_bytes!("../assets/break/flags/sg.svgz"),
        "si" => include_bytes!("../assets/break/flags/si.svgz"),
        "sk" => include_bytes!("../assets/break/flags/sk.svgz"),
        "sl" => include_bytes!("../assets/break/flags/sl.svgz"),
        "sm" => include_bytes!("../assets/break/flags/sm.svgz"),
        "sn" => include_bytes!("../assets/break/flags/sn.svgz"),
        "so" => include_bytes!("../assets/break/flags/so.svgz"),
        "sr" => include_bytes!("../assets/break/flags/sr.svgz"),
        "ss" => include_bytes!("../assets/break/flags/ss.svgz"),
        "st" => include_bytes!("../assets/break/flags/st.svgz"),
        "sv" => include_bytes!("../assets/break/flags/sv.svgz"),
        "sy" => include_bytes!("../assets/break/flags/sy.svgz"),
        "sz" => include_bytes!("../assets/break/flags/sz.svgz"),
        "td" => include_bytes!("../assets/break/flags/td.svgz"),
        "tg" => include_bytes!("../assets/break/flags/tg.svgz"),
        "th" => include_bytes!("../assets/break/flags/th.svgz"),
        "tj" => include_bytes!("../assets/break/flags/tj.svgz"),
        "tl" => include_bytes!("../assets/break/flags/tl.svgz"),
        "tm" => include_bytes!("../assets/break/flags/tm.svgz"),
        "tn" => include_bytes!("../assets/break/flags/tn.svgz"),
        "to" => include_bytes!("../assets/break/flags/to.svgz"),
        "tr" => include_bytes!("../assets/break/flags/tr.svgz"),
        "tt" => include_bytes!("../assets/break/flags/tt.svgz"),
        "tv" => include_bytes!("../assets/break/flags/tv.svgz"),
        "tw" => include_bytes!("../assets/break/flags/tw.svgz"),
        "tz" => include_bytes!("../assets/break/flags/tz.svgz"),
        "ua" => include_bytes!("../assets/break/flags/ua.svgz"),
        "ug" => include_bytes!("../assets/break/flags/ug.svgz"),
        "us" => include_bytes!("../assets/break/flags/us.svgz"),
        "uy" => include_bytes!("../assets/break/flags/uy.svgz"),
        "uz" => include_bytes!("../assets/break/flags/uz.svgz"),
        "va" => include_bytes!("../assets/break/flags/va.svgz"),
        "vc" => include_bytes!("../assets/break/flags/vc.svgz"),
        "ve" => include_bytes!("../assets/break/flags/ve.svgz"),
        "vn" => include_bytes!("../assets/break/flags/vn.svgz"),
        "vu" => include_bytes!("../assets/break/flags/vu.svgz"),
        "ws" => include_bytes!("../assets/break/flags/ws.svgz"),
        "ye" => include_bytes!("../assets/break/flags/ye.svgz"),
        "za" => include_bytes!("../assets/break/flags/za.svgz"),
        "zm" => include_bytes!("../assets/break/flags/zm.svgz"),
        "zw" => include_bytes!("../assets/break/flags/zw.svgz"),
        _ => return None,
    })
}

fn svgz_for(country: &str) -> Option<&'static [u8]> {
    bytes(&find_country(country)?.iso2.to_ascii_lowercase())
}

thread_local! {
    static THUMBS: RefCell<HashMap<String, Image>> = RefCell::new(HashMap::new());
    static RASTERS: RefCell<HashMap<String, std::rc::Rc<Vec<u8>>>> = RefCell::new(HashMap::new());
}

/// The flag as a (cached) Slint vector image; empty for an unknown country.
pub fn thumbnail(country: &str) -> Image {
    THUMBS.with(|t| {
        if let Some(i) = t.borrow().get(country) {
            return i.clone();
        }
        let image = svgz_for(country)
            .and_then(|b| Image::load_from_svg_data(b).ok())
            .unwrap_or_default();
        t.borrow_mut().insert(country.to_string(), image.clone());
        image
    })
}

/// `flagUrlToImageData`: the flag drawn into a 240 x 180 canvas, as straight-alpha RGBA8.
pub fn raster(country: &str) -> Option<std::rc::Rc<Vec<u8>>> {
    if let Some(r) = RASTERS.with(|r| r.borrow().get(country).cloned()) {
        return Some(r);
    }
    let tree = usvg::Tree::from_data(svgz_for(country)?, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(FLAG_WIDTH, FLAG_HEIGHT)?;
    let size = tree.size();
    let transform = tiny_skia::Transform::from_scale(
        FLAG_WIDTH as f32 / size.width(),
        FLAG_HEIGHT as f32 / size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // tiny-skia is premultiplied; canvas getImageData is not
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    let rc = std::rc::Rc::new(rgba);
    RASTERS.with(|r| {
        let mut r = r.borrow_mut();
        // keep the cache small: the answer plus at most a handful of guesses
        if r.len() > 12 {
            r.clear();
        }
        r.insert(country.to_string(), rc.clone());
    });
    Some(rc)
}

/// `revealTargetFlagByGuesses` as an image (the whole flag when `full`).
pub fn preview(answer: &str, guesses: &[String], full: bool) -> Option<Image> {
    let target = raster(answer)?;
    let pixels = if full {
        target.as_ref().clone()
    } else {
        let rasters: Vec<_> = guesses.iter().filter_map(|g| raster(g)).collect();
        let refs: Vec<&[u8]> = rasters.iter().map(|r| r.as_slice()).collect();
        reveal(&target, &refs)
    };
    Some(Image::from_rgba8(
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&pixels, FLAG_WIDTH, FLAG_HEIGHT),
    ))
}

/// The app's [`FlagSimilarity`]: production's `maskFlagByTargetColors` similarity.
pub struct ResvgFlags;

impl FlagSimilarity for ResvgFlags {
    fn similarity(&mut self, guess: &str, answer: &str) -> Option<f64> {
        Some(similarity(&raster(answer)?, &raster(guess)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::break_room::countries::countries;

    #[test]
    fn every_country_has_a_flag_that_rasterizes() {
        for c in countries() {
            let r = raster(c.name).unwrap_or_else(|| panic!("{}", c.name));
            assert_eq!(r.len(), (FLAG_WIDTH * FLAG_HEIGHT * 4) as usize);
            assert!(
                r.chunks_exact(4).filter(|p| p[3] >= 128).count() > 10_000,
                "{} draws something (Nepal is not rectangular)",
                c.name
            );
        }
    }

    #[test]
    fn similarity_behaves_like_production() {
        let mut flags = ResvgFlags;
        assert_eq!(flags.similarity("Japan", "Japan"), Some(100.0));
        // Italy and Ireland share green/white bands; Japan and Chad share nothing but white-ish red
        let it_ie = flags.similarity("Italy", "Ireland").unwrap();
        assert!(it_ie > 30.0, "{it_ie}");
        let jp_td = flags.similarity("Japan", "Chad").unwrap();
        assert!(jp_td < it_ie);
        assert!(flags.similarity("Atlantis", "Japan").is_none());
    }
}
