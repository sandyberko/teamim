use std::{cell::LazyCell, thread::LocalKey};

use leptess::leptonica::Pix;
use phf::{Map, phf_map};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    Top,
    Bottom,
    After,
}

#[derive(Clone, Copy)]
pub struct Glyph {
    pub name: &'static str,
    pub placement: Placement,
    pub pix: &'static LocalKey<LazyCell<Pix>>,
}

impl Glyph {
    const fn new(
        name: &'static str,
        placement: Placement,
        pix: &'static LocalKey<LazyCell<Pix>>,
    ) -> Self {
        Self {
            name,
            placement,
            pix,
        }
    }
}

macro_rules! glyph {
    ($name:expr, $placement:expr) => {{
        thread_local! {
            static PIX: LazyCell<Pix> = LazyCell::new(|| {
                let buf = include_bytes!(concat!("../assets/glyphs/", $name, ".tif"));
                leptess::leptonica::pix_read_mem(buf)
                    .expect(concat!("failed to load ", $name))
            });
        }
        Glyph::new($name, $placement, &PIX)
    }};
}

// keisarim
// sof_pasuq is meteg
pub static ETNAHTA: Glyph = glyph!("etnahta", Placement::Bottom);

// melakim
pub static SEGOL: Glyph = glyph!("segol", Placement::Top);
pub static SHALSHELET: Glyph = glyph!("shalshelet", Placement::Top);
pub static ZAQUEF_QATAN: Glyph = glyph!("zaquef_qatan", Placement::Top);
pub static ZAQUEF_GADOL: Glyph = glyph!("zaquef_gadol", Placement::Top);
pub static TIPEHA: Glyph = glyph!("tipeha", Placement::Bottom);

// sheni'im
pub static REVIA: Glyph = glyph!("revia", Placement::Top);
pub static PASHTA: Glyph = glyph!("pashta", Placement::Top);
pub static ZARQA: Glyph = glyph!("zarqa", Placement::Top);
pub static YETIV: Glyph = glyph!("yetiv", Placement::Bottom);
pub static TEVIR: Glyph = glyph!("tevir", Placement::Bottom);

// shalishim
pub static PAZER: Glyph = glyph!("pazer", Placement::Top);
pub static QARNEY_PARA: Glyph = glyph!("qarney_para", Placement::Top);
pub static TELISHA_GEDOLA: Glyph = glyph!("telisha_gedola", Placement::Top);
pub static GERESH: Glyph = glyph!("geresh", Placement::Top);
pub static GERESH_MUQDAM: Glyph = glyph!("geresh_muqdam", Placement::Top);
pub static GERSHAYIM: Glyph = glyph!("gershayim", Placement::Top);

// meshartim
pub static MUNAH: Glyph = glyph!("munah", Placement::Bottom);
pub static MERKHA: Glyph = glyph!("merkha", Placement::Bottom);
pub static MAHAPAKH: Glyph = glyph!("mahapakh", Placement::Bottom);
pub static DARGA: Glyph = glyph!("darga", Placement::Bottom);
pub static QADMA: Glyph = glyph!("qadma", Placement::Top);
pub static TELISHA_KETANA: Glyph = glyph!("telisha_ketana", Placement::Top);
pub static MERKHA_KEFULA: Glyph = glyph!("merkha_kefula", Placement::Bottom);
pub static YERAH_BEN_YOMO: Glyph = glyph!("yerah_ben_yomo", Placement::Bottom);

pub static METEG: Glyph = glyph!("meteg", Placement::Bottom);
pub static MAQAF: Glyph = glyph!("maqaf", Placement::After);
pub static PASEQ: Glyph = glyph!("paseq", Placement::After);

pub static SOF_PASUQ: Glyph = glyph!("sof_pasuq", Placement::After);

pub static GLYPHS: Map<char, Glyph> = phf_map! {
    '\u{0591}' => ETNAHTA,
    '\u{0592}' => SEGOL,
    '\u{0593}' => SHALSHELET,
    '\u{0594}' => ZAQUEF_QATAN,
    '\u{0595}' => ZAQUEF_GADOL,
    '\u{0596}' => TIPEHA,
    '\u{0597}' => REVIA,
    '\u{0598}' => ZARQA,
    '\u{0599}' => PASHTA,
    '\u{059A}' => YETIV,
    '\u{059B}' => TEVIR,
    '\u{059C}' => GERESH,
    '\u{059D}' => GERESH_MUQDAM,
    '\u{059E}' => GERSHAYIM,
    '\u{059F}' => QARNEY_PARA,
    '\u{05A0}' => TELISHA_GEDOLA,
    '\u{05A1}' => PAZER,
    // ---
    '\u{05A3}' => MUNAH,
    '\u{05A4}' => MAHAPAKH,
    '\u{05A5}' => MERKHA,
    '\u{05A6}' => MERKHA_KEFULA,
    '\u{05A7}' => DARGA,
    '\u{05A8}' => QADMA,
    '\u{05A9}' => TELISHA_KETANA,
    '\u{05AA}' => YERAH_BEN_YOMO,
    // ---
    '\u{05BD}' => METEG,
    '\u{05BE}' => MAQAF,
    // ---
    '\u{05C0}' => PASEQ,
    '\u{05C3}' => SOF_PASUQ,

};
