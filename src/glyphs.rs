use eyre::eyre;
use std::{cell::LazyCell, thread::LocalKey};

use leptess::leptonica::Pix;
use phf::{phf_map, Map};

macro_rules! glyph {
    ($var:ident, $file:expr) => {
        thread_local! {
            static $var: LazyCell<(Pix, &'static str)> = LazyCell::new(|| {
                let buf = include_bytes!(concat!("../assets/glyphs/", $file, ".tif"));
                let pix = leptess::leptonica::pix_read_mem(buf).expect(concat!("failed to load ", $file));
                (pix, $file)
            });
        }
    };
}

glyph!(ETNAHTA, "etnahta");
glyph!(SEGOL, "segol");
glyph!(SHALSHELET, "shalshelet");
glyph!(ZAQUEF_QATAN, "zaquef_qatan");
glyph!(ZAQUEF_GADOL, "zaquef_gadol");
glyph!(TIPEHA, "tipeha");
glyph!(REVIA, "revia");
glyph!(ZARQA, "zarqa");
glyph!(PASHTA, "pashta");
glyph!(YETIV, "yetiv");
glyph!(TEVIR, "tevir");
glyph!(GERESH, "geresh");
glyph!(GERESH_MUQDAM, "geresh_muqdam");
glyph!(GERSHAYIM, "gershayim");
glyph!(QARNEY_PARA, "qarney_para");
glyph!(TELISHA_GEDOLA, "telisha_gedola");
glyph!(PAZER, "pazer");
glyph!(MUNAH, "munah");
glyph!(MAHAPAKH, "mahapakh");
glyph!(MERKHA, "merkha");
glyph!(MERKHA_KEFULA, "merkha_kefula");
glyph!(DARGA, "darga");
glyph!(QADMA, "qadma");
glyph!(TELISHA_KETANA, "telisha_ketana");
glyph!(YERAH_BEN_YOMO, "yerah_ben_yomo");
glyph!(OLEH, "oleh");
glyph!(ILUY, "iluy");
glyph!(DEHI, "dehi");
glyph!(METEG, "meteg");
glyph!(MAQAF, "maqaf");
glyph!(SOF_PASUQ, "sof_pasuq");

static GLYPHS: Map<char, LocalKey<LazyCell<(Pix, &'static str)>>> = phf_map! {
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

pub(crate) fn with_glyph<T>(c: char, f: impl FnOnce(&Pix, &'static str) -> T) -> eyre::Result<T> {
    let Some(glyph) = GLYPHS.get(&c) else {
        return Err(eyre!("no glyph for {c:?}"));
    };
    Ok(glyph.with(|pix| f(&pix.0, pix.1)))
}
