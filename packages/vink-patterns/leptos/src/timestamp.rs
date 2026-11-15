//! Absolute instants: deterministic UTC on SSR/first hydrate, device zone after mount.
use leptos::prelude::*;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const INVALID: &str = "Thời gian không hợp lệ";

/// A real conversion, including nonzero source offsets and day boundaries. Invalid data is
/// not sliced into a plausible date. There is no relative clock or future-time clamping.
pub fn updated_at_label(timestamp: &str) -> String {
    let Ok(instant) = OffsetDateTime::parse(timestamp, &Rfc3339) else {
        return INVALID.into();
    };
    let utc = instant.to_offset(UtcOffset::UTC);
    format!(
        "{:02}/{:02}/{:04}, {:02}:{:02} UTC",
        utc.day(),
        utc.month() as u8,
        utc.year(),
        utc.hour(),
        utc.minute()
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
export function vinkLocalTimestamp(instant, fallback) {
    try {
        return new Intl.DateTimeFormat('vi-VN', {
            day: '2-digit', month: '2-digit', year: 'numeric',
            hour: '2-digit', minute: '2-digit', hourCycle: 'h23', timeZoneName: 'short'
        }).format(new Date(instant));
    } catch (_) { return fallback; }
}
")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = vinkLocalTimestamp)]
    fn local_timestamp(instant: &str, fallback: &str) -> String;
}

#[component]
pub(crate) fn Timestamp(instant: String) -> impl IntoView {
    let label = RwSignal::new(updated_at_label(&instant));
    // Effects run after hydration, never during SSR or its matching first client render.
    // A single absolute presentation needs no timer, provider call or independent clock.
    #[cfg(target_arch = "wasm32")]
    {
        let source = instant.clone();
        Effect::new(move |_| {
            let fallback = updated_at_label(&source);
            if fallback != INVALID {
                label.set(local_timestamp(&source, &fallback));
            }
        });
    }
    view! { <time datetime=instant.clone() title=instant>{move || label.get()}</time> }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_fallback_converts_offsets_and_rejects_invalid_dates() {
        for (source, expected) in [
            ("2026-11-01T08:30:00Z", "01/11/2026, 08:30 UTC"),
            ("2026-11-02T00:30:00+07:00", "01/11/2026, 17:30 UTC"),
            ("2026-11-01T23:30:00-08:00", "02/11/2026, 07:30 UTC"),
            ("2099-01-01T00:00:00Z", "01/01/2099, 00:00 UTC"),
            ("2026-02-30T08:30:00Z", INVALID),
            ("2026-11-01T25:30:00Z", INVALID),
            ("2026-11-01", INVALID),
            ("", INVALID),
            ("ỹỷữợ", INVALID),
        ] {
            assert_eq!(updated_at_label(source), expected, "{source}");
        }
    }

    #[test]
    fn ssr_keeps_instant_and_explicit_zone_in_all_time_consumers() {
        let html = Owner::new().with(|| {
            view! {
                <Timestamp instant="2026-11-02T00:30:00+07:00".into() />
            }
            .to_html()
        });
        assert!(
            html.contains("datetime=\"2026-11-02T00:30:00+07:00\""),
            "{html}"
        );
        assert!(html.contains("01/11/2026, 17:30 UTC"), "{html}");
    }
}
