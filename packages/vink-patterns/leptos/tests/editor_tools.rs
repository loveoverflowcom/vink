//! The Paper First disclosure must keep a visible save-focus destination, one live
//! readout and ordinary utility buttons. Browser checks own geometry and actual focus.
use leptos::prelude::*;
use vink_patterns::EditorToolbar;
use vink_ui::{Button, ButtonVariant};

fn render(paper_first: bool) -> String {
    Owner::new().with(|| {
        view! {
            <EditorToolbar status="Đã lưu" status_token="saved" status_detail="Phiên bản 2 trên máy chủ."
                paper_first
                utilities=std::sync::Arc::new(|| view! {
                    <Button id="tool-sample" variant=ButtonVariant::Text>"Công cụ phụ"</Button>
                }.into_any())>
                <Button id="preview" variant=ButtonVariant::Text>"Xem trước"</Button>
            </EditorToolbar>
        }.to_html()
    })
}

#[test]
fn paper_first_has_one_native_tools_disclosure_and_one_live_readout() {
    let html = render(true);
    assert_eq!(html.matches("id=\"editor-save-details\"").count(), 1);
    assert_eq!(
        html.matches("<details").count(),
        1,
        "no hidden nested summary: {html}"
    );
    assert_eq!(html.matches("role=\"status\"").count(), 1);
    assert_eq!(html.matches("aria-live=\"polite\"").count(), 1);
    assert!(!html.contains("role=\"menu"));
    assert!(!html.contains(" open"), "tools start closed: {html}");
    assert!(html.contains("Thêm"));
    assert!(html.contains("Phiên bản 2 trên máy chủ."));
    let utility = html.find("id=\"tool-sample\"").unwrap();
    let start = html[..utility].rfind("<button").unwrap();
    let end = utility + html[utility..].find('>').unwrap();
    assert!(
        html[start..=end].contains("type=\"button\""),
        "utility cannot submit: {html}"
    );
    assert!(html.find("<summary").unwrap() < start);
}

#[test]
fn standard_toolbar_keeps_the_existing_detail_anatomy() {
    let html = render(false);
    assert!(html.contains("Chi tiết trạng thái lưu"));
    assert!(!html.contains("vk-editor-tools"));
    assert!(!html.contains("vk-editor-toolbar--paper-first"));
    assert_eq!(html.matches("role=\"status\"").count(), 1);
}

#[test]
fn compact_details_uses_accessible_save_details_summary_without_children() {
    let html = Owner::new().with(|| {
        view! {
            <EditorToolbar
                status="Đã lưu"
                status_token="saved"
                status_detail="Phiên bản 2 trên máy chủ."
                paper_first=true
                compact_details=true
            />
        }
        .to_html()
    });
    assert_eq!(html.matches("id=\"editor-save-details\"").count(), 1);
    assert!(html.contains("aria-label=\"Chi tiết lưu\""), "{html}");
    assert!(html.contains("Phiên bản 2 trên máy chủ."), "{html}");
    assert!(!html.contains("Thêm"), "{html}");
    assert!(html.contains("justify-start"), "{html}");
    assert!(html.contains("p-vk-3"), "{html}");
}
