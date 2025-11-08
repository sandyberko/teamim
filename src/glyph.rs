use phf::{Map, phf_map};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CombiningClass {
    Above,
    Bottom,
    After,
}

#[derive(Clone, Copy)]
pub struct Glyph {
    pub diac: char,
    pub name: &'static str,
    pub combining_class: CombiningClass,
}

impl Glyph {
    const fn new(diac: char, name: &'static str, placement: CombiningClass) -> Self {
        Self { diac, name, combining_class: placement }
    }
}

macro_rules! glyph {
    ($diac:expr, $name:expr, $placement:expr) => {
        Glyph::new($diac, $name, $placement)
    };
}

pub mod diacs {
    use {super::Glyph, crate::glyph::CombiningClass};

    // keisarim
    // sof_pasuq is meteg
    pub const ETNAHTA: Glyph = glyph!('\u{0591}', "etnahta", CombiningClass::Bottom);

    // melakim
    pub const SEGOL: Glyph = glyph!('\u{0592}', "segol", CombiningClass::Above);
    pub const SHALSHELET: Glyph = glyph!('\u{0593}', "shalshelet", CombiningClass::Above);
    pub const ZAQUEF_QATAN: Glyph = glyph!('\u{0594}', "zaquef_qatan", CombiningClass::Above);
    pub const ZAQUEF_GADOL: Glyph = glyph!('\u{0595}', "zaquef_gadol", CombiningClass::Above);
    pub const TIPEHA: Glyph = glyph!('\u{0596}', "tipeha", CombiningClass::Bottom);

    // sheni'im
    pub const REVIA: Glyph = glyph!('\u{0597}', "revia", CombiningClass::Above);
    pub const PASHTA: Glyph = glyph!('\u{0598}', "pashta", CombiningClass::Above);
    pub const ZARQA: Glyph = glyph!('\u{0599}', "zarqa", CombiningClass::Above);
    pub const YETIV: Glyph = glyph!('\u{059A}', "yetiv", CombiningClass::Bottom);
    pub const TEVIR: Glyph = glyph!('\u{059B}', "tevir", CombiningClass::Bottom);

    // shalishim
    pub const PAZER: Glyph = glyph!('\u{059C}', "pazer", CombiningClass::Above);
    pub const QARNEY_PARA: Glyph = glyph!('\u{059D}', "qarney_para", CombiningClass::Above);
    pub const TELISHA_GEDOLA: Glyph = glyph!('\u{059E}', "telisha_gedola", CombiningClass::Above);
    pub const GERESH: Glyph = glyph!('\u{059F}', "geresh", CombiningClass::Above);
    pub const GERESH_MUQDAM: Glyph = glyph!('\u{05A0}', "geresh_muqdam", CombiningClass::Above);
    pub const GERSHAYIM: Glyph = glyph!('\u{05A1}', "gershayim", CombiningClass::Above);

    // meshartim
    pub const MUNAH: Glyph = glyph!('\u{05A3}', "munah", CombiningClass::Bottom);
    pub const MERKHA: Glyph = glyph!('\u{05A4}', "merkha", CombiningClass::Bottom);
    pub const MAHAPAKH: Glyph = glyph!('\u{05A5}', "mahapakh", CombiningClass::Bottom);
    pub const DARGA: Glyph = glyph!('\u{05A6}', "darga", CombiningClass::Bottom);
    pub const QADMA: Glyph = glyph!('\u{05A7}', "qadma", CombiningClass::Above);
    pub const TELISHA_KETANA: Glyph = glyph!('\u{05A8}', "telisha_ketana", CombiningClass::Above);
    pub const MERKHA_KEFULA: Glyph = glyph!('\u{05A9}', "merkha_kefula", CombiningClass::Bottom);
    pub const YERAH_BEN_YOMO: Glyph = glyph!('\u{05AA}', "yerah_ben_yomo", CombiningClass::Bottom);
    pub const METEG: Glyph = glyph!('\u{05BD}', "meteg", CombiningClass::Bottom);
    pub const MAQAF: Glyph = glyph!('\u{05BE}', "maqaf", CombiningClass::After);
    pub const PASEQ: Glyph = glyph!('\u{05C0}', "paseq", CombiningClass::After);
    pub const SOF_PASUQ: Glyph = glyph!('\u{05C3}', "sof_pasuq", CombiningClass::After);
}

pub static GLYPHS: Map<char, Glyph> = phf_map! {
    '\u{0591}' => diacs::ETNAHTA,
    '\u{0592}' => diacs::SEGOL,
    '\u{0593}' => diacs::SHALSHELET,
    '\u{0594}' => diacs::ZAQUEF_QATAN,
    '\u{0595}' => diacs::ZAQUEF_GADOL,
    '\u{0596}' => diacs::TIPEHA,
    '\u{0597}' => diacs::REVIA,
    '\u{0598}' => diacs::ZARQA,
    '\u{0599}' => diacs::PASHTA,
    '\u{059A}' => diacs::YETIV,
    '\u{059B}' => diacs::TEVIR,
    '\u{059C}' => diacs::GERESH,
    '\u{059D}' => diacs::GERESH_MUQDAM,
    '\u{059E}' => diacs::GERSHAYIM,
    '\u{059F}' => diacs::QARNEY_PARA,
    '\u{05A0}' => diacs::TELISHA_GEDOLA,
    '\u{05A1}' => diacs::PAZER,
    // ---
    '\u{05A3}' => diacs::MUNAH,
    '\u{05A4}' => diacs::MAHAPAKH,
    '\u{05A5}' => diacs::MERKHA,
    '\u{05A6}' => diacs::MERKHA_KEFULA,
    '\u{05A7}' => diacs::DARGA,
    '\u{05A8}' => diacs::QADMA,
    '\u{05A9}' => diacs::TELISHA_KETANA,
    '\u{05AA}' => diacs::YERAH_BEN_YOMO,
    // ---
    '\u{05BD}' => diacs::METEG,
    '\u{05BE}' => diacs::MAQAF,
    // ---
    '\u{05C0}' => diacs::PASEQ,
    '\u{05C3}' => diacs::SOF_PASUQ,

};
