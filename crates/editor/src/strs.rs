use teamim::DrawProgress;

pub const fn draw_progress(progress: DrawProgress) -> &'static str {
    match progress {
        DrawProgress::Pending => "מצייר...",
        DrawProgress::Recognizing => "מזהה...",
        DrawProgress::ImageEffects => "עורך תמונה...",
        DrawProgress::Searching => "מחפש...",
        DrawProgress::Diffing => "משווה...",
        DrawProgress::Placing => "ממקם...",
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
