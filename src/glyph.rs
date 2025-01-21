use eyre::eyre;
use std::{cell::LazyCell, thread::LocalKey};

use leptess::leptonica::Pix;
use phf::{phf_map, Map};

pub enum Placement {
    Top,
    Bottom,
}

pub struct Glyph {
    pub name: &'static str,
    pub placement: Placement,
    pub pix: Pix,
}

impl Glyph {
    const fn new(name: &'static str, placement: Placement, pix: Pix) -> Self {
        Self {
            name,
            placement,
            pix,
        }
    }
}

macro_rules! glyph {
    ($var:ident, $name:expr, $placement:expr) => {
        thread_local! {
            static $var: LazyCell<Glyph> = LazyCell::new(|| {
                let buf = include_bytes!(concat!("../assets/glyphs/", $name, ".tif"));
                let pix = leptess::leptonica::pix_read_mem(buf).expect(concat!("failed to load ", $name));
                Glyph::new($name, $placement, pix)
            });
        }
    };
}

glyph!(ETNAHTA, "etnahta", Placement::Bottom);
glyph!(SEGOL, "segol", Placement::Bottom);
glyph!(SHALSHELET, "shalshelet", Placement::Bottom);
glyph!(ZAQUEF_QATAN, "zaquef_qatan", Placement::Bottom);
glyph!(ZAQUEF_GADOL, "zaquef_gadol", Placement::Bottom);
glyph!(TIPEHA, "tipeha", Placement::Bottom);
glyph!(REVIA, "revia", Placement::Bottom);
glyph!(ZARQA, "zarqa", Placement::Bottom);
glyph!(PASHTA, "pashta", Placement::Top);
glyph!(YETIV, "yetiv", Placement::Bottom);
glyph!(TEVIR, "tevir", Placement::Bottom);
glyph!(GERESH, "geresh", Placement::Bottom);
glyph!(GERESH_MUQDAM, "geresh_muqdam", Placement::Bottom);
glyph!(GERSHAYIM, "gershayim", Placement::Bottom);
glyph!(QARNEY_PARA, "qarney_para", Placement::Bottom);
glyph!(TELISHA_GEDOLA, "telisha_gedola", Placement::Bottom);
glyph!(PAZER, "pazer", Placement::Bottom);
glyph!(MUNAH, "munah", Placement::Bottom);
glyph!(MAHAPAKH, "mahapakh", Placement::Bottom);
glyph!(MERKHA, "merkha", Placement::Bottom);
glyph!(MERKHA_KEFULA, "merkha_kefula", Placement::Bottom);
glyph!(DARGA, "darga", Placement::Bottom);
glyph!(QADMA, "qadma", Placement::Bottom);
glyph!(TELISHA_KETANA, "telisha_ketana", Placement::Bottom);
glyph!(YERAH_BEN_YOMO, "yerah_ben_yomo", Placement::Bottom);
glyph!(OLEH, "oleh", Placement::Bottom);
glyph!(ILUY, "iluy", Placement::Bottom);
glyph!(DEHI, "dehi", Placement::Bottom);
glyph!(METEG, "meteg", Placement::Bottom);
glyph!(MAQAF, "maqaf", Placement::Bottom);
glyph!(SOF_PASUQ, "sof_pasuq", Placement::Bottom);

static GLYPHS: Map<char, LocalKey<LazyCell<Glyph>>> = phf_map! {
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
    '\u{05AB}' => OLEH,
    '\u{05AC}' => ILUY,
    '\u{05AD}' => DEHI,
    // '\u{05AE}' !("tzinor,
    // ---
    '\u{05BD}' => METEG,
    '\u{05BE}' => MAQAF,
    // ---
    '\u{05C3}' => SOF_PASUQ,

};

pub(crate) fn with_glyph<T>(c: char, f: impl FnOnce(&Glyph) -> T) -> eyre::Result<T> {
    let Some(glyph) = GLYPHS.get(&c) else {
        return Err(eyre!("no glyph for {c:?}"));
    };
    Ok(glyph.with(|glyph| f(glyph)))
}
