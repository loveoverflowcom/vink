//! Deterministic UTC for SSR and first hydration, with device-local display after mount.
use leptos::prelude::*;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const INVALID: &str = "Thời gian không hợp lệ";

fn utc_instant(timestamp: &str) -> Option<OffsetDateTime> {
    let parsed = OffsetDateTime::parse(timestamp, &Rfc3339).ok()?;
    // HTML global dates require a positive year.
    if parsed.year() < 1 {
        return None;
    }
    parsed.checked_to_offset(UtcOffset::UTC)
}

pub fn updated_at_label(timestamp: &str) -> String {
    utc_instant(timestamp).map_or_else(|| INVALID.into(), utc_label)
}

fn utc_label(utc: OffsetDateTime) -> String {
    format!(
        "{:02}/{:02}/{:04}, {:02}:{:02} UTC",
        utc.day(),
        utc.month() as u8,
        utc.year(),
        utc.hour(),
        utc.minute()
    )
}

/// Zero-padded calendar date (`dd/mm/yyyy`) in `Asia/Ho_Chi_Minh` (UTC+7, no DST) for work-view
/// “Tính từ dd/mm/yyyy” labels (`VIEW-09`, `UX-10`).
pub fn work_view_date_hcm(timestamp: &str) -> Option<String> {
    let instant = OffsetDateTime::parse(timestamp, &Rfc3339).ok()?;
    let hcm = instant.checked_to_offset(UtcOffset::from_hms(7, 0, 0).ok()?)?;
    Some(format!(
        "{:02}/{:02}/{:04}",
        hcm.day(),
        hcm.month() as u8,
        hcm.year()
    ))
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeMode {
    #[default]
    Social,
    Date,
    Absolute,
}

pub fn timestamp_millis(timestamp: &str) -> Option<i64> {
    let instant = utc_instant(timestamp)?;
    Some(instant.unix_timestamp_nanos().div_euclid(1_000_000) as i64)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
let cachedZone, exactFormat;
export function viewerZone() {
    return new Intl.DateTimeFormat('vi-VN').resolvedOptions().timeZone;
}
// getTimezoneOffset rounds historical second offsets to minutes. Interpret the actual
// local civil fields as UTC to recover the complete offset. setUTCFullYear also keeps years 1–99.
export function viewerOffset(ms) {
    const local = new Date(ms), civil = new Date(0);
    civil.setUTCFullYear(local.getFullYear(), local.getMonth(), local.getDate());
    civil.setUTCHours(local.getHours(), local.getMinutes(), local.getSeconds(), local.getMilliseconds());
    return (civil.getTime() - ms) / 1000;
}
export function viewerExact(ms, zone) {
    if (cachedZone !== zone || !exactFormat) {
        exactFormat = new Intl.DateTimeFormat('vi-VN', {
            timeZone:zone, year:'numeric', month:'2-digit', day:'2-digit',
            hour:'2-digit', minute:'2-digit', second:'2-digit', hourCycle:'h23', timeZoneName:'longOffset'
        });
        cachedZone = zone;
    }
    return exactFormat.format(ms) + ' (' + zone + ')';
}
")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = viewerZone)]
    fn viewer_zone() -> Result<String, wasm_bindgen::JsValue>;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = viewerOffset)]
    fn viewer_offset(ms: f64) -> i32;
    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = viewerExact)]
    fn viewer_exact(ms: f64, zone: &str) -> Result<String, wasm_bindgen::JsValue>;
}

#[derive(Clone, Copy)]
struct DisplayClock {
    now: RwSignal<Option<i64>>,
    zone: RwSignal<Option<String>>,
    day: Memo<Option<time::Date>>,
}

fn local_instant(ms: i64) -> Option<OffsetDateTime> {
    #[cfg(target_arch = "wasm32")]
    {
        let utc = OffsetDateTime::from_unix_timestamp_nanos(i128::from(ms) * 1_000_000).ok()?;
        utc.checked_to_offset(UtcOffset::from_whole_seconds(viewer_offset(ms as f64)).ok()?)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ms;
        None
    }
}

/// Install once at the Web root. SSR and the initial hydration pass have no viewer zone.
pub fn provide_display_clock(visible: Signal<bool>) {
    let now = RwSignal::new(None::<i64>);
    let zone = RwSignal::new(None::<String>);
    let day = Memo::new(move |_| {
        zone.with(|zone| {
            zone.as_ref()
                .and_then(|_| local_instant(now.get()?))
                .map(|d| d.date())
        })
    });
    provide_context(DisplayClock { now, zone, day });
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        if visible.get() {
            let tick = move || now.set(Some(leptos::web_sys::js_sys::Date::now() as i64));
            zone.set(viewer_zone().ok());
            tick();
            if let Ok(handle) = set_interval_with_handle(tick, std::time::Duration::from_secs(60)) {
                on_cleanup(move || handle.clear());
            }
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = visible;
}

fn group(day: time::Date, today: time::Date) -> &'static str {
    match (today - day).whole_days() {
        0 => "Hôm nay",
        1..=6 => "7 ngày qua",
        n if n < 0 => "Sắp tới",
        _ => "Cũ hơn",
    }
}

/// Track only a change of local day or zone, independent of inbox data and row identities.
pub fn inbox_day_group() -> impl Fn(&str) -> &'static str + Copy {
    let clock = use_context::<DisplayClock>();
    move |timestamp| {
        let Some(ms) = timestamp_millis(timestamp) else {
            return "Chưa rõ thời gian";
        };
        let Some(clock) = clock else {
            return "Thông báo";
        };
        clock.zone.track();
        let Some((today, instant)) = clock.day.get().zip(local_instant(ms)) else {
            return "Thông báo";
        };
        group(instant.date(), today)
    }
}

fn absolute(at: OffsetDateTime) -> String {
    format!(
        "{:02}:{:02}, {:02}/{:02}/{:04}",
        at.hour(),
        at.minute(),
        at.day(),
        u8::from(at.month()),
        at.year()
    )
}

fn calendar_date(at: OffsetDateTime) -> String {
    format!("{}/{}/{}", at.day(), u8::from(at.month()), at.year())
}

fn social(at: OffsetDateTime, now_ms: i64, today: time::Date) -> String {
    let elapsed = i128::from(now_ms) * 1_000_000 - at.unix_timestamp_nanos();
    if elapsed < 0 {
        return absolute(at);
    }
    let seconds = elapsed / 1_000_000_000;
    if seconds < 60 {
        return "Vừa xong".into();
    }
    if seconds < 3600 {
        return format!("{} phút trước", seconds / 60);
    }
    match (today - at.date()).whole_days() {
        0 => format!("{} giờ trước", seconds / 3600),
        1 => "Hôm qua".into(),
        days @ 2..=6 => format!("{days} ngày trước"),
        _ if today.year() == at.year() => format!("{}/{}", at.day(), u8::from(at.month())),
        _ => calendar_date(at),
    }
}

#[derive(Clone, Copy)]
struct ListExact(RwSignal<bool>);

/// The disclosure belongs to the list so whole-row links keep their native activation and tab order.
#[component]
pub fn TimeList(children: Children, #[prop(optional)] show: Option<Signal<bool>>) -> impl IntoView {
    let exact = RwSignal::new(false);
    view! {
        <leptos::context::Provider value=ListExact(exact)>
            {children()}
            <button class="vk-time-list-toggle" hidden=move || show.is_some_and(|show| !show.get()) type="button" aria-pressed=move || exact.get().to_string()
                on:click=move |_| exact.update(|v| *v = !*v)
                on:keydown=move |e| if e.key() == "Escape" && exact.get_untracked() { e.stop_propagation(); exact.set(false) }>
                "Ngày giờ đầy đủ"
            </button>
        </leptos::context::Provider>
    }
}

#[derive(Clone, PartialEq)]
struct LocalTime {
    at: OffsetDateTime,
    exact: String,
}

#[component]
pub fn Timestamp(
    instant: String,
    #[prop(optional)] mode: TimeMode,
    #[prop(optional)] disclosure: bool,
    #[prop(optional)] class: &'static str,
    #[prop(optional)] description_id: String,
    #[prop(optional)] context: &'static str,
) -> impl IntoView {
    let Some(at) = utc_instant(&instant) else {
        return view! { <span class=class id=(!description_id.is_empty()).then_some(description_id)>{INVALID}</span> }.into_any();
    };
    let fallback = utc_label(at);
    let exact_utc = format!(
        "{:02}:{:02}:{:02} UTC, {:02}/{:02}/{:04}",
        at.hour(),
        at.minute(),
        at.second(),
        at.day(),
        u8::from(at.month()),
        at.year()
    );
    let clock = use_context::<DisplayClock>();
    let local = Memo::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            let ms = at.unix_timestamp_nanos().div_euclid(1_000_000) as i64;
            let zone = clock?.zone.get()?;
            Some(LocalTime {
                at: at.checked_to_offset(local_instant(ms)?.offset())?,
                exact: viewer_exact(ms as f64, &zone).ok()?,
            })
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            None::<LocalTime>
        }
    });
    let exact = Memo::new(move |_| {
        local.with(|value| {
            value
                .as_ref()
                .map_or_else(|| exact_utc.clone(), |v| v.exact.clone())
        })
    });
    let expanded = if disclosure {
        None
    } else {
        use_context::<ListExact>().map(|c| c.0)
    }
    .unwrap_or_else(|| RwSignal::new(false));
    let label = Memo::new(move |_| {
        if expanded.get() {
            return exact.get();
        }
        local.with(|value| {
            let (Some(value), Some(clock)) = (value, clock) else {
                return fallback.clone();
            };
            match mode {
                TimeMode::Social => match clock.now.get().zip(clock.day.get()) {
                    Some((now, today)) => social(value.at, now, today),
                    None => fallback.clone(),
                },
                TimeMode::Absolute => absolute(value.at),
                TimeMode::Date => calendar_date(value.at),
            }
        })
    });
    let description = Memo::new(move |_| {
        if context.is_empty() {
            exact.get()
        } else {
            format!("{context}: {}", exact.get())
        }
    });
    let time = view! {
        <time class=class datetime=instant title=move || exact.get() aria-description=move || description.get()>
            {move || label.get()}
        </time>
    };
    if disclosure {
        view! { <button type="button" class="vk-time-disclosure" aria-pressed=move || expanded.get().to_string()
            aria-label=move || format!("Ngày giờ đầy đủ: {}", exact.get())
            on:click=move |_| expanded.update(|value| *value = !*value)
            on:keydown=move |e| if e.key() == "Escape" && expanded.get_untracked() { e.stop_propagation(); expanded.set(false) }>
            {time}
        </button> }.into_any()
    } else {
        view! { {time} {(!description_id.is_empty()).then(|| view! { <span hidden id=description_id>{move || description.get()}</span> })} }.into_any()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_precedence_boundaries_dst_and_future() {
        let parse = |s| OffsetDateTime::parse(s, &Rfc3339).unwrap();
        for (now, event, expected) in [
            (
                "2026-11-28T00:10:00+07:00",
                "2026-11-28T00:09:01+07:00",
                "Vừa xong",
            ),
            (
                "2026-11-28T00:10:00+07:00",
                "2026-11-28T00:09:00+07:00",
                "1 phút trước",
            ),
            (
                "2026-11-28T00:10:00+07:00",
                "2026-11-27T23:11:00+07:00",
                "59 phút trước",
            ),
            (
                "2026-11-28T00:10:00+07:00",
                "2026-11-27T23:10:00+07:00",
                "Hôm qua",
            ),
            (
                "2026-11-28T15:00:00+07:00",
                "2026-11-28T01:30:00+07:00",
                "13 giờ trước",
            ),
            (
                "2026-11-28T00:00:00-08:00",
                "2026-11-27T10:30:00-08:00",
                "Hôm qua",
            ),
            (
                "2026-03-09T00:01:00-07:00",
                "2026-03-08T00:02:00-08:00",
                "Hôm qua",
            ),
            (
                "2026-11-01T23:59:00-08:00",
                "2026-11-01T00:00:00-07:00",
                "24 giờ trước",
            ),
            (
                "2026-11-28T09:00:00Z",
                "2026-11-22T09:00:00Z",
                "6 ngày trước",
            ),
            ("2026-11-28T09:00:00Z", "2026-11-21T09:00:00Z", "21/11"),
            ("2027-01-01T09:00:00Z", "2026-12-31T09:00:00Z", "Hôm qua"),
            ("2027-01-01T09:00:00Z", "2020-12-31T09:00:00Z", "31/12/2020"),
            (
                "2024-03-01T10:00:00+09:00",
                "2024-02-29T22:00:00+09:00",
                "Hôm qua",
            ),
            (
                "2027-01-01T18:00:00+14:00",
                "2026-12-31T21:00:00+14:00",
                "Hôm qua",
            ),
            (
                "2026-11-28T09:00:00Z",
                "2026-11-28T09:00:00.000001Z",
                "09:00, 28/11/2026",
            ),
            (
                "2026-11-28T09:00:00Z",
                "2026-11-28T09:00:01Z",
                "09:00, 28/11/2026",
            ),
        ] {
            assert_eq!(
                social(
                    parse(event),
                    timestamp_millis(now).unwrap(),
                    parse(now).date()
                ),
                expected,
                "{event} at {now}"
            );
        }
        assert_eq!(
            work_view_date_hcm("2026-11-27T18:30:00Z").as_deref(),
            Some("28/11/2026")
        );
        assert_eq!(
            group(
                parse("2026-03-08T00:02:00-08:00").date(),
                parse("2026-03-09T00:01:00-07:00").date()
            ),
            "7 ngày qua"
        );
    }

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

    #[test]
    fn work_view_date_hcm_zero_pads_and_respects_utc_plus_7_boundaries() {
        for (source, expected) in [
            ("2026-12-01T01:00:00Z", Some("01/12/2026")),
            ("2026-11-30T17:00:00Z", Some("01/12/2026")),
            ("2026-11-30T16:59:59Z", Some("30/11/2026")),
            ("2026-01-01T00:00:00Z", Some("01/01/2026")),
            ("2026-12-31T17:00:00Z", Some("01/01/2027")),
            ("invalid", None),
            ("", None),
        ] {
            assert_eq!(work_view_date_hcm(source).as_deref(), expected, "{source}");
        }
    }
}
