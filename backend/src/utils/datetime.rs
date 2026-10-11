use time::{
    format_description::well_known::Rfc3339, Date, Duration, OffsetDateTime, PrimitiveDateTime,
    Time, UtcOffset,
};

/// Standard default timezone offset for Indian schools (+05:30 IST).
pub const DEFAULT_IST_OFFSET: UtcOffset = match UtcOffset::from_hms(5, 30, 0) {
    Ok(offset) => offset,
    Err(_) => UtcOffset::UTC,
};

/// Date & Time Utility providing standard ISO 8601 / RFC 3339 formatting with "T" separator
/// and timezone-aware conversions across the ERAVAYA ERP backend.
pub struct DateUtils;

#[allow(dead_code)]
impl DateUtils {
    /// Return the current time in UTC.
    pub fn now_utc() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    /// Return the current time in IST (+05:30).
    pub fn now_ist() -> OffsetDateTime {
        Self::now_utc().to_offset(DEFAULT_IST_OFFSET)
    }

    /// Return current time in a specified school timezone or default IST.
    pub fn now_in_timezone(tz: Option<&str>) -> OffsetDateTime {
        let offset = tz
            .and_then(Self::parse_timezone_offset)
            .unwrap_or(DEFAULT_IST_OFFSET);
        Self::now_utc().to_offset(offset)
    }

    /// Return current UTC timestamp in seconds.
    pub fn now_unix_timestamp() -> i64 {
        Self::now_utc().unix_timestamp()
    }

    /// Return current date in school timezone.
    pub fn today_in_timezone(tz: Option<&str>) -> Date {
        Self::now_in_timezone(tz).date()
    }

    /// Convert a timezone name or offset string to a `UtcOffset`.
    pub fn parse_timezone_offset(tz_str: &str) -> Option<UtcOffset> {
        let trimmed = tz_str.trim();
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("UTC")
            || trimmed.eq_ignore_ascii_case("Z")
        {
            return Some(UtcOffset::UTC);
        }

        if trimmed.eq_ignore_ascii_case("Asia/Kolkata")
            || trimmed.eq_ignore_ascii_case("Asia/Calcutta")
            || trimmed.eq_ignore_ascii_case("IST")
        {
            return Some(DEFAULT_IST_OFFSET);
        }

        // Try parsing ±HH:MM or ±HHMM
        if trimmed.starts_with('+') || trimmed.starts_with('-') {
            let sign = if trimmed.starts_with('-') { -1 } else { 1 };
            let raw = &trimmed[1..];
            let parts: Vec<&str> = raw.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(h), Ok(m)) = (parts[0].parse::<i8>(), parts[1].parse::<i8>()) {
                    return UtcOffset::from_hms(sign * h, sign * m, 0).ok();
                }
            } else if raw.len() == 4 {
                if let (Ok(h), Ok(m)) = (raw[..2].parse::<i8>(), raw[2..].parse::<i8>()) {
                    return UtcOffset::from_hms(sign * h, sign * m, 0).ok();
                }
            }
        }

        Some(DEFAULT_IST_OFFSET)
    }

    /// Format an `OffsetDateTime` into ISO 8601 / RFC 3339 string (e.g., `2026-10-03T11:05:06+05:30` or `2026-10-03T05:35:06Z`).
    pub fn format_iso(dt: &OffsetDateTime) -> String {
        dt.format(&Rfc3339).unwrap_or_else(|_| {
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                dt.year(),
                dt.month() as u8,
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            )
        })
    }

    /// Format current time in ISO 8601 with "T" separator in school's timezone.
    pub fn now_iso(tz: Option<&str>) -> String {
        let dt = Self::now_in_timezone(tz);
        Self::format_iso(&dt)
    }

    /// Format a `Date` into ISO 8601 date string (`YYYY-MM-DD`).
    pub fn format_date_iso(date: &Date) -> String {
        format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            date.month() as u8,
            date.day()
        )
    }

    /// Parse flexible date inputs supporting ISO 8601 / RFC 3339 with 'T' separator,
    /// `YYYY-MM-DD`, `DD/MM/YYYY`, `DD-MM-YYYY`, and `YYYY/MM/DD`.
    pub fn parse_iso_date(date_str: &str) -> Result<Date, String> {
        let trimmed = date_str.trim();
        if trimmed.is_empty() {
            return Err("Date string cannot be empty".to_string());
        }

        // 1. If it contains "T", try parsing full RFC 3339 / ISO 8601 datetime first
        if trimmed.contains('T') || trimmed.contains('t') {
            if let Ok(odt) = OffsetDateTime::parse(trimmed, &Rfc3339) {
                return Ok(odt.date());
            }

            // Also try format [year]-[month]-[day]T[hour]:[minute]:[second]
            if let Ok(format) = time::format_description::parse_borrowed::<2>(
                "[year]-[month]-[day]T[hour]:[minute]:[second]",
            ) {
                if let Ok(pdt) = PrimitiveDateTime::parse(trimmed, &format) {
                    return Ok(pdt.date());
                }
            }
        }

        // 2. Try standard ISO 8601 date format [year]-[month]-[day]
        if let Ok(format) = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]") {
            if let Ok(date) = Date::parse(trimmed, &format) {
                return Ok(date);
            }
        }

        // 3. Try Indian/European date formats [day]/[month]/[year] or [day]-[month]-[year]
        if let Ok(format) = time::format_description::parse_borrowed::<2>("[day]/[month]/[year]") {
            if let Ok(date) = Date::parse(trimmed, &format) {
                return Ok(date);
            }
        }

        if let Ok(format) = time::format_description::parse_borrowed::<2>("[day]-[month]-[year]") {
            if let Ok(date) = Date::parse(trimmed, &format) {
                return Ok(date);
            }
        }

        if let Ok(format) = time::format_description::parse_borrowed::<2>("[year]/[month]/[day]") {
            if let Ok(date) = Date::parse(trimmed, &format) {
                return Ok(date);
            }
        }

        Err(format!(
            "Invalid date format '{}'. Expected ISO 8601 (YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ) or DD/MM/YYYY",
            trimmed
        ))
    }

    /// Parse flexible datetime input into an `OffsetDateTime` using the given timezone fallback.
    pub fn parse_iso_datetime(
        dt_str: &str,
        tz_fallback: Option<&str>,
    ) -> Result<OffsetDateTime, String> {
        let trimmed = dt_str.trim();
        if trimmed.is_empty() {
            return Err("Datetime string cannot be empty".to_string());
        }

        // Try standard RFC 3339 (e.g. 2026-10-03T11:05:06+05:30 or 2026-10-03T05:35:06Z)
        if let Ok(odt) = OffsetDateTime::parse(trimmed, &Rfc3339) {
            return Ok(odt);
        }

        // Fallback: parse as ISO Date and assign midnight at timezone
        let date = Self::parse_iso_date(trimmed)?;
        let offset = tz_fallback
            .and_then(Self::parse_timezone_offset)
            .unwrap_or(DEFAULT_IST_OFFSET);
        let pdt = PrimitiveDateTime::new(date, Time::MIDNIGHT);
        Ok(pdt.assume_offset(offset))
    }

    /// Check if a date has expired compared to current UTC time.
    pub fn is_expired(expires_at: &OffsetDateTime) -> bool {
        *expires_at < Self::now_utc()
    }

    /// Add duration to current time.
    pub fn now_plus(duration: Duration) -> OffsetDateTime {
        Self::now_utc() + duration
    }
}
