//! The Editor R4 save bar must keep a visible save-focus destination, one live readout and the
//! caller's save action outside the disclosure. Browser checks own geometry and actual focus.
use leptos::prelude::*;
use vink_patterns::EditorToolbar;
use vink_ui::{Button, ButtonVariant};

fn render(with_action: bool) -> String {
    Owner::new().with(|| {
        let save_action = with_action.then(|| -> ChildrenFn {
            std::sync::Arc::new(|| {
                view! { <Button id="save" variant=ButtonVariant::Filled>"Lưu"</Button> }.into_any()
            })
        });
        match save_action {
            Some(save_action) => view! {
                <EditorToolbar status="Chưa lưu chữ" status_token="unsaved"
                    status_detail="Thay đổi chữ trên trang này chưa được lưu lên máy chủ."
                    save_action />
            }
            .to_html(),
            None => view! {
                <EditorToolbar status="Đã lưu" status_token="saved"
                    status_detail="Phiên bản 2 trên máy chủ." />
            }
            .to_html(),
        }
    })
}

#[test]
fn one_native_details_disclosure_and_one_live_readout() {
    let html = render(false);
    assert_eq!(html.matches("id=\"editor-save-details\"").count(), 1);
    assert_eq!(
        html.matches("<details").count(),
        1,
        "no hidden nested summary: {html}"
    );
    assert_eq!(html.matches("role=\"status\"").count(), 1);
    assert_eq!(html.matches("aria-live=\"polite\"").count(), 1);
    assert!(!html.contains("role=\"menu"));
    assert!(!html.contains(" open"), "details start closed: {html}");
    assert!(html.contains("aria-label=\"Chi tiết lưu\""), "{html}");
    assert!(html.contains("Phiên bản 2 trên máy chủ."), "{html}");
    assert!(
        !html.contains("Xem trước") && !html.contains("Thêm"),
        "no legacy tools: {html}"
    );
}

#[test]
fn the_save_action_stays_in_the_slot_before_the_disclosure() {
    let html = render(true);
    let slot = html.find("id=\"save-slot\"").unwrap();
    let save = html.find("id=\"save\"").unwrap();
    let details = html.find("<details").unwrap();
    assert!(slot < save && save < details, "{html}");
    assert_eq!(html.matches("role=\"status\"").count(), 1);
}
