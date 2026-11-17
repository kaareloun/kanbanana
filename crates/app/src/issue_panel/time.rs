//! Relative times: "now", "40 min ago", "2 h ago", "yesterday", then dates like "12 Nov".

use chrono::{DateTime, Datelike, Local, Utc};

pub fn relative(t: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - t).num_seconds().max(0);
    if secs < 60 {
        return "now".into();
    }
    if secs < 3600 {
        return format!("{} min ago", secs / 60);
    }
    let (lt, ln) = (t.with_timezone(&Local), now.with_timezone(&Local));
    if secs < 86_400 && lt.date_naive() == ln.date_naive() {
        return format!("{} h ago", secs / 3600);
    }
    if ln.date_naive().pred_opt() == Some(lt.date_naive()) {
        return "yesterday".into();
    }
    if secs < 86_400 {
        return format!("{} h ago", secs / 3600);
    }
    if lt.year() == ln.year() {
        lt.format("%-d %b").to_string()
    } else {
        lt.format("%-d %b %Y").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn noon() -> DateTime<Utc> {
        Local.with_ymd_and_hms(2026, 11, 17, 12, 0, 0).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn minutes_and_hours() {
        let now = noon();
        assert_eq!(relative(now - Duration::seconds(20), now), "now");
        assert_eq!(relative(now - Duration::minutes(40), now), "40 min ago");
        assert_eq!(relative(now - Duration::hours(2), now), "2 h ago");
    }

    #[test]
    fn yesterday_and_dates() {
        let now = noon();
        assert_eq!(relative(now - Duration::hours(26), now), "yesterday");
        assert_eq!(relative(now - Duration::days(5), now), "12 Nov");
        assert_eq!(relative(now - Duration::days(400), now), "13 Oct 2025");
    }

    #[test]
    fn future_times_count_as_now() {
        let now = noon();
        assert_eq!(relative(now + Duration::minutes(5), now), "now");
    }
}
