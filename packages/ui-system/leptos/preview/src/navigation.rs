//! UIS-04 specimens: shell and navigation primitives through the public `vink_ui` API. The
//! preview has no router: destinations dispatch into an in-memory route (logged), the layout is
//! either fixed per scenario or measured from its frame, and toggles stay in memory. The real
//! router, guard and history are checked against the app by the Web browser smoke.

use leptos::prelude::*;
use vink_ui::catalog::fixture;
use vink_ui::{
    AdaptiveScaffold, BottomNavigation, Button, ButtonVariant, CloseReason, Dialog, DismissReason,
    IconButton, IconName, NavContext, NavDestination, NavLayout, NavUtility, NavigationRail,
    OverlayLayer, RailPreference, Space, Stack, Text, TextArea, TextInput, TextTone, TopBar,
    TypeRole, WindowClass,
};

use crate::components::log;

const PROFILE: &str = "nav-profile";

fn note(text: &'static str) -> impl IntoView {
    view! { <p class="vkp-note">{text}</p> }
}

fn brand() -> impl IntoView {
    view! { <span class="vkp-nav-brand" aria-hidden="true">"Vink"</span> }
}

/// The host's destinations: the order and the set are the host's, never the library's.
fn destination(key: &str, long: bool) -> NavDestination {
    match (key, long) {
        ("explore", false) => NavDestination::new("explore", "Khám phá", IconName::Explore, "/"),
        ("explore", true) => NavDestination::new(
            "explore",
            "Khám phá những tác phẩm mới công khai",
            IconName::Explore,
            "/",
        ),
        ("desk", false) => NavDestination::new("desk", "Bàn viết", IconName::Edit, "/ban-viet"),
        ("desk", true) => NavDestination::new(
            "desk",
            "Bàn viết của tôi và các bản nháp",
            IconName::Edit,
            "/ban-viet",
        ),
        _ => NavDestination::new("me", "Tôi", IconName::Person, "/toi"),
    }
}

/// An in-memory host: route, rail preference and overlay layer, with every intent logged.
#[derive(Clone, Copy)]
struct Host {
    route: RwSignal<&'static str>,
    preference: RwSignal<Option<RailPreference>>,
    overlays: RwSignal<OverlayLayer>,
}

impl Host {
    fn new(route: &'static str) -> Self {
        Self {
            route: RwSignal::new(route),
            preference: RwSignal::new(None),
            overlays: RwSignal::new(OverlayLayer::default()),
        }
    }

    fn navigate(self) -> Callback<String> {
        Callback::new(move |key: String| {
            // A real navigation closes an open overlay first (shared/navigation.md).
            self.overlays.update(|layer| {
                layer.close(PROFILE, CloseReason::Navigate);
            });
            let route = match key.as_str() {
                "explore" => "explore",
                "desk" => "desk",
                _ => "me",
            };
            self.route.set(route);
            log(format!("điều hướng → {key}"));
        })
    }

    /// Reader pages belong to Explore; the editor to Desk.
    fn active(self, key: &'static str) -> Signal<bool> {
        Signal::derive(move || {
            let route = self.route.get();
            route == key
                || (key == "explore" && route == "reader")
                || (key == "desk" && route == "editor")
        })
    }

    fn items(self, keys: &[&'static str], long: bool) -> Vec<(NavDestination, Signal<bool>)> {
        keys.iter()
            .map(|&key| (destination(key, long), self.active(key)))
            .collect()
    }
}

struct Frame {
    window: WindowClass,
    context: NavContext,
    route: &'static str,
    long: bool,
    primary: &'static [&'static str],
    height: u32,
    adaptive: bool,
    rail_only: bool,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            window: WindowClass::Expanded,
            context: NavContext::default(),
            route: "explore",
            long: false,
            primary: &["explore", "desk"],
            height: 460,
            adaptive: false,
            rail_only: false,
        }
    }
}

/// A scaffold in a fixed frame. The page text is a controlled TextArea owned here, outside the
/// scaffold's layout decisions, so toggles and layout changes cannot reset it.
fn shell(frame: Frame, host: Host, page: AnyView, overlays: ViewFn) -> AnyView {
    let Frame {
        window,
        context,
        route,
        long,
        primary,
        height,
        adaptive,
        rail_only,
    } = frame;
    host.route.set(route);
    let viewport = NodeRef::<leptos::html::Div>::new();
    let width = RwSignal::new(0.0);
    #[cfg(feature = "csr")]
    Effect::new(move |_| observe_width(viewport, width));
    let layout = Memo::new(move |_| {
        let window = if adaptive {
            WindowClass::from_css_px(width.get())
        } else {
            window
        };
        NavLayout::resolve(window, context, host.preference.get())
    });
    let navigate = host.navigate();
    let toggle = Callback::new(move |()| {
        if let Some(next) = layout.get_untracked().next_preference() {
            host.preference.set(Some(next));
            log(format!(
                "toggle → {} (giữ trang {})",
                next.value(),
                host.route.get_untracked()
            ));
        }
    });
    let open_profile = Callback::new(move |()| {
        let opened = host.overlays.try_update(|layer| layer.open(PROFILE));
        log(match opened {
            Some(Ok(())) => "Tôi → mở lớp phủ".to_string(),
            Some(Err(busy)) => format!("Tôi → từ chối: {} đang mở", busy.active),
            None => "Tôi → không còn owner".to_string(),
        });
    });
    let profile_open = Signal::derive(move || host.overlays.with(|layer| layer.is_open(PROFILE)));
    let rail_items = host.items(primary, long);
    let bar_keys: Vec<&'static str> = primary.iter().copied().chain(["me"]).collect();
    let bar_items = host.items(&bar_keys, long);
    let me_label = if long {
        "Tôi và giao diện của tôi"
    } else {
        "Tôi"
    };
    let rail = ViewFn::from(move || {
        view! {
            <NavigationRail
                id="nav-rail"
                label="Không gian Vink"
                nav_label="Điều hướng chính"
                expanded=Signal::derive(move || layout.get().rail_expanded())
                expand_label="Mở rộng điều hướng"
                collapse_label="Thu gọn điều hướng"
                on_toggle=toggle
                destinations=rail_items.clone()
                on_navigate=navigate
                brand=brand
                account=move || view! {
                    <NavUtility
                        id="nav-profile-trigger"
                        label=me_label
                        icon=IconName::Person
                        controls=PROFILE
                        expanded=profile_open
                        current=host.active("me")
                        on_open=open_profile
                    />
                }
            />
        }
    });
    let content = if rail_only {
        view! {
            <div class="vk-ui-scaffold vkp-rail-only" data-layout=move || layout.get().id()>
                {rail.run()}
            </div>
        }
        .into_any()
    } else {
        view! {
            <AdaptiveScaffold
                id="nav-shell"
                main_id="nav-main"
                skip_label="Bỏ qua tới nội dung"
                fixed=Signal::derive(move || layout.get())
                context=context
                rail=rail
                top_bar=|| view! { <TopBar brand=brand /> }
                bar=move || view! {
                    <BottomNavigation
                        id="nav-bottom"
                        label="Điều hướng mobile"
                        destinations=bar_items.clone()
                        on_navigate=navigate
                    />
                }
                overlays=overlays
            >
                <main id="nav-main" class="vkp-shell-main" tabindex="-1">
                    {page}
                </main>
            </AdaptiveScaffold>
        }
        .into_any()
    };
    let minimum = if adaptive || rail_only || window == WindowClass::Compact {
        0.0
    } else if window == WindowClass::Expanded {
        WindowClass::CSS_PX.1
    } else {
        WindowClass::CSS_PX.0
    };
    let requested = if rail_only {
        "rail riêng"
    } else if adaptive {
        "adaptive"
    } else {
        window.id()
    };
    view! {
        <p class="vkp-shell-meta" data-testid="nav-route">
            {move || format!("Yêu cầu: {requested} · hiệu lực: {} · frame: {:.1} CSS px · Trang hiện tại: {}",
                layout.get().id(), width.get(), host.route.get())}
        </p>
        <div class="vkp-shell-viewport" node_ref=viewport tabindex="0" role="region"
            aria-label="Khung mẫu điều hướng; khung ép cố định có thể cuộn ngang">
            <div class="vkp-shell-frame"
                style=format!("--vk-ui-scaffold-height: {height}px; min-inline-size: {minimum}px")
                data-requested=requested data-layout=move || layout.get().id()>
                {content}
            </div>
        </div>
    }.into_any()
}

/// One observer per mounted frame. A hidden/zero-width frame starts Compact and is measured
/// again when shown. Cleanup disconnects before dropping the callback; no timer or remount.
#[cfg(feature = "csr")]
fn observe_width(frame: NodeRef<leptos::html::Div>, width: RwSignal<f64>) {
    use wasm_bindgen::{JsCast, closure::Closure};
    let Some(frame) = frame.get() else {
        return;
    };
    let measured = frame.clone();
    let measure = move || {
        let _ = width.try_set(measured.client_width() as f64);
    };
    measure();
    let callback = Closure::<dyn FnMut()>::new(measure);
    if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
        observer.observe(&frame);
        let observer = StoredValue::new_local(Some((observer, callback)));
        on_cleanup(move || {
            if let Some((observer, _)) = observer.try_update_value(Option::take).flatten() {
                observer.disconnect();
            }
        });
    }
}

fn profile_dialog(host: Host) -> AnyView {
    view! {
        <Dialog id=PROFILE title="Tôi" context="Lớp phủ [Mẫu]; không có tài khoản hay lưu."
            on:close=move |_| focus_visible_page()
            open=Signal::derive(move || host.overlays.with(|layer| layer.is_open(PROFILE)))
            on_dismiss=Callback::new(move |_: DismissReason| host.overlays.update(|layer| { layer.close(PROFILE, CloseReason::Dismiss); }))
            actions=move || view! {
                <Button on_press=move |_| host.overlays.update(|layer| { layer.close(PROFILE, CloseReason::Dismiss); })>"Đóng"</Button>
            }
        />
    }.into_any()
}

// A resize may hide the rail trigger while the modal is open. The close event runs after
// native close/focus return; move focus to the still-mounted page when that trigger is hidden.
fn focus_visible_page() {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        if document()
            .get_element_by_id("nav-profile-trigger")
            .is_some_and(|trigger| trigger.get_bounding_client_rect().width() == 0.0)
            && let Some(main) = document()
                .get_element_by_id("nav-main")
                .and_then(|main| main.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = main.focus_with_options(&options);
        }
    }
}

fn draft() -> AnyView {
    let text = RwSignal::new(fixture::READING.to_string());
    view! {
        <TextArea
            id="nav-draft"
            label="Chữ chưa lưu [Mẫu]"
            value=text
            on_input=move |input: TextInput| text.set(input.value)
            rows=4
        />
    }
    .into_any()
}

fn quiet() -> AnyView {
    view! { <Text tone=TextTone::Muted>"Nội dung của route thuộc host; khung không đổi nó."</Text> }
        .into_any()
}

pub fn specimen(id: &str, scenario: &str) -> Option<AnyView> {
    let host = Host::new("explore");
    Some(match (id, scenario) {
        ("navigation.rail", scenario) => {
            let mut frame = Frame { rail_only: true, ..Frame::default() };
            let text = match scenario {
                "compact" => {
                    frame.window = WindowClass::Medium;
                    "Rail 96: icon trên, nhãn dưới; nút Mở rộng vẫn hiển thị thường trực."
                }
                "reader" => {
                    frame.route = "reader";
                    "Trang đọc tác phẩm thuộc Khám phá: Khám phá sáng, không có tab Trang đọc."
                }
                "long-labels" => {
                    frame.long = true;
                    frame.window = WindowClass::Medium;
                    "Stress rail 96 với nhãn dài: chữ lớn có thể tách từ; dùng nút Mở rộng để đọc trong rail 220. Không coi đây là PASS khả năng đọc."
                }
                "guest" => {
                    frame.primary = &["explore"];
                    "Host cấp danh sách theo capability thật; thư viện không tự thêm hay ẩn đích."
                }
                _ => "Rail 220: thương hiệu → nút thu/mở → Khám phá → Bàn viết; Tôi ở chân rail.",
            };
            view! { {note(text)} {shell(frame, host, quiet(), ViewFn::default())} }.into_any()
        }
        ("navigation.bottom-bar", scenario) => {
            let mut frame = Frame {
                window: WindowClass::Compact,
                height: 420,
                ..Frame::default()
            };
            match scenario {
                "desk" => frame.route = "desk",
                "me" => frame.route = "me",
                "long-labels" => frame.long = true,
                _ => {}
            }
            view! {
                {note("Mobile: Khám phá → Bàn viết → Tôi; Tôi là màn, không phải lớp phủ.")}
                {shell(frame, host, quiet(), ViewFn::default())}
            }
            .into_any()
        }
        ("navigation.top-bar", scenario) => {
            let frame = Frame {
                window: WindowClass::Compact,
                height: 360,
                ..Frame::default()
            };
            let actions = scenario == "with-action";
            view! {
                {note("Header hẹp chỉ có thương hiệu và hành động của route; đích chính nằm ở thanh dưới.")}
                <div class="vkp-topbar-sample">
                    <TopBar
                        brand=brand
                        actions=move || actions.then(|| view! {
                            <IconButton icon=IconName::Search label="Tìm trong Khám phá" on_press=Callback::new(|()| log("top bar → tìm"))/>
                        })
                    />
                </div>
                {shell(frame, host, quiet(), ViewFn::default())}
            }
            .into_any()
        }
        ("navigation.scaffold", scenario) => {
            let mut frame = Frame::default();
            let text = match scenario {
                "adaptive" | "adaptive-editor" => {
                    frame.adaptive = true;
                    if scenario == "adaptive-editor" {
                        frame.route = "editor";
                        frame.context = NavContext { compact_rail: true, hide_bar: true };
                    }
                    "Khung thích ứng: đo bề rộng frame thật; giữ trang, chữ, vùng chọn và lớp phủ khi đổi cỡ."
                }
                "medium" => {
                    frame.window = WindowClass::Medium;
                    "Cửa sổ vừa: mặc định rail 96."
                }
                "compact" => {
                    frame.window = WindowClass::Compact;
                    "Cửa sổ hẹp: thanh dưới; preference rail không áp."
                }
                "editor" => {
                    frame.route = "editor";
                    frame.context = NavContext {
                        compact_rail: true,
                        hide_bar: true,
                    };
                    "Đang viết: rail 96 trên cửa sổ rộng, thanh dưới ẩn trên cửa sổ hẹp; Bàn viết sáng."
                }
                "toggle" => {
                    frame.window = WindowClass::Medium;
                    "Bấm nút: 96 → 220 → 96. Trang, chữ và vị trí con trỏ giữ nguyên; không có lưu."
                }
                "short" => {
                    frame.height = 260;
                    "Thấp hoặc ngang: rail cuộn, nút thu/mở và Tôi vẫn tới được."
                }
                _ => "Cửa sổ rộng: rail 220.",
            };
            let page = if matches!(scenario, "toggle" | "editor" | "adaptive" | "adaptive-editor") {
                draft()
            } else {
                quiet()
            };
            let mode_note = if frame.adaptive { "" } else {
                "Khung ép cố định để kiểm variant. Khi không vừa, Tab vào vùng khung và cuộn ngang để xem nội dung; dùng adaptive để kiểm reflow."
            };
            let overlays = ViewFn::from(move || profile_dialog(host));
            view! { {note(text)} {(!mode_note.is_empty()).then(|| note(mode_note))} {shell(frame, host, page, overlays)} }.into_any()
        }
        ("navigation.overlay", scenario) => overlay(scenario, host),
        ("navigation.adapter", _) => view! {
            <Stack gap=Space::S3>
                {note("Leptos: unsupported. Adapter Web là router + NavigationGuard thật của app; deep link, history, guard và 404 được kiểm bằng browser smoke (cargo xtask verify web), không bằng mô phỏng trong gallery.")}
                <Text role=TypeRole::Caption tone=TextTone::Muted>"Adapter chẩn đoán có back stack và deep link nằm ở preview CMP."</Text>
            </Stack>
        }
        .into_any(),
        _ => return None,
    })
}

fn overlay(scenario: &str, host: Host) -> AnyView {
    let open = Signal::derive(move || host.overlays.with(|layer| layer.is_open(PROFILE)));
    let close = move |reason: CloseReason| {
        let closed = host
            .overlays
            .try_update(|layer| layer.close(PROFILE, reason))
            .flatten();
        log(match closed {
            Some(closed) => format!("lớp phủ → đóng ({})", closed.reason.id()),
            None => format!("lớp phủ → bỏ đóng muộn ({})", reason.id()),
        });
    };
    let back = move || {
        let closed = host.overlays.try_update(OverlayLayer::back).flatten();
        log(match closed {
            Some(_) => "Back → đóng lớp phủ trước, vẫn ở trang".to_string(),
            None => "Back → không có lớp phủ: host rời màn".to_string(),
        });
    };
    let second = move || {
        let opened = host.overlays.try_update(|layer| layer.open("nav-leave"));
        log(match opened {
            Some(Err(busy)) => format!("mở hộp thoại khác → từ chối: {} đang mở", busy.active),
            _ => "mở hộp thoại khác → mở".to_string(),
        });
    };
    let dismissed = Callback::new(move |reason: DismissReason| {
        close(match reason {
            DismissReason::Escape => CloseReason::Escape,
            DismissReason::Outside => CloseReason::Outside,
        });
    });
    if scenario != "default" {
        host.overlays.update(|layer| {
            let _ = layer.open(PROFILE);
        });
    }
    let text = match scenario {
        "open" => {
            "Tôi đang mở: một modal, focus ở trong, Escape/bấm ngoài/Đóng trả focus về trigger."
        }
        "back" => "Back khi lớp phủ mở chỉ đóng lớp phủ; lần Back sau mới rời màn.",
        "busy" => "Chỉ một modal: mở hộp thoại thứ hai khi Tôi đang mở bị từ chối.",
        _ => "Đóng: bấm Tôi ở chân rail để mở lớp phủ.",
    };
    let frame = Frame::default();
    let overlays = ViewFn::from(move || {
        view! {
            <Dialog
                id=PROFILE
                open=open
                title="Tôi"
                context="Chưa có thông tin tài khoản. Vink chưa mở đăng nhập."
                on_dismiss=dismissed
                actions=move || view! {
                    <span data-testid="nav-back">
                        <Button variant=ButtonVariant::Text on_press=move |_| back()>"Mô phỏng Back"</Button>
                    </span>
                    <span data-testid="nav-second">
                        <Button variant=ButtonVariant::Text on_press=move |_| second()>"Mở hộp thoại khác"</Button>
                    </span>
                    <span data-testid="nav-close">
                        <Button on_press=move |_| close(CloseReason::Dismiss)>"Đóng"</Button>
                    </span>
                }
            />
        }
    });
    let page = view! {
        <span data-testid="nav-back-outside">
            <Button variant=ButtonVariant::Tonal on_press=move |_| back()>"Mô phỏng Back"</Button>
        </span>
    }
    .into_any();
    view! { {note(text)} {shell(frame, host, page, overlays)} }.into_any()
}
