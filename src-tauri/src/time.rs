use chrono::{DateTime, FixedOffset, Utc};

const SHANGHAI_OFFSET_SECONDS: i32 = 8 * 60 * 60;

pub fn shanghai_now() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(
        &FixedOffset::east_opt(SHANGHAI_OFFSET_SECONDS).expect("Shanghai UTC offset must be valid"),
    )
}

pub fn shanghai_rfc3339() -> String {
    shanghai_now().to_rfc3339()
}

pub fn shanghai_clock() -> String {
    shanghai_now().format("%H:%M:%S").to_string()
}

pub fn shanghai_file_stamp() -> String {
    shanghai_now().format("%Y%m%d_%H%M%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_shanghai_offset() {
        let now = shanghai_now();
        assert_eq!(now.offset().local_minus_utc(), 8 * 60 * 60);
        assert!(shanghai_rfc3339().ends_with("+08:00"));
    }
}
