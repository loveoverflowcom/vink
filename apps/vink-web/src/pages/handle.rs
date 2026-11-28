use crate::contract::{AuthorResolution, ProfileWork, ProfileWorks};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::{use_params_map, use_query_map};
use vink_ui::{HiddenField, Link as UiLink};

use crate::ui::empty_state::{EmptyArtwork, EmptyState, NotFoundContent};

const NO_PUBLIC_WORKS: &str = "Chưa có tác phẩm công khai.";

#[component]
pub fn Author() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    view! { <For
        each=move || vec![(params.read().get("author").unwrap_or_default(), query.read().get("cursor"))]
        key=Clone::clone
        children=|(segment, cursor)| match segment.strip_prefix('@') {
            Some(name) => view!{<PublicProfile name=name.to_string() cursor />}.into_any(),
            None => view!{<super::NotFound />}.into_any()
        }
    /> }
}

#[component]
fn PublicProfile(name: String, cursor: Option<String>) -> impl IntoView {
    let (generation, advance) = signal(0u64);
    // Answers older than `floor` may still show a field that was revoked since (a profile
    // change) or the failure the reader asked to retry. Lifecycle wake-ups only revalidate:
    // the current answer stays mounted with its focus and scroll until the new one arrives.
    let (floor, set_floor) = signal(0u64);
    let own_relationships: RwSignal<
        Option<(String, crate::contract::follow::OwnRelationshipsSummary)>,
    > = RwSignal::new(None);
    let retry_focus = RwSignal::new(false);
    let renew = move || {
        advance.update(|stamp| *stamp += 1);
        set_floor.set(generation.get_untracked());
    };
    let data = crate::api::author_resource(name.clone(), cursor.clone(), generation);
    let status = super::ResponseStatus::capture();
    status.set_no_store();
    #[cfg(feature = "browser")]
    {
        super::profile::observe_changes(renew);
        crate::read_events::observe(move || {
            if crate::read_events::visible() {
                advance.update(|stamp| *stamp += 1);
            }
        });
    }
    view! {
        <Transition fallback=|| view! { <crate::ui::PendingRead label="Đang mở trang tác giả…" /> }>
            {move || match data.get() {
                Some((stamp, result)) if stamp >= floor.get() => {
                    if retry_focus.get_untracked() {
                        retry_focus.set(false);
                        super::focus_recovery("author-profile-heading");
                    }
                    render_profile(
                        result,
                        &status,
                        &name,
                        cursor.clone(),
                        own_relationships,
                        move || {
                            #[cfg(feature = "browser")]
                            retry_focus.set(document().active_element().is_some_and(|el| el.id() == "profile-retry"));
                            renew();
                        },
                    )
                }
                _ => view! { <crate::ui::PendingRead label="Đang mở trang tác giả…" /> }.into_any(),
            }}
        </Transition>
    }
}

fn render_profile(
    result: Result<AuthorResolution, crate::api::LoadError>,
    status: &super::ResponseStatus,
    name: &str,
    cursor: Option<String>,
    own_relationships: RwSignal<Option<(String, crate::contract::follow::OwnRelationshipsSummary)>>,
    retry: impl Fn() + Copy + Send + Sync + 'static,
) -> AnyView {
    let continued = cursor.is_some();
    let (code, heading, detail) = match result {
        Ok(AuthorResolution::Current {
            author,
            bio,
            publications,
            has_public_works,
            relationships,
        }) if author.valid_href().is_some() => {
            // The overlay belongs to this exact public projection. An authoritative refresh
            // may have revoked a list or count since the write; no old overlay may outlive it.
            let (public_summary, on_summary) =
                profile_summary(author.handle.clone(), *relationships);
            let action_handle = author.handle.clone();
            let action_name = author.pen_name.clone();
            return view! {
                <Title text=format!("{} · Vink", author.pen_name) />
                <Link rel="canonical" href=crate::canonical_url(&author.href) />
                <article class="vk-author-public">
                    <ProfileIdentity pen_name=author.pen_name handle=author.handle.clone()>
                        <super::follow::ProfileSocial
                            handle=action_handle
                            pen_name=action_name
                            public=public_summary.into()
                            on_summary
                            own_relationships
                        />
                    </ProfileIdentity>
                    {bio.map(|text| view! { <p class="vk-author-bio">{text}</p> })}
                    <section aria-labelledby="author-works-heading"
                        class="mt-vk-8 [&>.vk-ui-link]:inline-block [&>.vk-ui-link]:[margin-inline-end:var(--vk-space-3)]">
                        <super::saved_profile::PublicWorksSlot handle=author.handle.clone() />
                        {works_region(publications, has_public_works, continued, author.href, retry)}
                    </section>
                </article>
            }.into_any();
        }
        Ok(AuthorResolution::Alias { href }) => {
            redirect_alias(status, &format!("/@{name}"), &href);
            return view! { <a href=href>"Mở địa chỉ hiện tại"</a> }.into_any();
        }
        Ok(AuthorResolution::Gone) => (
            410,
            "Hồ sơ không còn trên Vink",
            "Tài khoản của tác giả đã được xóa. Địa chỉ này sẽ không dẫn đến hồ sơ khác.",
        ),
        Ok(AuthorResolution::Missing) => {
            status.set(404);
            return view! {
                <Meta name="robots" content="noindex" />
                <NotFoundContent heading_id="author-profile-heading" />
            }
            .into_any();
        }
        _ => (
            503,
            "Chưa tải được hồ sơ",
            "Kết nối có thể bị gián đoạn hoặc máy chủ đang bận. Bạn có thể thử lại ngay tại đây.",
        ),
    };
    status.set(code);
    view! {
        <Title text=format!("{heading} · Vink") />
        <Meta name="robots" content="noindex" />
        <section id="profile-unavailable" class="vk-author-public vk-profile-unavailable"
            aria-labelledby="author-profile-heading">
            <h1 id="author-profile-heading" tabindex="-1">{heading}</h1>
            <p>{detail}</p>
            {if code == 503 {
                view! {
                    <RetryRead id="profile-retry" path=format!("/@{name}") cursor
                        on_retry=Callback::new(move |()| retry()) />
                }.into_any()
            } else {
                view! {
                    <UiLink variant=vink_ui::LinkVariant::Standalone attr:id="profile-unavailable-explore"
                        href="/">"Về Khám phá"</UiLink>
                }.into_any()
            }}
        </section>
    }
    .into_any()
}

/// Works region of a current public profile (#587). Only a settled, empty first page with no
/// further page is a true empty and carries the literary artwork. A continuation or an
/// unexpected empty head, a cursor restart and a failed list read keep their own text and
/// recovery without artwork, so none of them passes for an author who never published.
fn works_region(
    publications: Option<ProfileWorks>,
    has_public_works: bool,
    continued: bool,
    href: String,
    retry: impl Fn() + Copy + Send + Sync + 'static,
) -> AnyView {
    match publications {
        Some(ProfileWorks::Page { works, next_cursor }) => {
            let next = next_cursor.map(|cursor| format!("{href}?cursor={cursor}"));
            let body = if !works.is_empty() {
                view! { <vink_patterns::TimeList><ul class="vk-author-works">
                    {works.into_iter().map(|work| view! { <ProfileWorkItem work /> }).collect_view()}
                </ul></vink_patterns::TimeList> }.into_any()
            } else if continued || next.is_some() {
                view! { <EmptyState compact=true hook="vk-author-works-empty"
                message="Trang này không còn tác phẩm." /> }
                .into_any()
            } else {
                no_public_works()
            };
            view! {
                {body}
                {next.map(|href| view! { <UiLink href>"Trang tiếp"</UiLink> })}
                {continued.then(|| view! { <UiLink href>"Về trang đầu"</UiLink> })}
            }
            .into_any()
        }
        Some(ProfileWorks::Restart) => view! {
            <p role="alert">"Trang tác phẩm không còn khớp với hồ sơ này."</p>
            <UiLink href>"Về trang đầu"</UiLink>
        }
        .into_any(),
        None if !has_public_works => no_public_works(),
        None => view! {
            <p role="alert">"Chưa tải được danh sách tác phẩm."</p>
            <vink_ui::Button variant=vink_ui::ButtonVariant::Tonal
                on_press=Callback::new(move |()| retry())>"Thử lại"</vink_ui::Button>
        }
        .into_any(),
    }
}

/// True empty: the shared artwork and one sentence, in the works region's own flow (CHOICE-01).
/// No action: guests and other readers have nothing to do here, and the owner's way to write
/// is the Desk destination the shell already offers.
fn no_public_works() -> AnyView {
    view! {
        <EmptyState region=true artwork=EmptyArtwork::NoPublicWorks hook="vk-author-works-empty"
            message=NO_PUBLIC_WORKS />
    }
    .into_any()
}

/// One editorial row (#587): the title is the only link and its overlay covers the row, so the
/// row is one focus stop named by the title alone. The meta line (primary tag, first public
/// date) and the excerpt are shown only when the public projection supplied them; the meta line
/// follows the heading in the DOM and is drawn above it. No byline: the profile names the author.
#[component]
fn ProfileWorkItem(work: ProfileWork) -> impl IntoView {
    let ProfileWork {
        work_id,
        title,
        excerpt,
        first_published_at,
        tags,
    } = work;
    let primary_tag = tags.into_iter().next().map(|tag| tag.label);
    let date = first_published_at.filter(|s| vink_patterns::timestamp_millis(s).is_some()).map(|instant| {
        view! { <vink_patterns::Timestamp class="vk-feed-date" context="Công khai lần đầu" instant mode=vink_patterns::TimeMode::Date /> }
    });
    let separate = primary_tag.is_some() && date.is_some();
    let meta = (primary_tag.is_some() || date.is_some()).then(|| {
        view! {
            <p class="vk-public-row__eyebrow vk-profile-work__meta">
                {primary_tag.map(|tag| view! { <span class="vk-public-row__tag">{tag}</span> })}
                {separate.then_some(" · ")}
                {date}
            </p>
        }
    });
    // Line-broken text (verse, stanzas) keeps one more visible line than running prose.
    let excerpt = excerpt.filter(|text| !text.trim().is_empty()).map(|text| {
        let class = if text.contains('\n') {
            "vk-public-row__excerpt vk-profile-work__excerpt vk-profile-work__excerpt--lines"
        } else {
            "vk-public-row__excerpt vk-profile-work__excerpt"
        };
        view! { <p class=class>{text}</p> }
    });
    view! {
        <li class="vk-profile-work">
            <h3 class="vk-public-row__title vk-profile-work__title">
                <a class="vk-profile-work__link" id=format!("profile-work-{work_id}")
                    data-profile-return="true" href=format!("/tac-pham/{work_id}")>{title}</a>
            </h3>
            {meta}
            {excerpt}
        </li>
    }
}

// The public summary lives with the rendered profile. A viewer change affects the
// command/receipt and the private overlay, but the public read can still settle or retry.
pub(super) fn profile_summary(
    handle: String,
    initial: crate::contract::follow::PublicRelationships,
) -> (
    ReadSignal<super::follow::RelationshipSummary>,
    Callback<Option<crate::contract::follow::PublicRelationships>>,
) {
    use super::follow::RelationshipSummary;
    let (summary, set_summary) = signal(RelationshipSummary::Ready(initial));
    #[cfg(feature = "browser")]
    let publish = {
        let ticket = RwSignal::new(0u64);
        Callback::new(
            move |confirmed: Option<crate::contract::follow::PublicRelationships>| {
                ticket.update(|n| *n += 1);
                match confirmed {
                    Some(relationships) => {
                        set_summary.set(RelationshipSummary::Ready(relationships))
                    }
                    None => {
                        let target = handle.clone();
                        let request = ticket.get_untracked();
                        set_summary.set(RelationshipSummary::Reading);
                        leptos::task::spawn_local(async move {
                            let result =
                                crate::api::load_author_relationships_summary(&target).await;
                            if ticket.try_get_untracked() == Some(request) {
                                set_summary.set(match result {
                                    Ok(relationships) => RelationshipSummary::Ready(relationships),
                                    Err(_) => RelationshipSummary::Unavailable,
                                });
                            }
                        });
                    }
                }
            },
        )
    };
    #[cfg(not(feature = "browser"))]
    let publish = Callback::new(
        move |_: Option<crate::contract::follow::PublicRelationships>| {
            let _ = (&handle, set_summary);
        },
    );
    (summary, publish)
}

// A native GET is the fallback for the same path and cursor before hydration or without JS.
#[component]
pub(super) fn RetryRead(
    id: &'static str,
    path: String,
    cursor: Option<String>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let action = crate::follow_state::safe_profile_return(&path);
    view! {
        <form method="get" action=action on:submit=move |event: leptos::ev::SubmitEvent| {
            event.prevent_default();
            on_retry.run(());
        }>
            {cursor.map(|value| view! { <HiddenField name="cursor" value=Signal::stored(value) /> })}
            <vink_ui::Button id=id variant=vink_ui::ButtonVariant::Tonal kind=vink_ui::ButtonKind::Submit>
                "Thử lại"
            </vink_ui::Button>
        </form>
    }
}

pub(super) fn redirect_alias(status: &super::ResponseStatus, source_path: &str, href: &str) {
    #[cfg(feature = "ssr")]
    if let Some(opts) = &status.0
        && let Ok(url) = http::HeaderValue::from_str(href)
    {
        opts.set_status(http::StatusCode::MOVED_PERMANENTLY);
        opts.insert_header(http::header::LOCATION, url);
    }
    #[cfg(feature = "browser")]
    replace_profile_alias(source_path, href);
    let _ = (status, source_path, href);
}

#[cfg(feature = "browser")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/profile/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = replaceProfileAlias)]
    fn replace_profile_alias(source_path: &str, href: &str);
}

/// Public author header: seal, pen name H1 and handle, then the social row supplied by the
/// caller (relationship facts before the Follow action, in visual and tab order). The row's
/// state owners stay in `follow`; this component only places them (#588).
#[component]
pub(super) fn ProfileIdentity(
    pen_name: String,
    handle: String,
    children: Children,
) -> impl IntoView {
    let initials = pen_name
        .split_whitespace()
        .filter_map(|s| s.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    view! {
        <MobileBrand />
        <div class="vk-author-identity">
            <span class="vk-author-seal" aria-hidden="true">{initials}</span>
            <header>
                <h1 id="author-profile-heading" class="vk-ui-identity-name" tabindex="-1">{pen_name}</h1>
                <p class="vk-ui-identity-handle">{format!("@{handle}")}</p>
            </header>
            {children()}
        </div>
    }
}

#[component]
fn MobileBrand() -> impl IntoView {
    view! { <a class="vk-brand vk-handle-brand hidden [@media(max-width:759px)]:inline-flex [@media(max-width:759px)]:[margin-block-end:1rem]" href="/" aria-label="Vink — trang chủ">
    <span class="vk-brand__lockup" aria-hidden="true"></span></a> }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::PublicTagDto;

    const ART: &str = "/pkg/empty/vink-empty-no-public-works.webp";

    /// Drops the renderer's placeholder comments so assertions read the visible markup.
    fn clean(html: String) -> String {
        html.replace("<!---->", "").replace("<!>", "")
    }

    fn region(
        publications: Option<ProfileWorks>,
        has_public_works: bool,
        continued: bool,
    ) -> String {
        clean(Owner::new().with(|| {
            works_region(
                publications,
                has_public_works,
                continued,
                "/@le-thu".into(),
                || {},
            )
            .to_html()
        }))
    }

    fn page(works: Vec<ProfileWork>, next: Option<&str>) -> Option<ProfileWorks> {
        Some(ProfileWorks::Page {
            works,
            next_cursor: next.map(str::to_owned),
        })
    }

    fn work(id: &str, title: &str) -> ProfileWork {
        ProfileWork {
            work_id: id.into(),
            title: title.into(),
            excerpt: None,
            first_published_at: None,
            tags: vec![],
        }
    }

    fn tag(facet: &str, label: &str) -> PublicTagDto {
        PublicTagDto {
            tag_id: format!("{facet}-{label}"),
            facet: facet.into(),
            label: label.into(),
            slug: label.into(),
        }
    }

    /// The whole public profile for one resolution, as the route renders it.
    fn profile(
        publications: serde_json::Value,
        has_public_works: bool,
        cursor: Option<&str>,
    ) -> String {
        let resolution: AuthorResolution = serde_json::from_value(serde_json::json!({
            "state": "current",
            "author": {"pen_name": "Lê Thu", "handle": "le-thu", "href": "/@le-thu"},
            "has_public_works": has_public_works,
            "publications": publications,
        }))
        .unwrap();
        clean(Owner::new().with(|| {
            leptos_meta::provide_meta_context();
            crate::account::Session::provide();
            let status = crate::pages::ResponseStatus::capture();
            render_profile(
                Ok(resolution),
                &status,
                "le-thu",
                cursor.map(str::to_owned),
                RwSignal::new(None),
                || {},
            )
            .to_html()
        }))
    }

    #[test]
    fn true_empty_first_page_shows_artwork_inside_the_works_section() {
        let out = profile(
            serde_json::json!({"state": "page", "works": []}),
            false,
            None,
        );
        let section = out
            .find("aria-labelledby=\"author-works-heading\"")
            .expect("works section");
        let heading = out
            .find("id=\"author-works-heading\"")
            .expect("works heading");
        let state = out
            .find("vk-empty-state--region vk-author-works-empty")
            .expect("region state");
        let art = out.find(ART).expect("artwork");
        let message = out.find(NO_PUBLIC_WORKS).expect("message");
        assert!(
            section < heading && heading < state && state < art && art < message,
            "{out}"
        );
        assert!(
            out[section..].contains("</section>")
                && message < section + out[section..].find("</section>").unwrap(),
            "{out}"
        );
        // No call to action and no compact or page-wide placement for this state.
        assert!(!out[state..].contains("vk-empty-state__actions"), "{out}");
        assert!(
            !out.contains("vk-empty-state--compact") && !out.contains("vk-state-page"),
            "{out}"
        );
        assert_eq!(out.matches(ART).count(), 1, "{out}");
    }

    #[test]
    fn only_a_settled_empty_head_is_a_true_empty() {
        for out in [
            region(page(vec![], None), false, false),
            region(None, false, false),
        ] {
            assert!(
                out.contains("vk-empty-state--region vk-author-works-empty"),
                "{out}"
            );
            assert!(out.contains(ART) && out.contains(NO_PUBLIC_WORKS), "{out}");
            assert!(
                !out.contains("<a") && !out.contains("vk-ui-button") && !out.contains("role="),
                "{out}"
            );
        }
        let tail = region(page(vec![], None), false, true);
        assert!(
            tail.contains("vk-empty-state--compact vk-author-works-empty"),
            "{tail}"
        );
        assert!(
            tail.contains("Trang này không còn tác phẩm.") && tail.contains("Về trang đầu"),
            "{tail}"
        );
        let head_with_next = region(page(vec![], Some("c2")), true, false);
        assert!(
            head_with_next.contains("Trang tiếp") && head_with_next.contains("?cursor=c2"),
            "{head_with_next}"
        );
        let restart = region(Some(ProfileWorks::Restart), true, true);
        assert!(
            restart.contains("role=\"alert\"") && restart.contains("Về trang đầu"),
            "{restart}"
        );
        let failed = region(None, true, false);
        assert!(
            failed.contains("Chưa tải được danh sách tác phẩm.")
                && failed.contains("vk-ui-button")
                && failed.contains("Thử lại"),
            "{failed}"
        );
        for out in [&tail, &head_with_next, &restart, &failed] {
            assert!(
                !out.contains("vk-empty-state__art") && !out.contains(ART),
                "{out}"
            );
            assert!(
                !out.contains(NO_PUBLIC_WORKS) && !out.contains("vk-empty-state--region"),
                "{out}"
            );
        }
        for out in [&restart, &failed] {
            assert!(!out.contains("vk-empty-state"), "{out}");
        }
        // The profile's unavailable pages keep their own branch without the works artwork.
        for resolution in [
            Ok(AuthorResolution::Gone),
            Ok(AuthorResolution::Missing),
            Err(crate::api::LoadError::Unavailable),
        ] {
            let out = Owner::new().with(|| {
                leptos_meta::provide_meta_context();
                let status = crate::pages::ResponseStatus::capture();
                render_profile(
                    resolution,
                    &status,
                    "le-thu",
                    None,
                    RwSignal::new(None),
                    || {},
                )
                .to_html()
            });
            assert!(
                !out.contains(ART) && !out.contains(NO_PUBLIC_WORKS),
                "{out}"
            );
        }
    }

    #[test]
    fn editorial_item_shows_only_supplied_public_facts_behind_one_link() {
        let mut rich = work("w-1", "Mưa tháng Bảy");
        rich.excerpt = Some("Trời ngả chiều qua mái ngói cũ.\nGió về chậm qua hiên.".into());
        rich.first_published_at = Some("2026-10-29T18:00:00Z".into());
        rich.tags = vec![tag("form", "Thơ"), tag("theme", "Ký ức")];
        let mut prose = work("w-2", "Gió ngoài hiên");
        prose.excerpt = Some("Chiều hôm ấy, gió trở về qua những tán cây.".into());
        let long = "Mộtnhanđềrấtdàikhôngcókhoảngtrắngđểxuốngdòngtrênmànhìnhhẹp".repeat(3);
        let out = region(
            page(vec![rich, prose, work("w-3", &long)], None),
            true,
            false,
        );
        assert_eq!(out.matches("<li").count(), 3, "{out}");
        assert_eq!(out.matches("<a").count(), 3, "{out}");
        assert!(
            !out.contains("vk-ui-button") && !out.contains("vk-ui-link"),
            "{out}"
        );
        for (id, title) in [
            ("w-1", "Mưa tháng Bảy"),
            ("w-2", "Gió ngoài hiên"),
            ("w-3", long.as_str()),
        ] {
            let link = format!(
                "<a id=\"profile-work-{id}\" data-profile-return=\"true\" href=\"/tac-pham/{id}\" class=\"vk-profile-work__link\">{title}</a>"
            );
            assert!(out.contains(&link), "{out}");
        }
        // Each anchor holds only its title: meta and excerpt sit outside and no link nests.
        for chunk in out.split("<a ").skip(1) {
            let inner = &chunk[..chunk.find("</a>").unwrap()];
            assert!(
                !inner.contains("<p") && !inner.contains("<time") && !inner.contains("<a"),
                "{inner}"
            );
        }
        let first = &out[..out.find("profile-work-w-2").unwrap()];
        assert!(
            first.contains("Thơ</span> · ")
                && first.contains("datetime=\"2026-10-29T18:00:00Z\"")
                && first.contains("Công khai lần đầu")
                && first.contains("29/10/2026, 18:00 UTC"),
            "{first}"
        );
        assert!(
            !first.contains("Ký ức"),
            "only the primary tag is shown: {first}"
        );
        // Line breaks of verse survive the render; prose gets the shorter clamp.
        assert!(first.contains("vk-profile-work__excerpt--lines\">Trời ngả chiều qua mái ngói cũ.\nGió về chậm qua hiên.</p>"), "{first}");
        let second =
            &out[out.find("profile-work-w-2").unwrap()..out.find("profile-work-w-3").unwrap()];
        assert!(
            second
                .contains("class=\"vk-public-row__excerpt vk-profile-work__excerpt\">Chiều hôm ấy"),
            "{second}"
        );
        assert!(
            !second.contains("vk-profile-work__meta") && !second.contains("<time"),
            "{second}"
        );
        let third = &out[out.find("profile-work-w-3").unwrap()..];
        assert!(
            !third.contains("vk-profile-work__meta") && !third.contains("vk-profile-work__excerpt"),
            "{third}"
        );
        assert!(
            !out.contains("vk-empty-state") && !out.contains(ART),
            "{out}"
        );
        // No invented counters: the profile item carries no Thấm, comment or view number.
        for counter in ["Thấm", "nhận xét", "Hữu ích", "lượt xem", "Lượt xem"] {
            assert!(!out.contains(counter), "{counter}: {out}");
        }
        // Date alone, or tag alone, renders without a dangling separator.
        let mut dated = work("w-4", "Chỉ có ngày");
        dated.first_published_at = Some("2026-10-30T08:00:00Z".into());
        let mut tagged = work("w-5", "Chỉ có thẻ");
        tagged.tags = vec![tag("form", "Tản văn")];
        let mut invalid = work("w-6", "Ngày hỏng");
        invalid.first_published_at = Some("không phải ngày".into());
        invalid.excerpt = Some("   ".into());
        let out = region(page(vec![dated, tagged, invalid], None), true, false);
        assert!(
            out.contains("<p class=\"vk-public-row__eyebrow vk-profile-work__meta\"><time"),
            "{out}"
        );
        assert!(
            out.contains("<span class=\"vk-public-row__tag\">Tản văn</span></p>"),
            "{out}"
        );
        assert!(!out.contains(" · "), "{out}");
        let broken = &out[out.find("profile-work-w-6").unwrap()..];
        assert!(
            !broken.contains("vk-profile-work__meta")
                && !broken.contains("vk-profile-work__excerpt"),
            "{broken}"
        );
    }
}
