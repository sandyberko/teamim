use teamim::PositProgress;

pub const fn draw_progress(progress: PositProgress) -> &'static str {
    match progress {
        PositProgress::Pending => "מצייר...",
        PositProgress::Recognizing => "מזהה...",
        PositProgress::ImageEffects => "עורך תמונה...",
        PositProgress::Searching => "מחפש...",
        PositProgress::Diffing => "משווה...",
        PositProgress::Placing => "ממקם...",
    }
}
pub const TITLE: &str = "טעמים";
pub const SAVE_FAILED: &str = "שמירה נכשלה";
pub const ERROR: &str = "שגיאה";
pub const LOADING: &str = "טוען...";
pub const SAVE: &str = "שמור";
pub const DRAW_TEAMIM: &str = "צייר טעמים";
pub const NO_IMG_SELECTED: &str = "לא נבחרה תמונה";
pub const SELECT_IMG: &str = "בחר תמונה";
pub const BLUR: &str = "טשטוש";
