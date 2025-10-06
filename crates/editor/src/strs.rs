use std::borrow::Cow;

use iced::widget::text::{Fragment, IntoFragment};
use teamim::PositStatus;

use crate::{
    SelectProgress,
    task::{Poll, Progress},
};

impl<Ready> IntoFragment<'static> for &Poll<Ready, Progress<PositStatus>> {
    fn into_fragment(self) -> Fragment<'static> {
        Cow::Borrowed(match self {
            Poll::Ready(_) => "צייר טעמים",
            Poll::Pending(progress) => match progress.status {
                PositStatus::Pending => "מצייר...",
                PositStatus::Recognizing => "מזהה...",
                PositStatus::ImageEffects => "עורך תמונה...",
                PositStatus::Searching => "מחפש...",
                PositStatus::Diffing => "משווה...",
                PositStatus::Placing => "ממקם...",
            },
        })
    }
}

impl<Ready> IntoFragment<'static> for &Poll<Ready, Progress<SelectProgress>> {
    fn into_fragment(self) -> Fragment<'static> {
        Cow::Borrowed(SELECT_IMG)
    }
}

pub const TITLE: &str = "טעמים";
pub const SELECT_IMG: &str = "בחר תמונה";
pub const SAVE_FAILED: &str = "שמירה נכשלה";
pub const ERROR: &str = "שגיאה";
pub const LOADING: &str = "טוען...";
pub const SAVE: &str = "שמור";
pub const NO_IMG_SELECTED: &str = "לא נבחרה תמונה";
pub const BLUR: &str = "טשטוש";
