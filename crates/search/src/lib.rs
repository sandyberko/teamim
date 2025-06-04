use std::sync::LazyLock;

use suffix::SuffixTable;

static SUFFIX_TABLE: LazyLock<SuffixTable> =
    LazyLock::new(|| SuffixTable::new(include_str!("../../../assets/text/mam/training.txt")));

const SEARCH_CHUNK: usize = 16;

pub fn approx_match(query: &str) -> Option<usize> {
    for (i, kgram) in Kgrams::new(query, SEARCH_CHUNK).enumerate() {
        eprintln!("Searching for kgram: {kgram}");
        // Check if the kgram exists in the suffix table
        if let Some(&pos) = SUFFIX_TABLE.positions(kgram).first() {
            let c_pos = SUFFIX_TABLE.text().char_indices().position(|(i, _)| i == pos as usize)?;
            return Some(c_pos - i * SEARCH_CHUNK);
        }
    }
    None
}

struct Kgrams<'s> {
    k: usize,
    str: &'s str,
}

impl<'s> Kgrams<'s> {
    fn new(str: &'s str, k: usize) -> Self {
        Self { k, str }
    }
}

impl<'s> Iterator for Kgrams<'s> {
    type Item = &'s str;

    fn next(&mut self) -> Option<Self::Item> {
        let end = self.str.char_indices().nth(self.k).map_or(self.str.len(), |(idx, _)| idx);
        if end == 0 {
            return None;
        }
        let kgram = &self.str[..end];
        self.str = &self.str[end..];
        Some(kgram)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_text() {
        let query = "ותלר התבה על פני המים והמים גברו מאד מאד\nעל הארץ ויכסו כל ההרים הגבהים אשר תחת כל\nהשמים חמש עשרה אמה מלמעלה גברו המים\nויכסו ההרים ויגוע כל בשר הרמש על הארץ\nבעוה ובבהמה ובחיה ובכל השרץ השרץ על\nהארץ וכל האדם כל אשר נשמת רוח חיים באפיו\nמכל אשר בחרבה מתו וימח את כל היקום אשר\nעל פני האדמה מאדם עד בהמה עד רמש ועד\nעוף השמים וימחו מן הארץ וישאר ארך נח ואשר\nאתו בתבה ויגברו המים על הארץ חמשים ומאת\nיום ויזכר אלהים את נח ואת כל החיה ואת לעק\nהבהמה אשר אתו בתבה ויעבר אלהים רוח ע\nהארץ וישכו המים ויסכרו מעינת תהום וארבת\nהשמים ויכלא הגשם מן השמים וישבו המים מ\nהארץ הלוך ושוב ויחסרו המים מקצה חמשים\nומאת יום ותנח התבה בחדש השביעי בשבעה\nעשר יום לחדש על הרי אררט והמים היו הלוך\nוחסור עד החדש העשירי בעשירי באחד לחדש\nנראו ראשי ההרים ויהי מקץ ארבעים יום ויפתח\nנח את חלון התבה אשר עשה וישלח את הערב\nויצא יצוא ושוב עד יבשת המים מעל הארץ\nוישלח את היונה מאתו לראות הקלו שרח מעל\nפני האדמה ולא מצאה היונה מנוח לכף רגלה\nותשב אליו אל התבה כי מים על פני כל הארץ\nוישלח ידו ויקחה ויבא אתה אליו אל התבה ויחל\nעוד שבעת ימים אחרים ויסף שלח את היונה מץ\nהתבה ותבא אליו היונה לעת ערב והנה עלה זית\nטרף בפיה וידע נח כי קלו המים מעל הארץ וייחל\nעוד שבעת ימים אחרים וישלח את היונה ולא\nיסהה שוב אליו עוד ויהי באחת ושש מאות שנה\nבראשון באחד לחדש חרבו המים מעל הארץ\nויסר נח את מכסה התבה וירא והנה חרבו פני\nהאדמה ובחדש השני בשבעה ועשרים יו לחדש\nיבשה הארץ וידבר אלהים א\nן נח לאמר צא מן התבה אתה ואשתך ובניך ונשי\nבניך אתך כל החיה אשר אתך מכל בשר בעוף\nץ ובבהמה ובכל הרמש הרמש על הארץ הוצא אתך\nו שרצו בארץ ופרו ורבו על הארץ ויצא נח ובניו\nו אשתו ונשי בניו אתו כל החיה כל הרמש וכ\nהעוף כל רומש על הארץ למשפחתיהם יצאו מן\nהתבה ויבן נח מזבח ליהוה ויקח מכל הבהמה\nהטהרה ומכל העוף הטהור ויעל עלת במזבח\n\n";
        let query = query.trim().replace('\n', " ");

        let Some(pos) = approx_match(&query) else {
            panic!("Expected to find a match for the full text");
        };
        eprintln!("Query: {query}");
        let (b_pos, _) = SUFFIX_TABLE.text().char_indices().nth(pos).unwrap();
        eprintln!("Match: {}", &SUFFIX_TABLE.text()[b_pos..b_pos + query.len()]);
    }
}
