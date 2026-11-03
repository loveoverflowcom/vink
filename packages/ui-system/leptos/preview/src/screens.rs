//! Deterministic sample adapter. Only this consumer imports fixture data.
use leptos::prelude::*;
use vink_patterns::*;
use vink_ui::catalog::fixture;
use vink_ui::*;

fn author(long: bool) -> Author {
    Author {
        name: if long {
            fixture::COMBINING
        } else {
            "[Mẫu] Nguyễn Thị Ánh Tuyết"
        }
        .into(),
        key: Some("screen-author".into()),
        published_at: "2026-11-02T02:00:00Z".into(),
    }
}

fn discussion_sample(anchor: bool) -> DiscussionThreadFacts {
    let author = Author {
        name: "[Mẫu] Lan Anh".into(),
        key: None,
        published_at: "2026-11-03T08:00:00Z".into(),
    };
    let comment = |id: &str, context| DiscussionComment {
        id: id.into(),
        author: author.clone(),
        body: fixture::READING.into(),
        status: None,
        context,
        can_reply: true,
        can_remove: false,
    };
    DiscussionThreadFacts {
        root_id: "sample-root".into(),
        root: (!anchor).then(|| comment("sample-root", None)),
        replies: vec![
            comment("sample-reply", Some(DiscussionContext::Root)),
            comment(
                "sample-context",
                Some(DiscussionContext::Reply {
                    name: "[Mẫu] Lan Anh".into(),
                    href: Some("#comment-sample-reply".into()),
                }),
            ),
            comment("sample-unavailable", Some(DiscussionContext::Unavailable)),
        ],
    }
}

fn comments() -> Vec<Feedback> {
    vec![Feedback {
        id: "feedback-01".into(),
        person: Person {
            name: Some("[Mẫu] Lê Mai".into()),
            key: Some("screen-commenter".into()),
            image: ImageState::Absent,
        },
        text: fixture::BODY.into(),
        context: fixture::TITLE.into(),
        href: "#screen.reader/feedback".into(),
    }]
}

fn draft(long: bool) -> DraftSummary {
    DraftSummary {
        title: if long {
            fixture::LONG_TITLE
        } else {
            fixture::TITLE
        }
        .into(),
        version: 7,
        updated_at: "2026-11-02T02:00:00Z".into(),
        href: "#screen.editor/dirty".into(),
    }
}

pub fn specimen(id: &str, scenario: &str) -> Option<AnyView> {
    if !id.starts_with("screen.") && id != "composition.feedback" {
        return None;
    }
    let (id, scenario) = (id.to_string(), scenario.to_string());
    Some(view! { <ScreenRecipe id scenario /> }.into_any())
}

#[component]
fn ScreenRecipe(id: String, scenario: String) -> impl IntoView {
    let input = RwSignal::new(fixture::READING.to_string());
    let search = RwSignal::new(String::new());
    let theme = RwSignal::new("system".to_string());
    let title = RwSignal::new(
        if scenario == "long" {
            fixture::LONG_TITLE
        } else {
            fixture::TITLE
        }
        .to_string(),
    );
    let message = RwSignal::new(String::new());
    let log = use_context::<super::EventLog>();
    let record = move |text: &str| {
        if let Some(log) = log {
            log.0.run(text.to_string());
        }
    };
    let retry = move |_| {
        message.set("Mẫu · Đã yêu cầu tải lại; không gọi dịch vụ.".into());
        record("screen → retry (sample)");
    };
    let submit = move |_| {
        message.set("Mẫu · Intent gửi nhận xét, chưa gửi hoặc lưu.".into());
        record("feedback → submit intent (sample)");
    };
    let blocked = match scenario.as_str() {
        "pending" => ActionState::pending("Mẫu · Đang chờ phản hồi"),
        "auth-required" => ActionState::disabled("Cần đăng nhập; mẫu không có dịch vụ xác thực."),
        "uncertain" => ActionState::disabled("Chưa rõ kết quả; cần đối chiếu trước khi gửi thêm."),
        _ => ActionState::Enabled,
    };
    let long = scenario == "long";
    let body = if scenario == "loading" {
        view! { <p role="status">"Đang tải…"</p><Skeleton shape=SkeletonShape::Card /> }.into_any()
    } else if matches!(
        scenario.as_str(),
        "empty" | "no-results" | "not-found" | "auth-required" | "forbidden"
    ) && id != "composition.feedback"
        && id != "screen.me"
    {
        let kind = match scenario.as_str() {
            "empty" => PanelKind::Empty(EmptyKind::FirstUse),
            "no-results" => PanelKind::Empty(EmptyKind::NoResults),
            "auth-required" => PanelKind::Error(ProblemKind::AuthRequired),
            "forbidden" => PanelKind::Error(ProblemKind::Forbidden),
            _ => PanelKind::Error(ProblemKind::NotFound),
        };
        view! { <StatePanel content=PanelContent { kind, title: format!("Mẫu · {scenario}"), message: "Không suy ra quyền, tài khoản hay dữ liệu từ trạng thái mẫu.".into(), actions: vec![], code: None } /> }.into_any()
    } else {
        let notice = match scenario.as_str() {
            "error" => Some((
                Tone::Error,
                "Chưa tải/lưu được. Chữ đang viết được giữ nguyên; hãy thử lại.",
            )),
            "conflict" => Some((
                Tone::Warning,
                "Phiên bản đã đổi. Giữ chữ trên trang để đối chiếu, chưa xác nhận lưu.",
            )),
            "uncertain" => Some((
                Tone::Warning,
                "Chưa rõ kết quả. Đối chiếu trước khi gửi thêm; không tự gửi lại.",
            )),
            "refresh" => Some((
                Tone::Info,
                "Đang tải lại. Nội dung đã tải vẫn còn trên trang.",
            )),
            _ => None,
        };
        let content = match id.as_str() {
            "screen.explore" => view! {
                <Heading level=HeadingLevel::H1>"Khám phá"</Heading>
                <Text>"Đọc một câu chuyện, góp một ý hay, rồi viết tiếp."</Text>
                <SearchField id="sample-public-search" label="Tìm trong danh sách mẫu" value=search on_input=move |e: TextInput| search.set(e.value) />
                <Text role=TypeRole::Caption tone=TextTone::Muted>"Mẫu · Chỉ tìm title/tác giả trong danh sách đã tải, tối đa 12 ở app."</Text>
                <div class="vkp-screen-grid">
                    <div class="vk-public-row"><WorkCard title=title.get_untracked() author=author(long) href="#screen.reader/default" /></div>
                    <Surface tone=SurfaceTone::Primary padding=Space::S6 radius=Radius::Lg>
                        <Heading level=HeadingLevel::H2 role=TypeRole::Title>"Một nơi bạn nhớ"</Heading>
                        <Link href="#screen.editor/dirty" variant=LinkVariant::Standalone>"Bắt đầu viết"</Link>
                    </Surface>
                </div>
                <ListEnd />
            }.into_any(),
            "screen.reader" => view! {
                <Link href="#screen.explore/default">"← Khám phá"</Link>
                <ReaderHeader title=title.get_untracked() author=author(long) />
                <Text role=TypeRole::Reading>{fixture::READING}</Text>
                {(scenario == "feedback").then(|| view! {
                    <FeedbackThread items=comments() />
                    <FeedbackComposer id="reader-feedback" intent=FeedbackIntent::Impressions
                        value=input on_input=move |e: TextInput| input.set(e.value)
                        state=blocked.clone() on_submit=submit />
                })}
            }.into_any(),
            "screen.desk" => view! {
                <Heading level=HeadingLevel::H1>"Bàn viết"</Heading>
                <DraftResume draft=draft(long) />
                <SearchField id="sample-draft-search" label="Tìm trong bản thảo đã tải" value=search on_input=move |e: TextInput| search.set(e.value) />
                <Text role=TypeRole::Caption>"Mẫu · Tối đa 100 nháp trong app; bản nháp chỉ bạn đọc được."</Text>
                <DraftRow draft=draft(long) />
            }.into_any(),
            "screen.editor" => view! {
                <Heading level=HeadingLevel::H1 role=TypeRole::Title>"Bản nháp — mẫu chrome"</Heading>
                <EditorToolbar status=format!("Mẫu · {scenario} · Không kết nối lưu") status_token=scenario.clone()>
                    <Button variant=ButtonVariant::Text on_press=move |_| record("editor → preview (sample)")>"Xem trước"</Button>
                    <Button state=blocked.clone() on_press=move |_| { record("editor → save intent (sample)"); message.set("Mẫu · Chỉ ghi intent, chưa lưu lên máy chủ.".into()); }>"Lưu nháp"</Button>
                </EditorToolbar>
                <TextField id="sample-title" label="Tiêu đề" value=title max_length=200 on_input=move |e: TextInput| title.set(e.value) />
                <TextArea id="sample-body" label="Nội dung — controlled specimen" value=input on_input=move |e: TextInput| input.set(e.value) />
                <Text role=TypeRole::Caption>"Mẫu không có CodeMirror, backend hay native editor parity. Editor app được kiểm riêng."</Text>
            }.into_any(),
            "screen.me" => view! {
                <Heading level=HeadingLevel::H1>"Tôi"</Heading>
                <Avatar kind=AvatarKind::Account />
                <Text>"Chưa có thông tin tài khoản"</Text>
                <Text tone=TextTone::Muted>"Vink chưa mở đăng nhập."</Text>
                <RadioGroup id="sample-theme" legend="Giao diện" options=vec![ChoiceOption::new("light", "Sáng"), ChoiceOption::new("dark", "Tối"), ChoiceOption::new("system", "Hệ thống")]
                    value=Signal::derive(move || Some(theme.get())) on_change=move |v: String| theme.set(v) />
            }.into_any(),
            _ => view! {
                <RecentFeedback items=comments() />
                {scenario.starts_with("discussion-").then(|| view! {
                    <DiscussionThread facts=discussion_sample(scenario == "discussion-anchor")
                        on_reply=move |_: String| record("feedback → reply intent (sample)")
                        on_remove=move |_: String| record("feedback → remove intent (sample)") />
                })}
                <FeedbackComposer id="sample-feedback" intent=if scenario.starts_with("sharing") { FeedbackIntent::Sharing } else if scenario == "technique" { FeedbackIntent::Technique } else { FeedbackIntent::Impressions }
                    value=input on_input=move |e: TextInput| input.set(e.value) state=blocked
                    comments_closed=scenario == "sharing-closed"
                    on_submit=submit />
            }.into_any(),
        };
        view! {
            {notice.map(|(tone, text)| view! { <InlineNotice tone message=text announce=Announce::Polite /> })}
            {content}
            {(scenario == "error").then(|| view! { <Button on_press=retry>"Thử lại"</Button> })}
        }.into_any()
    };
    view! {
        <section class="vkp-screen" data-screen=id data-scenario=scenario>
            <Text role=TypeRole::Caption tone=TextTone::Muted>"Mẫu · Dữ liệu cố định · Không kết nối dịch vụ"</Text>
            {body}
            <p role="status" data-testid="sample-result">{move || message.get()}</p>
        </section>
    }
}
