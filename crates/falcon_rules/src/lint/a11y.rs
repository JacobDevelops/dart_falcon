//! Accessibility lint rules.

use crate::meta::{Domain, RuleSource};

crate::declare_rules! {
    A11y;
    accessible_widget_labels::AccessibleWidgetLabels => ("accessible-widget-labels", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
    minimum_touch_target::MinimumTouchTarget => ("minimum-touch-target", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
    avoid_missing_image_alt::AvoidMissingImageAlt => ("avoid-missing-image-alt", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
    prefer_action_button_tooltip::PreferActionButtonTooltip => ("prefer-action-button-tooltip", resolved, Some(Domain::Flutter), false, RuleSource::Falcon);
}
