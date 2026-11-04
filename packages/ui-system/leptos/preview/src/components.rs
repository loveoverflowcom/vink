//! UIS-02 specimens: every component through the public `vink_ui` API, with real states and
//! events written to the preview's event log. Names and works are [Mẫu]; nothing is fetched.

use leptos::prelude::*;
use vink_ui::catalog::fixture;
use vink_ui::identity_vectors::{BROKEN_AVIF_BASE64, PEOPLE, SAMPLE_PNG_BASE64};
use vink_ui::{
    ActionState, Align, AspectRatio, AuthorByline, Avatar, AvatarGroup, AvatarKind, AvatarSize,
    Button, ButtonKind, ButtonVariant, Checkbox, ChoiceOption, Container, Direction, Divider,
    FieldMode, FieldVariant, FilterChip, FormSummary, Grid, GridMin, Heading, HeadingLevel, Icon,
    IconButton, IconName, ImageFrame, ImageState, Link, LinkTarget, LinkVariant, Measure, Person,
    RadioGroup, Radius, SearchField, Select, Space, Stack, Surface, SurfaceTone, Switch, Text,
    TextArea, TextField, TextInput, TextKind, TextTag, TextTone, ThemePreference, ThemeScope,
    TypeRole,
};

/// The preview's event log; absent when a specimen is rendered in a unit test.
#[derive(Clone, Copy)]
pub struct EventLog(pub Callback<String>);

pub(crate) fn log(message: impl Into<String>) {
    if let Some(EventLog(record)) = use_context::<EventLog>() {
        record.run(message.into());
    }
}

fn sample_png() -> String {
    format!("data:image/png;base64,{SAMPLE_PNG_BASE64}")
}

fn broken_avif() -> String {
    format!("data:image/avif;base64,{BROKEN_AVIF_BASE64}")
}

fn person(index: usize) -> (Option<&'static str>, Option<&'static str>) {
    PEOPLE[index]
}

fn note(text: &'static str) -> impl IntoView {
    view! { <p class="vkp-note">{text}</p> }
}

/// The specimen body for a UIS-02 catalog entry, or `None` for an unknown ID.
pub fn specimen(id: &str, scenario: &str) -> Option<AnyView> {
    Some(match id {
        "text.text" => text_text(scenario == "long-label"),
        "text.heading" => text_heading(),
        "layout.surface" => layout_surface(scenario == "nested-theme"),
        "layout.container" => layout_container(),
        "layout.stack" => layout_stack(scenario == "long-label"),
        "layout.grid" => layout_grid(),
        "layout.divider" => layout_divider(),
        "action.button" => action_button(scenario),
        "action.icon-button" => action_icon_button(scenario),
        "action.link" => action_link(scenario == "long-label"),
        "action.filter-chip" => action_filter_chip(scenario),
        "field.text-field" => field_text(scenario),
        "field.text-area" => field_area(scenario),
        "field.search-field" => field_search(scenario == "long-label"),
        "field.checkbox" => field_checkbox(scenario),
        "field.radio" => field_radio(scenario),
        "field.switch" => field_switch(scenario == "disabled"),
        "field.select" => field_select(scenario),
        "field.form" => field_form(scenario == "invalid"),
        "identity.icon" => identity_icon(),
        "identity.avatar" => identity_avatar(scenario),
        "identity.avatar-group" => identity_group(scenario == "absent-identity"),
        "identity.author-byline" => identity_byline(scenario == "long-label"),
        "identity.image-frame" => identity_frame(scenario == "image-error"),
        "composition.nested" => composition(scenario == "narrow"),
        other => {
            return crate::states::specimen(other, scenario)
                .or_else(|| crate::navigation::specimen(other, scenario));
        }
    })
}

fn text_text(long: bool) -> AnyView {
    if long {
        return view! {
            <Stack gap=Space::S4>
                <Text role=TypeRole::Title>{fixture::LONG_TITLE}</Text>
                <Text role=TypeRole::Body>{fixture::COMBINING}</Text>
                <Text role=TypeRole::Reading>{fixture::READING}</Text>
            </Stack>
        }
        .into_any();
    }
    view! {
        <Stack gap=Space::S3>
            {TypeRole::ALL
                .into_iter()
                .map(|role| view! {
                    <div class="vkp-row" data-role=role.id()>
                        <code>{role.text_role().token}</code>
                        <Text role=role>{match role {
                            TypeRole::Reading => fixture::READING,
                            TypeRole::Code => fixture::CODE,
                            TypeRole::Display => fixture::DISPLAY,
                            TypeRole::Title => fixture::TITLE,
                            TypeRole::Label => fixture::LABEL,
                            TypeRole::Caption => fixture::CAPTION,
                            _ => fixture::BODY,
                        }}</Text>
                    </div>
                })
                .collect_view()}
            <Divider decorative=true />
            <Stack direction=Direction::Row gap=Space::S4 wrap=true>
                {[TextTone::Default, TextTone::Muted, TextTone::Primary, TextTone::Error, TextTone::Warning]
                    .into_iter()
                    .map(|tone| view! { <Text tone=tone tag=TextTag::Span>{format!("Tông {}", tone.id())}</Text> })
                    .collect_view()}
            </Stack>
        </Stack>
    }
    .into_any()
}

fn text_heading() -> AnyView {
    view! {
        {note("Cấp tiêu đề (h1–h4) quyết định dàn ý; vai trò chữ chỉ đổi hình thức.")}
        <Stack gap=Space::S3>
            <Heading level=HeadingLevel::H2 role=TypeRole::Headline>"h2 · headline — Những trang viết đầu tiên"</Heading>
            <Heading level=HeadingLevel::H3>"h3 · title (mặc định) — Ghi chép từ phố Hàng Đường"</Heading>
            <Heading level=HeadingLevel::H4>"h4 · label (mặc định) — Khổ thơ chưa có tên"</Heading>
            <Heading level=HeadingLevel::H5 role=TypeRole::Caption tone=TextTone::Muted>"h5 · caption — Cập nhật 09:00"</Heading>
        </Stack>
    }
    .into_any()
}

fn tones() -> impl IntoView {
    view! {
        <Grid min=GridMin::Small gap=Space::S3>
            {[SurfaceTone::Base, SurfaceTone::Low, SurfaceTone::Container, SurfaceTone::High, SurfaceTone::Highest, SurfaceTone::Primary]
                .into_iter()
                .map(|tone| view! {
                    <Surface tone=tone radius=Radius::Lg padding=Space::S4>
                        <Text role=TypeRole::Label>{format!("Surface {}", tone.id())}</Text>
                        <Text role=TypeRole::Caption>"Chữ trên nền"</Text>
                    </Surface>
                })
                .collect_view()}
        </Grid>
    }
}

fn layout_surface(nested: bool) -> AnyView {
    if !nested {
        return tones().into_any();
    }
    view! {
        <ThemeScope theme=Some(ThemePreference::Dark) label="Scope Tối">
            <div class="vkp-nested__pane" data-nested="dark">
                <Text role=TypeRole::Label>"Tối (tường minh)"</Text>
                {tones()}
                <ThemeScope theme=Some(ThemePreference::Light) label="Scope Sáng">
                    <div class="vkp-nested__pane" data-nested="light">
                        <Text role=TypeRole::Label>"Sáng bên trong Tối"</Text>
                        {tones()}
                    </div>
                </ThemeScope>
            </div>
        </ThemeScope>
    }
    .into_any()
}

fn layout_container() -> AnyView {
    view! {
        <Stack gap=Space::S4>
            {[Measure::Reading, Measure::Wide, Measure::Full]
                .into_iter()
                .map(|measure| view! {
                    <div class="vkp-outline" data-measure=measure.id()>
                        <Container measure=measure>
                            <Surface tone=SurfaceTone::Container padding=Space::S3>
                                <Text role=TypeRole::Caption>{format!("Container {}", measure.id())}</Text>
                                <Text>{fixture::BODY}</Text>
                            </Surface>
                        </Container>
                    </div>
                })
                .collect_view()}
        </Stack>
    }
    .into_any()
}

fn layout_stack(long: bool) -> AnyView {
    let labels: &[&str] = if long {
        &[
            "Truyện ngắn về mùa thu Hà Nội",
            "Tùy bút",
            "Thơ lục bát có dấu tổ hợp",
            "Phê bình văn học đương đại",
            "Ký",
        ]
    } else {
        &["Thơ", "Văn xuôi", "Tùy bút"]
    };
    view! {
        <Stack gap=Space::Density>
            <Text role=TypeRole::Label>"Stack dọc · khoảng density.stack-gap"</Text>
            <Stack direction=Direction::Row gap=Space::S2 align=Align::Center wrap=true>
                {labels
                    .iter()
                    .map(|label| view! { <Surface tone=SurfaceTone::High radius=Radius::Sm padding=Space::S2><Text tag=TextTag::Span>{*label}</Text></Surface> })
                    .collect_view()}
            </Stack>
            <Stack direction=Direction::Row gap=Space::S3 wrap=true>
                <Button variant=ButtonVariant::Tonal on_press=|_| log("action.button → Hủy")>"Hủy"</Button>
                <Button on_press=|_| log("action.button → Lưu bản nháp")>{fixture::LABEL}</Button>
            </Stack>
        </Stack>
    }
    .into_any()
}

fn layout_grid() -> AnyView {
    view! {
        <Stack gap=Space::S6>
            <Text role=TypeRole::Label>"GridMin::Small (10rem)"</Text>
            <Grid min=GridMin::Small gap=Space::S3>
                {(1..=6).map(|n| view! { <Surface tone=SurfaceTone::Container radius=Radius::Md padding=Space::S3><Text>{format!("Ô {n}")}</Text></Surface> }).collect_view()}
            </Grid>
            <Text role=TypeRole::Label>"GridMin::Medium (15rem)"</Text>
            <Grid gap=Space::S4>
                {[fixture::TITLE, fixture::HEADLINE, fixture::LONG_TITLE]
                    .into_iter()
                    .map(|title| view! { <Surface tone=SurfaceTone::Low radius=Radius::Lg padding=Space::S4><Text role=TypeRole::Title>{title}</Text></Surface> })
                    .collect_view()}
            </Grid>
        </Stack>
    }
    .into_any()
}

fn layout_divider() -> AnyView {
    view! {
        <Stack gap=Space::S3>
            <Text>"Phần một: bản nháp."</Text>
            <Divider />
            <Text>"Phần hai: bản đã gửi. Divider có nghĩa là <hr>."</Text>
            <Divider decorative=true />
            <Text tone=TextTone::Muted>"Divider trang trí bị ẩn khỏi công nghệ hỗ trợ."</Text>
        </Stack>
    }
    .into_any()
}

fn action_button(scenario: &str) -> AnyView {
    let presses = RwSignal::new(0u32);
    let press = move |what: &'static str| {
        move |_| {
            presses.update(|n| *n += 1);
            log(format!(
                "action.button → {what} (lần {})",
                presses.get_untracked()
            ));
        }
    };
    let counter = move || {
        view! { <p class="vkp-note" data-testid="press-count">{move || format!("Đã nhận {} lần kích hoạt", presses.get())}</p> }
    };
    match scenario {
        "disabled" => view! {
            {counter()}
            <Stack direction=Direction::Row gap=Space::S4 wrap=true align=Align::Start>
                {ButtonVariant::ALL
                    .into_iter()
                    .map(|variant| {
                        let id = format!("disabled-{}", variant.id());
                        view! {
                            <Button
                                id=id
                                variant=variant
                                state=ActionState::disabled("Chưa có thay đổi để lưu")
                                on_press=press("disabled")
                            >
                                {format!("Lưu ({})", variant.id())}
                            </Button>
                        }
                    })
                    .collect_view()}
            </Stack>
        }
        .into_any(),
        "pending" => {
            let state = RwSignal::new(ActionState::Enabled);
            view! {
                {counter()}
                <Stack direction=Direction::Row gap=Space::S4 wrap=true align=Align::Start>
                    <Button
                        id="pending-save"
                        state=state
                        on_press=move |_| {
                            presses.update(|n| *n += 1);
                            state.set(ActionState::pending("Đang lưu… (chờ xác nhận)"));
                            log(format!("action.button → Lưu (lần {}) → pending", presses.get_untracked()));
                        }
                    >
                        "Lưu bản nháp"
                    </Button>
                    <Button
                        variant=ButtonVariant::Text
                        on_press=move |_| {
                            state.set(ActionState::Enabled);
                            log("action.button → hoàn tất (caller trả Enabled)");
                        }
                    >
                        "Mô phỏng hoàn tất"
                    </Button>
                </Stack>
            }
            .into_any()
        }
        "long-label" => view! {
            {counter()}
            <Stack gap=Space::S3 align=Align::Start>
                <Button leading=IconName::Add on_press=press("long")>
                    "Tạo bản nháp mới cho tập truyện ngắn về những con phố cũ của Hà Nội"
                </Button>
                <Button variant=ButtonVariant::Outlined on_press=press("long")>"Xem trước bài viết"</Button>
            </Stack>
        }
        .into_any(),
        _ => view! {
            {counter()}
            <Stack direction=Direction::Row gap=Space::S3 wrap=true align=Align::Center>
                <span data-testid="press-once">
                    <Button on_press=press("Filled")>"Lưu bản nháp"</Button>
                </span>
                <Button variant=ButtonVariant::Tonal leading=IconName::Visibility on_press=press("Tonal")>"Xem trước"</Button>
                <Button variant=ButtonVariant::Outlined on_press=press("Outlined")>"Tải .txt"</Button>
                <Button variant=ButtonVariant::Text on_press=press("Text")>"Hủy"</Button>
                <Button variant=ButtonVariant::Danger on_press=press("Danger")>"Xóa bản nháp"</Button>
            </Stack>
            <form
                class="vkp-form-probe"
                data-testid="form-probe"
                on:submit=move |ev| {
                    ev.prevent_default();
                    log("form → submit");
                }
            >
                <Text role=TypeRole::Caption tone=TextTone::Muted>
                    "Trong form: Button mặc định không gửi form; chỉ ButtonKind::Submit gửi."
                </Text>
                <Stack direction=Direction::Row gap=Space::S3 wrap=true>
                    <span data-testid="in-form-button">
                        <Button variant=ButtonVariant::Tonal on_press=press("trong form")>"Không gửi"</Button>
                    </span>
                    <Button kind=ButtonKind::Submit>"Gửi (Submit)"</Button>
                </Stack>
            </form>
        }
        .into_any(),
    }
}

fn action_icon_button(scenario: &str) -> AnyView {
    match scenario {
        "disabled" => view! {
            <Stack direction=Direction::Row gap=Space::S4 wrap=true align=Align::Start>
                <IconButton id="icon-disabled" icon=IconName::Edit label="Sửa tiêu đề" state=ActionState::disabled("Bản đã gửi không sửa được") on_press=|_| log("icon-button → Sửa (không được gọi)") />
            </Stack>
        }
        .into_any(),
        "selected" => {
            let on = RwSignal::new(true);
            view! {
                <Stack direction=Direction::Row gap=Space::S3 align=Align::Center>
                    <IconButton
                        icon=IconName::Visibility
                        label="Xem trước"
                        pressed=on
                        on_press=move |_| {
                            on.update(|v| *v = !*v);
                            log(format!("icon-button → Xem trước = {}", on.get_untracked()));
                        }
                    />
                    <Text tag=TextTag::Span>{move || if on.get() { "Xem trước: bật" } else { "Xem trước: tắt" }}</Text>
                </Stack>
            }
            .into_any()
        }
        _ => view! {
            <Stack direction=Direction::Row gap=Space::S2 align=Align::Center>
                <IconButton icon=IconName::ArrowBack label="Quay lại" on_press=|_| log("icon-button → Quay lại") />
                <IconButton icon=IconName::Edit label="Sửa tiêu đề" on_press=|_| log("icon-button → Sửa tiêu đề") />
                <IconButton icon=IconName::Add label="Tạo bản nháp" on_press=|_| log("icon-button → Tạo bản nháp") />
                <IconButton icon=IconName::Close label="Đóng" on_press=|_| log("icon-button → Đóng") />
            </Stack>
        }
        .into_any(),
    }
}

fn action_link(long: bool) -> AnyView {
    if long {
        return view! {
            <Container measure=Measure::Reading>
                <p class="vk-ui-text vk-ui-type-body">
                    "Đọc tiếp "
                    <Link href="/tac-pham/mau-1">"Một tiêu đề rất dài để kiểm tra việc xuống dòng của đường link tiếng Việt có dấu ở màn hình hẹp"</Link>
                    " rồi quay lại."
                </p>
            </Container>
        }
        .into_any();
    }
    view! {
        <Stack gap=Space::S3>
            <p class="vk-ui-text vk-ui-type-body">
                "Link trong đoạn: đọc "
                <Link href="/tac-pham/mau-1">"Ghi chép từ phố Hàng Đường"</Link>
                " trên Khám phá."
            </p>
            <Link href="/ban-viet" variant=LinkVariant::Standalone>"Về Bàn viết"</Link>
            <Link href="https://github.com/google/material-design-icons" target=LinkTarget::NewTab variant=LinkVariant::Standalone>
                "Nguồn icon (Apache-2.0)"
            </Link>
            <span data-testid="refused-link">
                <Link href="javascript:alert(1)">"URL bị từ chối hiển thị như chữ"</Link>
            </span>
        </Stack>
    }
    .into_any()
}

fn action_filter_chip(scenario: &str) -> AnyView {
    let genres = [
        ("tho", "Thơ"),
        ("van-xuoi", "Văn xuôi"),
        ("tuy-but", "Tùy bút"),
        ("ky", "Ký"),
    ];
    let disabled = scenario == "disabled";
    let initially = scenario == "selected";
    view! {
        <Stack direction=Direction::Row gap=Space::S2 wrap=true attr:data-testid="chips">
            {genres
                .into_iter()
                .enumerate()
                .map(|(i, (id, label))| {
                    let selected = RwSignal::new(initially && i % 2 == 0);
                    // One visible reason for the row; the other chips are disabled without repeating it.
                    let state = match (disabled, i) {
                        (true, 0) => ActionState::disabled("Bộ lọc chưa dùng được"),
                        (true, _) => ActionState::Disabled { reason: None },
                        _ => ActionState::Enabled,
                    };
                    view! {
                        <FilterChip
                            id=format!("chip-{id}")
                            label=label
                            selected=selected
                            state=state
                            on_toggle=move |next: bool| {
                                selected.set(next);
                                log(format!("filter-chip → {id} = {next}"));
                            }
                        />
                    }
                })
                .collect_view()}
        </Stack>
    }
    .into_any()
}

fn text_logger(
    field: &'static str,
    value: RwSignal<String>,
) -> impl Fn(TextInput) + Copy + Send + Sync + 'static {
    move |input: TextInput| {
        value.set(input.value.clone());
        log(format!(
            "{field} → {:?}{}",
            input.value,
            if input.composing {
                " (đang soạn IME)"
            } else {
                ""
            }
        ));
    }
}

fn field_text(scenario: &str) -> AnyView {
    let (value, mode, error, label) = match scenario {
        "invalid" => (
            "",
            FieldMode::Editable,
            Some("Cần có tiêu đề trước khi gửi."),
            "Tiêu đề",
        ),
        "read-only" => (
            fixture::TITLE,
            FieldMode::ReadOnly,
            None,
            "Tiêu đề (bản đã gửi)",
        ),
        "disabled" => (fixture::TITLE, FieldMode::Disabled, None, "Tiêu đề"),
        "long-label" => (
            "0",
            FieldMode::Editable,
            None,
            "Số chương dự kiến cho tập truyện ngắn đầu tay của bạn (có thể để 0)",
        ),
        _ => ("", FieldMode::Editable, None, "Tiêu đề"),
    };
    let text = RwSignal::new(value.to_string());
    let email = RwSignal::new(String::new());
    let variant = if scenario == "external-label" {
        FieldVariant::ExternalLabel
    } else {
        FieldVariant::FloatingTonal
    };
    view! {
        <Stack gap=Space::S6>
            <TextField
                id="title"
                variant=variant
                label=label
                value=text
                on_input=text_logger("title", text)
                help="Có thể sửa sau; Vink không tự cắt khoảng trắng."
                error=Signal::stored(error.map(str::to_string))
                required=true
                mode=mode
            />
            <TextField
                id="contact"
                label="Email liên hệ"
                kind=TextKind::Email
                value=email
                on_input=text_logger("contact", email)
                autocomplete="email"
            />
            <p class="vkp-note" data-testid="title-value">{move || format!("Giá trị caller giữ: {:?}", text.get())}</p>
        </Stack>
    }
    .into_any()
}

fn field_area(scenario: &str) -> AnyView {
    let initial = if scenario == "multiline" {
        fixture::READING
    } else {
        ""
    };
    let body = RwSignal::new(initial.to_string());
    let error =
        (scenario == "invalid").then(|| "Nội dung trống: thêm ít nhất một dòng.".to_string());
    view! {
        <TextArea
            id="body"
            label="Nội dung"
            value=body
            rows=6
            on_input=text_logger("body", body)
            help="Dòng trống giữa hai khổ được giữ nguyên."
            error=Signal::stored(error)
            required=true
        />
        <p class="vkp-note" data-testid="body-lines">{move || format!("{} dòng", body.get().split('\n').count())}</p>
    }
    .into_any()
}

fn field_search(long: bool) -> AnyView {
    let query = RwSignal::new(if long {
        "Hàng Đường".to_string()
    } else {
        String::new()
    });
    let label = if long {
        "Tìm trong các tác phẩm đã tải theo tiêu đề hoặc tên tác giả, có hoặc không dấu"
    } else {
        "Tìm tác phẩm"
    };
    view! {
        <SearchField
            id="search"
            label=label
            value=query
            on_input=text_logger("search", query)
            on_submit=move |q: String| log(format!("search → gửi {q:?}"))
            placeholder="Ví dụ: mùa thu"
            help="Enter để tìm; không gửi khi đang gõ dấu."
        />
    }
    .into_any()
}

fn field_checkbox(scenario: &str) -> AnyView {
    let agree = RwSignal::new(false);
    let draft = RwSignal::new(true);
    let disabled = scenario == "disabled";
    let error = (scenario == "invalid").then(|| "Cần đồng ý trước khi gửi duyệt.".to_string());
    view! {
        <Stack gap=Space::S2>
            <Checkbox
                id="agree"
                label="Tôi xác nhận đây là tác phẩm của mình"
                checked=agree
                on_change=move |v: bool| {
                    agree.set(v);
                    log(format!("checkbox agree = {v}"));
                }
                help="Không thay cho quy định cộng đồng."
                error=Signal::stored(error)
                disabled=disabled
            />
            <Checkbox
                id="keep-draft"
                label="Giữ bản nháp sau khi gửi"
                checked=draft
                on_change=move |v: bool| {
                    draft.set(v);
                    log(format!("checkbox keep-draft = {v}"));
                }
                disabled=disabled
            />
        </Stack>
    }
    .into_any()
}

fn genres() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("tho", "Thơ"),
        ChoiceOption::new("van-xuoi", "Văn xuôi"),
        ChoiceOption::new("tuy-but", "Tùy bút"),
        ChoiceOption {
            value: "kich".into(),
            label: "Kịch (chưa hỗ trợ)".into(),
            disabled: true,
        },
    ]
}

fn field_radio(scenario: &str) -> AnyView {
    let value = RwSignal::new((scenario == "default").then(|| "tho".to_string()));
    let error = (scenario == "invalid").then(|| "Chọn một thể loại.".to_string());
    view! {
        <RadioGroup
            id="genre"
            legend="Thể loại"
            options=genres()
            value=value
            on_change=move |v: String| {
                log(format!("radio genre = {v}"));
                value.set(Some(v));
            }
            help="Giúp người đọc tìm thấy tác phẩm."
            error=Signal::stored(error)
            required=true
            disabled=scenario == "disabled"
        />
    }
    .into_any()
}

fn field_switch(disabled: bool) -> AnyView {
    let on = RwSignal::new(false);
    view! {
        <Switch
            id="notify"
            label="Báo khi có nhận xét mới"
            checked=on
            on_change=move |v: bool| {
                on.set(v);
                log(format!("switch notify = {v}"));
            }
            help="Chỉ là tùy chọn hiển thị trong preview."
            disabled=disabled
        />
    }
    .into_any()
}

fn field_select(scenario: &str) -> AnyView {
    let value = RwSignal::new(None::<String>);
    let error = (scenario == "invalid").then(|| "Chọn thể loại trước khi gửi.".to_string());
    let pick = move |v: Option<String>| {
        log(format!("select genre = {v:?}"));
        value.set(v);
    };
    view! {
        <Select
            id="genre-select"
            label="Thể loại"
            options=genres()
            value=value
            on_change=pick
            placeholder="Chọn thể loại…"
            error=Signal::stored(error)
            required=true
            disabled=scenario == "disabled"
        />
    }
    .into_any()
}

/// Field messages and the summary come from the caller's validation on submit.
fn field_form(start_invalid: bool) -> AnyView {
    let title = RwSignal::new(String::new());
    let genre = RwSignal::new(None::<String>);
    let agree = RwSignal::new(false);
    let submitted = RwSignal::new(start_invalid);
    let errors = Memo::new(move |_| {
        if !submitted.get() {
            return Vec::new();
        }
        let mut items = Vec::new();
        if title.with(|t| t.trim().is_empty()) {
            items.push((
                "form-title".to_string(),
                "Tiêu đề: cần có tiêu đề.".to_string(),
            ));
        }
        if genre.with(Option::is_none) {
            items.push((
                "form-genre".to_string(),
                "Thể loại: chọn một thể loại.".to_string(),
            ));
        }
        if !agree.get() {
            items.push((
                "form-agree".to_string(),
                "Xác nhận: cần đồng ý.".to_string(),
            ));
        }
        items
    });
    let error_for = move |id: &'static str| {
        Signal::derive(move || {
            errors
                .get()
                .into_iter()
                .find(|(field, _)| field == id)
                .map(|(_, message)| {
                    message
                        .split_once(": ")
                        .map_or(message.clone(), |(_, m)| m.to_string())
                })
        })
    };
    let summary = Signal::derive(move || errors.get());
    let pick_genre = move |v: Option<String>| genre.set(v);
    view! {
        <form
            class="vkp-form"
            novalidate=true
            on:submit=move |ev| {
                ev.prevent_default();
                submitted.set(true);
                log(format!("form → submit, {} lỗi", errors.get_untracked().len()));
            }
        >
            <Stack gap=Space::S4>
                <FormSummary title="Cần sửa trước khi gửi" items=summary />
                <TextField id="form-title" label="Tiêu đề" value=title on_input=text_logger("form-title", title) required=true error=error_for("form-title") />
                <Select
                    id="form-genre"
                    label="Thể loại"
                    options=genres()
                    value=genre
                    on_change=pick_genre
                    required=true
                    error=error_for("form-genre")
                />
                <Checkbox
                    id="form-agree"
                    label="Tôi xác nhận đây là tác phẩm của mình"
                    checked=agree
                    on_change=move |v: bool| agree.set(v)
                    error=error_for("form-agree")
                />
                <Stack direction=Direction::Row gap=Space::S3 wrap=true>
                    <Button kind=ButtonKind::Submit>"Gửi duyệt (mẫu)"</Button>
                    <Button variant=ButtonVariant::Text on_press=move |_| { submitted.set(false); log("form → bỏ kiểm"); }>"Bỏ kiểm"</Button>
                </Stack>
            </Stack>
        </form>
    }
    .into_any()
}

fn identity_icon() -> AnyView {
    view! {
        <p class="vkp-note">{format!("Nguồn: {}", vink_ui::icons::SOURCE)}</p>
        <Grid min=GridMin::Small gap=Space::S2>
            {IconName::ALL
                .into_iter()
                .map(|name| view! {
                    <div class="vkp-icon-cell" data-icon-cell=name.id()>
                        <Icon name=name label=name.id() />
                        <code>{name.id()}</code>
                    </div>
                })
                .collect_view()}
        </Grid>
    }
    .into_any()
}

fn avatar(index: usize, size: AvatarSize, image: ImageState) -> impl IntoView {
    let (key, name) = person(index);
    view! {
        <Avatar
            name=name.unwrap_or_default()
            identity_key=key.unwrap_or_default()
            image=Signal::stored(image)
            size=size
        />
    }
}

fn identity_avatar(scenario: &str) -> AnyView {
    match scenario {
        "image-error" => view! {
            {note("Ảnh hợp lệ hiện trên initials; AVIF giải mã lỗi, URL javascript: và Failed đều quay về initials.")}
            <Stack direction=Direction::Row gap=Space::S6 wrap=true align=Align::Center attr:data-testid="avatar-images">
                <figure class="vkp-figure" data-case="ready">{avatar(0, AvatarSize::Reader, ImageState::ready(sample_png()))}<figcaption>"Ready PNG"</figcaption></figure>
                <figure class="vkp-figure" data-case="broken">{avatar(1, AvatarSize::Reader, ImageState::ready(broken_avif()))}<figcaption>"AVIF lỗi"</figcaption></figure>
                <figure class="vkp-figure" data-case="unsafe">{avatar(4, AvatarSize::Reader, ImageState::ready("javascript:alert(1)"))}<figcaption>"URL bị từ chối"</figcaption></figure>
                <figure class="vkp-figure" data-case="failed">{avatar(5, AvatarSize::Reader, ImageState::Failed)}<figcaption>"Failed"</figcaption></figure>
                <figure class="vkp-figure" data-case="loading">{avatar(0, AvatarSize::Reader, ImageState::Loading)}<figcaption>"Loading"</figcaption></figure>
            </Stack>
        }
        .into_any(),
        "absent-identity" => view! {
            {note("Thiếu tên → dấu người trung tính; thiếu key → tông trung tính. Không suy ra tài khoản từ tên.")}
            <Stack direction=Direction::Row gap=Space::S6 wrap=true align=Align::Center>
                <figure class="vkp-figure" data-case="none"><Avatar size=AvatarSize::Feed /><figcaption>"Không tên, không key"</figcaption></figure>
                <figure class="vkp-figure" data-case="name-only"><Avatar name="An Nhiên" size=AvatarSize::Feed /><figcaption>"Có tên, thiếu key"</figcaption></figure>
                <figure class="vkp-figure" data-case="key-only"><Avatar identity_key="acct-003" size=AvatarSize::Feed kind=AvatarKind::Account /><figcaption>"Có key, thiếu tên"</figcaption></figure>
                <figure class="vkp-figure" data-case="emoji"><Avatar name="🌸🌸" identity_key="acct-001" size=AvatarSize::Feed /><figcaption>"Tên chỉ có emoji"</figcaption></figure>
            </Stack>
        }
        .into_any(),
        "long-label" => {
            let order = RwSignal::new(vec![2usize, 3, 0, 1]);
            view! {
                {note("Trần Linh và Trần Lâm cùng initials TL nhưng khác key nên khác tông; đảo thứ tự không đổi màu.")}
                <Button variant=ButtonVariant::Tonal on_press=move |_| {
                    order.update(|o| o.reverse());
                    log("avatar → đảo thứ tự");
                }>"Đảo thứ tự"</Button>
                <Stack direction=Direction::Row gap=Space::S6 wrap=true align=Align::Center attr:data-testid="avatar-order">
                    {move || order
                        .get()
                        .into_iter()
                        .map(|i| {
                            let (key, name) = person(i);
                            view! {
                                <figure class="vkp-figure" data-key=key.unwrap_or_default()>
                                    {avatar(i, AvatarSize::Feed, ImageState::Absent)}
                                    <figcaption>{format!("{} · {}", name.unwrap_or_default(), key.unwrap_or_default())}</figcaption>
                                </figure>
                            }
                        })
                        .collect_view()}
                </Stack>
            }
            .into_any()
        }
        _ => view! {
            <Stack gap=Space::S4>
                <Stack direction=Direction::Row gap=Space::S6 wrap=true align=Align::End>
                    {[AvatarSize::Small, AvatarSize::Medium, AvatarSize::Spotlight, AvatarSize::Feed, AvatarSize::Reader]
                        .into_iter()
                        .map(|size| view! {
                            <figure class="vkp-figure" data-size=size.id()>{avatar(0, size, ImageState::Absent)}<figcaption>{size.id()}</figcaption></figure>
                        })
                        .collect_view()}
                </Stack>
                <Stack direction=Direction::Row gap=Space::S4 wrap=true align=Align::Center attr:data-testid="avatar-tones">
                    {(0..6).map(|i| avatar(i, AvatarSize::Feed, ImageState::Absent)).collect_view()}
                </Stack>
            </Stack>
        }
        .into_any(),
    }
}

fn people(count: usize) -> Vec<Person> {
    PEOPLE
        .iter()
        .take(count)
        .map(|(key, name)| Person {
            key: key.map(str::to_string),
            name: name.map(str::to_string),
            image: ImageState::Absent,
        })
        .collect()
}

fn identity_group(unknown: bool) -> AnyView {
    if unknown {
        return view! {
            {note("Chưa biết tổng: không hiện +N, không hiện 0.")}
            <span data-testid="group-unknown"><AvatarGroup people=people(5) label="Người đã nhận xét" /></span>
        }
        .into_any();
    }
    view! {
        {note("Tổng 12 do caller cấp: hiện 3 avatar + 9. Cả nhóm có một nhãn, không có điểm dừng focus.")}
        <span data-testid="group-known"><AvatarGroup people=people(5) label="Người đã nhận xét" total=12 /></span>
        <AvatarGroup people=people(4) label="Người theo dõi" total=4 max_visible=4 size=AvatarSize::Medium />
    }
    .into_any()
}

fn identity_byline(long: bool) -> AnyView {
    if long {
        let (key, name) = person(6);
        return view! {
            <Container measure=Measure::Reading>
                <AuthorByline
                    name=name.unwrap_or_default()
                    identity_key=key.unwrap_or_default()
                    meta=vec!["Cập nhật 09:00 · 02/11/2026".to_string(), "Truyện ngắn".to_string(), "12 phút đọc".to_string()]
                />
            </Container>
        }
        .into_any();
    }
    let rows = [
        (AvatarSize::Feed, "feed", None),
        (AvatarSize::Spotlight, "spotlight", None),
        (AvatarSize::Reader, "reader", Some("/tac-gia/mau-an-nhien")),
    ];
    view! {
        {note("Tên đậm trước metadata; avatar ẩn với công nghệ hỗ trợ. Chỉ dòng reader có href (giả định đã xác minh).")}
        <Stack gap=Space::S6>
            {rows
                .into_iter()
                .map(|(size, label, href)| {
                    let (key, name) = person(0);
                    view! {
                        <div data-byline=label>
                            <AuthorByline
                                name=name.unwrap_or_default()
                                identity_key=key.unwrap_or_default()
                                size=size
                                href=href.map(str::to_string).unwrap_or_default()
                                meta=vec![fixture::CAPTION.to_string(), format!("byline {label}")]
                            />
                        </div>
                    }
                })
                .collect_view()}
            <div data-byline="blank"><AuthorByline name="  " meta=vec!["Tên trống được nói rõ".to_string()] /></div>
        </Stack>
    }
    .into_any()
}

fn identity_frame(errors: bool) -> AnyView {
    if errors {
        return view! {
            <Grid min=GridMin::Small gap=Space::S4 attr:data-testid="frames">
                <div data-case="broken"><ImageFrame image=ImageState::ready(broken_avif()) alt="[Mẫu] Bìa tập thơ" /></div>
                <div data-case="failed"><ImageFrame image=ImageState::Failed alt="[Mẫu] Bìa tập thơ" /></div>
                <div data-case="absent"><ImageFrame image=ImageState::Absent alt="" ratio=AspectRatio::Square /></div>
                <div data-case="loading"><ImageFrame image=ImageState::Loading alt="[Mẫu] Ảnh minh họa" ratio=AspectRatio::Wide /></div>
            </Grid>
        }
        .into_any();
    }
    view! {
        <Grid gap=Space::S4>
            <ImageFrame image=ImageState::ready(sample_png()) alt="[Mẫu] Ảnh nền lavender" />
            <ImageFrame image=ImageState::ready(sample_png()) alt="" ratio=AspectRatio::Wide />
        </Grid>
    }
    .into_any()
}

fn composition(narrow: bool) -> AnyView {
    let title = RwSignal::new(fixture::TITLE.to_string());
    let genre = RwSignal::new(Some("van-xuoi".to_string()));
    let state = RwSignal::new(ActionState::Enabled);
    let (key, name) = person(0);
    let (other_key, other_name) = person(1);
    let body = view! {
        <ThemeScope theme=Some(ThemePreference::Dark) label="Bàn viết (Tối)">
            <div class="vkp-nested__pane" data-nested="dark">
                <Surface tone=SurfaceTone::Low radius=Radius::Xl padding=Space::S6 tag=vink_ui::SurfaceTag::Section label="Bản nháp mẫu">
                    <Stack gap=Space::S4>
                        <AuthorByline name=name.unwrap_or_default() identity_key=key.unwrap_or_default() meta=vec![fixture::CAPTION.to_string()] />
                        <TextField id="compose-title" label="Tiêu đề" value=title on_input=text_logger("compose-title", title) required=true />
                        <RadioGroup id="compose-genre" legend="Thể loại" options=genres() value=genre on_change=move |v: String| genre.set(Some(v)) />
                        <Stack direction=Direction::Row gap=Space::S3 wrap=true align=Align::Start>
                            <Button id="compose-save" state=state on_press=move |_| { state.set(ActionState::pending("Đang lưu…")); log("composition → Lưu"); }>"Lưu bản nháp"</Button>
                            <Button variant=ButtonVariant::Text on_press=move |_| state.set(ActionState::Enabled)>"Đặt lại"</Button>
                        </Stack>
                        <ThemeScope theme=Some(ThemePreference::Light) label="Thẻ đọc (Sáng)">
                            <div class="vkp-nested__pane" data-nested="light">
                                <Surface tone=SurfaceTone::Base radius=Radius::Lg padding=Space::S4 tag=vink_ui::SurfaceTag::Article>
                                    <Stack gap=Space::S2>
                                        <AuthorByline name=other_name.unwrap_or_default() identity_key=other_key.unwrap_or_default() size=AvatarSize::Spotlight meta=vec!["Thơ".to_string()] />
                                        <Text role=TypeRole::Reading>{fixture::READING}</Text>
                                        <AvatarGroup people=people(3) label="Người đã nhận xét" />
                                    </Stack>
                                </Surface>
                            </div>
                        </ThemeScope>
                    </Stack>
                </Surface>
            </div>
        </ThemeScope>
    };
    if narrow {
        view! { <div class="vkp-narrow" data-testid="narrow-frame">{body}</div> }.into_any()
    } else {
        body.into_any()
    }
}
