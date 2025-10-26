
use phf::{Map, phf_map};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    Top,
    Bottom,
    After,
}

#[derive(Clone, Copy)]
pub struct Glyph {
    pub diac: char,
    pub name: &'static str,
    pub placement: Placement,
}

impl Glyph {
    const fn new(diac: char, name: &'static str, placement: Placement) -> Self {
        Self { diac, name, placement }
    }
}

macro_rules! glyph {
    ($diac:expr, $name:expr, $placement:expr) => {
        Glyph::new($diac, $name, $placement)
    };
}

// keisarim
// sof_pasuq is meteg
pub static ETNAHTA: Glyph = glyph!('\u{0591}', "etnahta", Placement::Bottom);

// melakim
pub static SEGOL: Glyph = glyph!('\u{0592}', "segol", Placement::Top);
pub static SHALSHELET: Glyph = glyph!('\u{0593}', "shalshelet", Placement::Top);
pub static ZAQUEF_QATAN: Glyph = glyph!('\u{0594}', "zaquef_qatan", Placement::Top);
pub static ZAQUEF_GADOL: Glyph = glyph!('\u{0595}', "zaquef_gadol", Placement::Top);
pub static TIPEHA: Glyph = glyph!('\u{0596}', "tipeha", Placement::Bottom);

// sheni'im
pub static REVIA: Glyph = glyph!('\u{0597}', "revia", Placement::Top);
pub static PASHTA: Glyph = glyph!('\u{0598}', "pashta", Placement::Top);
pub static ZARQA: Glyph = glyph!('\u{0599}', "zarqa", Placement::Top);
pub static YETIV: Glyph = glyph!('\u{059A}', "yetiv", Placement::Bottom);
pub static TEVIR: Glyph = glyph!('\u{059B}', "tevir", Placement::Bottom);

// shalishim
pub static PAZER: Glyph = glyph!('\u{059C}', "pazer", Placement::Top);
pub static QARNEY_PARA: Glyph = glyph!('\u{059D}', "qarney_para", Placement::Top);
pub static TELISHA_GEDOLA: Glyph = glyph!('\u{059E}', "telisha_gedola", Placement::Top);
pub static GERESH: Glyph = glyph!('\u{059F}', "geresh", Placement::Top);
pub static GERESH_MUQDAM: Glyph = glyph!('\u{05A0}', "geresh_muqdam", Placement::Top);
pub static GERSHAYIM: Glyph = glyph!('\u{05A1}', "gershayim", Placement::Top);

// meshartim
pub static MUNAH: Glyph = glyph!('\u{05A3}', "munah", Placement::Bottom);
pub static MERKHA: Glyph = glyph!('\u{05A4}', "merkha", Placement::Bottom);
pub static MAHAPAKH: Glyph = glyph!('\u{05A5}', "mahapakh", Placement::Bottom);
pub static DARGA: Glyph = glyph!('\u{05A6}', "darga", Placement::Bottom);
pub static QADMA: Glyph = glyph!('\u{05A7}', "qadma", Placement::Top);
pub static TELISHA_KETANA: Glyph = glyph!('\u{05A8}', "telisha_ketana", Placement::Top);
pub static MERKHA_KEFULA: Glyph = glyph!('\u{05A9}', "merkha_kefula", Placement::Bottom);
pub static YERAH_BEN_YOMO: Glyph = glyph!('\u{05AA}', "yerah_ben_yomo", Placement::Bottom);

pub static METEG: Glyph = glyph!('\u{05BD}', "meteg", Placement::Bottom);
pub static MAQAF: Glyph = glyph!('\u{05BE}', "maqaf", Placement::After);
pub static PASEQ: Glyph = glyph!('\u{05C0}', "paseq", Placement::After);

pub static SOF_PASUQ: Glyph = glyph!('\u{05C3}', "sof_pasuq", Placement::After);

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
