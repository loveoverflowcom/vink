//! Present profile visibility without owning permission or mutation decisions.

use leptos::prelude::*;
use vink_ui::{ChoiceOption, Icon, IconName, IconSize, Select, SelectVariant};

use crate::contract::profile::Visibility;

pub(crate) fn visibility_choice(visibility: Visibility) -> ChoiceOption {
    match visibility {
        Visibility::Everyone => {
            ChoiceOption::new("everyone", "Công khai").with_icon(IconName::Public)
        }
        Visibility::Owner => ChoiceOption::new("self", "Chỉ mình tôi").with_icon(IconName::Lock),
    }
}

/// The host supplies the draft value and whether the stored policy is locked.
#[component]
pub(crate) fn PrivacySelect(
    id: &'static str,
    label: &'static str,
    #[prop(into)] value: Signal<Visibility>,
    #[prop(into)] locked: Signal<bool>,
    #[prop(into)] disabled: Signal<bool>,
    #[prop(into)] on_change: Callback<Visibility>,
    #[prop(optional)] lock_description_id: Option<&'static str>,
) -> impl IntoView {
    let values = [Visibility::Everyone, Visibility::Owner];
    let options = StoredValue::new(values.map(visibility_choice).to_vec());
    let selected = Signal::derive(move || {
        let index = usize::from(value.get() == Visibility::Owner);
        Some(options.with_value(|options| options[index].value.clone()))
    });
    let pick = move |picked: Option<String>| {
        if locked.get_untracked() || disabled.get_untracked() {
            return;
        }
        if let Some(visibility) = options.with_value(|options| {
            options.iter().zip(values).find_map(|(option, visibility)| {
                (picked.as_deref() == Some(option.value.as_str())).then_some(visibility)
            })
        }) {
            on_change.run(visibility);
        }
    };
    move || {
        if locked.get() {
            let private = visibility_choice(Visibility::Owner);
            let label_id = format!("{id}-label");
            return view! {
                <div class="vk-profile-policy vk-profile-policy--locked flex min-w-0 flex-wrap items-center gap-x-vk-3 gap-y-vk-1"
                    role="group" aria-labelledby=label_id.clone() aria-describedby=lock_description_id>
                    <span id=label_id class="flex-[1_1_10rem] text-vk-label">{label}</span>
                    <span id=id class="inline-flex min-h-[48px] min-w-[min(11rem,100%)] flex-[0_1_11rem] items-center gap-vk-2 px-vk-3 text-vk-label text-vk-on-surface-variant">
                        {private.icon.map(|name| view! { <Icon name size=IconSize::Small /> })}
                        <span>{private.label}</span>
                    </span>
                </div>
            }.into_any();
        }
        view! {
            <div class="vk-profile-policy">
                <Select id label options=options.get_value() value=selected on_change=pick
                    disabled variant=SelectVariant::IconTonal inline=true />
            </div>
        }
        .into_any()
    }
}
